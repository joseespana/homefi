package com.perseusoft.homefi.data

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings
import androidx.core.content.FileProvider
import com.perseusoft.homefi.BuildConfig
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.AppRelease
import com.perseusoft.homefi.core.checkForUpdate
import com.perseusoft.homefi.core.releaseSha256
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest

enum class UpdateStatus { IDLE, CHECKING, UP_TO_DATE, AVAILABLE, DOWNLOADING, READY, ERROR }

/** What the update card and Settings → About show. */
data class UpdateUi(
    val status: UpdateStatus = UpdateStatus.IDLE,
    /** The newer release, once one was found. */
    val release: AppRelease? = null,
    val downloaded: Long = 0,
    /** 0 when the size isn't known. */
    val total: Long = 0,
    /** Already in plain words. */
    val error: String? = null,
    /** Epoch ms of the last check; 0 = never. */
    val lastChecked: Long = 0,
    /** "Later": the card stays hidden for this version until the app is opened again. */
    val dismissed: Boolean = false,
    /** Android asks to allow installing from HomeFi first ("Install unknown apps"). */
    val needsPermission: Boolean = false,
)

/**
 * "HomeFi X is available": looks at GitHub's releases (through the core; nothing about the
 * user is sent) at most every [CHECK_INTERVAL_MS] while the app is in the foreground,
 * downloads `HomeFi-android.apk`, checks its SHA-256 against the release's `SHA256SUMS` and
 * hands it to Android's installer, which asks the user. Off for builds that aren't the
 * official app ([OFFICIAL_ID]): the SR35 one takes another app's id and debug builds end in
 * `.debug`; those are updated by USB.
 */
class AppUpdater(private val context: Context, private val settings: SettingsStore, private val scope: CoroutineScope) {
    companion object {
        const val OFFICIAL_ID = "com.perseusoft.homefi"
        const val CHECK_INTERVAL_MS = 6 * 60 * 60 * 1000L
        const val APK_NAME = "HomeFi-android.apk"
    }

    /** Only the official build updates itself. */
    val enabled: Boolean = BuildConfig.APPLICATION_ID == OFFICIAL_ID

    private val prefs = context.getSharedPreferences("updates", Context.MODE_PRIVATE)
    private val _state = MutableStateFlow(UpdateUi(lastChecked = prefs.getLong("last_checked", 0L)))
    val state: StateFlow<UpdateUi> = _state.asStateFlow()
    private var job: Job? = null

    private val dir: File get() = context.cacheDir.resolve("updates")
    private fun apkFor(version: String) = dir.resolve("HomeFi-$version.apk")

    /** The app came to the foreground: check when it's on and due. */
    fun onForeground() {
        if (!enabled || !settings.current.updateAutoCheck) return
        if (System.currentTimeMillis() - _state.value.lastChecked >= CHECK_INTERVAL_MS) check(manual = false)
        // back from "Install unknown apps" with it allowed: carry on with the install
        if (_state.value.needsPermission && canInstall()) install()
    }

    /** "Check now" (manual) or the periodic check. */
    fun check(manual: Boolean = true) {
        if (!enabled) return
        val s = _state.value.status
        if (s == UpdateStatus.CHECKING || s == UpdateStatus.DOWNLOADING) return
        job = scope.launch {
            _state.update { it.copy(status = UpdateStatus.CHECKING, error = null, dismissed = if (manual) false else it.dismissed) }
            val now = System.currentTimeMillis()
            prefs.edit().putLong("last_checked", now).apply()
            try {
                val release = checkForUpdate(BuildConfig.VERSION_NAME, settings.current.updatePrereleases)
                // a copy already downloaded and checked for this version is reused
                val ready = release != null && apkFor(release.version).isFile && prefs.getString("verified", null) == release.version
                _state.update {
                    it.copy(
                        status = when {
                            release == null -> UpdateStatus.UP_TO_DATE
                            ready -> UpdateStatus.READY
                            else -> UpdateStatus.AVAILABLE
                        },
                        release = release,
                        lastChecked = now,
                        dismissed = if (it.release?.version != release?.version) false else it.dismissed,
                    )
                }
                if (release == null) cleanUp(keep = null)
            } catch (e: Exception) {
                _state.update { it.copy(status = UpdateStatus.ERROR, error = e.userMessage(), lastChecked = now) }
            }
        }
    }

    /** "Later". */
    fun dismiss() = _state.update { it.copy(dismissed = true, needsPermission = false) }

    /** The "allow installing" explanation was closed without going there. */
    fun skipPermission() = _state.update { it.copy(needsPermission = false) }

    /** "Update": downloads the APK, checks it, then opens Android's installer. */
    fun download() {
        val release = _state.value.release ?: return
        if (_state.value.status == UpdateStatus.READY) return install()
        val url = release.apkUrl
        val sums = release.sha256sumsUrl
        if (url == null || sums == null) {
            _state.update { it.copy(status = UpdateStatus.ERROR, error = context.getString(R.string.update_no_apk)) }
            return
        }
        job?.cancel()
        job = scope.launch {
            _state.update { it.copy(status = UpdateStatus.DOWNLOADING, downloaded = 0, total = release.apkSize.toLong(), error = null) }
            val target = apkFor(release.version)
            try {
                val expected = releaseSha256(sums, APK_NAME)
                    ?: throw UpdateError(context.getString(R.string.update_no_checksum))
                val actual = fetch(url, target)
                if (!actual.equals(expected, ignoreCase = true)) {
                    target.delete()
                    throw UpdateError(context.getString(R.string.update_bad_checksum))
                }
                checkPackage(target, release)
                prefs.edit().putString("verified", release.version).apply()
                cleanUp(keep = target)
                _state.update { it.copy(status = UpdateStatus.READY, downloaded = target.length(), total = target.length()) }
                install()
            } catch (e: kotlinx.coroutines.CancellationException) {
                target.delete()
                throw e
            } catch (e: Exception) {
                target.delete()
                _state.update { it.copy(status = UpdateStatus.ERROR, error = (e as? UpdateError)?.message ?: e.userMessage()) }
            }
        }
    }

    /** Streams [url] into [file], reporting progress; returns its SHA-256 (hex). */
    private suspend fun fetch(url: String, file: File): String = withContext(Dispatchers.IO) {
        dir.mkdirs()
        val digest = MessageDigest.getInstance("SHA-256")
        // GitHub answers with a redirect to its file host (https to https: followed)
        val connection = (URL(url).openConnection() as HttpURLConnection).apply {
            connectTimeout = 15_000
            readTimeout = 30_000
            setRequestProperty("User-Agent", "HomeFi-android/${BuildConfig.VERSION_NAME}")
        }
        try {
            if (connection.responseCode !in 200..299) throw UpdateError(context.getString(R.string.update_download_failed))
            val length = connection.contentLengthLong.takeIf { it > 0 } ?: _state.value.total
            var got = 0L
            var lastReport = 0L
            connection.inputStream.use { input ->
                file.outputStream().use { output ->
                    val buffer = ByteArray(64 * 1024)
                    while (true) {
                        ensureActive()
                        val n = input.read(buffer)
                        if (n < 0) break
                        output.write(buffer, 0, n)
                        digest.update(buffer, 0, n)
                        got += n
                        val now = System.currentTimeMillis()
                        if (now - lastReport > 150) {
                            lastReport = now
                            _state.update { it.copy(downloaded = got, total = length) }
                        }
                    }
                }
            }
            digest.digest().joinToString("") { "%02x".format(it) }
        } finally {
            connection.disconnect()
        }
    }

    /** The file is HomeFi, and the version the release says (Android checks the signature). */
    private fun checkPackage(apk: File, release: AppRelease) {
        val info = context.packageManager.getPackageArchiveInfo(apk.absolutePath, 0)
            ?: throw UpdateError(context.getString(R.string.update_bad_package))
        if (info.packageName != context.packageName || info.versionName != release.version) {
            throw UpdateError(context.getString(R.string.update_bad_package))
        }
    }

    private fun cleanUp(keep: File?) {
        dir.listFiles()?.filter { it != keep }?.forEach { it.delete() }
    }

    private fun canInstall(): Boolean =
        Build.VERSION.SDK_INT < Build.VERSION_CODES.O || context.packageManager.canRequestPackageInstalls()

    /** Opens Android's installer on the checked APK (it asks the user; the music stops while it installs). */
    fun install() {
        val release = _state.value.release ?: return
        val apk = apkFor(release.version)
        if (!apk.isFile) {
            _state.update { it.copy(status = UpdateStatus.AVAILABLE) }
            return
        }
        if (!canInstall()) {
            _state.update { it.copy(needsPermission = true) }
            return
        }
        _state.update { it.copy(needsPermission = false) }
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.updates", apk)
        val intent = Intent(Intent.ACTION_VIEW)
            .setDataAndType(uri, "application/vnd.android.package-archive")
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
        try {
            context.startActivity(intent)
        } catch (e: ActivityNotFoundException) {
            _state.update { it.copy(status = UpdateStatus.ERROR, error = context.getString(R.string.update_no_installer)) }
        }
    }

    /** "Allow from this source": the system page for HomeFi, then back to [install]. */
    fun openInstallPermission() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val intent = Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${context.packageName}"))
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        runCatching { context.startActivity(intent) }
    }

    /** The release page in the browser ("What's new" in full). */
    fun openPage() {
        val url = _state.value.release?.pageUrl ?: "https://github.com/joseespana/homefi/releases"
        runCatching { context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
    }

    private class UpdateError(message: String) : Exception(message)
}
