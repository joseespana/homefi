package com.perseusoft.homefi.tv

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import android.widget.Toast
import com.perseusoft.homefi.BuildConfig
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.AppRelease
import com.perseusoft.homefi.data.UpdateStatus
import com.perseusoft.homefi.data.UpdateUi
import com.perseusoft.homefi.core.parseLink
import com.perseusoft.homefi.data.AppEntry
import com.perseusoft.homefi.homefi
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch

/**
 * What the TV app does with the intents it is opened with (a notification's tap, `MainActivity`'s
 * forward, a Tidal link, an audio file): the same rules as the phone's `MainActivity`.
 * Returns true when the full player should open.
 */
class TvIntents(private val context: Context, private val scope: CoroutineScope) {
    private val app get() = context.homefi

    fun handle(intent: Intent?): Boolean {
        intent ?: return false
        if (BuildConfig.DEBUG) intent.getStringExtra(DEBUG_UPDATE)?.let(::previewUpdate)
        if (intent.getBooleanExtra(AppEntry.EXTRA_OPEN_PLAYER, false)) return true
        val data = intent.data
        return when {
            intent.action == Intent.ACTION_VIEW && data != null && data.scheme in LOCAL_SCHEMES -> playFile(data, intent.type)
            intent.action == Intent.ACTION_SEND -> {
                queueLink(intent.getStringExtra(Intent.EXTRA_TEXT))
                false
            }
            intent.action == Intent.ACTION_VIEW && data != null && data.scheme != "homefi" -> {
                queueLink(intent.dataString)
                false
            }
            else -> false
        }
    }

    /** Debug builds: `--es debug_update available|downloading|ready|permission|error` shows that update state without a release. */
    private fun previewUpdate(kind: String) {
        val release = AppRelease(
            version = "9.9.9", tag = "v9.9.9", notes = "## Added\n- A new thing for the TV\n- Another one\n## Fixed\n- Something that was broken",
            pageUrl = "https://github.com/joseespana/homefi/releases", prerelease = false, publishedAt = null, apkUrl = null, apkSize = 80_000_000uL, sha256sumsUrl = null,
        )
        val ui = when (kind) {
            "downloading" -> UpdateUi(UpdateStatus.DOWNLOADING, release, downloaded = 32_000_000, total = 80_000_000)
            "ready" -> UpdateUi(UpdateStatus.READY, release)
            "permission" -> UpdateUi(UpdateStatus.READY, release, needsPermission = true)
            "error" -> UpdateUi(UpdateStatus.ERROR, release, error = "The download failed. Try again in a moment.")
            else -> UpdateUi(UpdateStatus.AVAILABLE, release)
        }
        app.updates.preview(ui)
    }

    private fun playFile(uri: Uri, type: String?): Boolean {
        val mime = type ?: context.contentResolver.getType(uri)
        if (mime != null && !mime.startsWith("audio/") && mime != "application/ogg") {
            Toast.makeText(context, R.string.open_unsupported, Toast.LENGTH_SHORT).show()
            return false
        }
        val path = uri.path
        if (uri.scheme == "file" && path != null) app.player.playFile(path) else app.player.playUri(uri, displayName(uri))
        return true
    }

    private fun displayName(uri: Uri): String? =
        if (uri.scheme == "file") {
            uri.lastPathSegment?.substringBeforeLast('.')
        } else {
            runCatching {
                context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
                    if (c.moveToFirst()) c.getString(0)?.substringBeforeLast('.') else null
                }
            }.getOrNull()
        }

    /** A Tidal link shared to the app: queued for download, like the phone does (a cold app may not know the account yet). */
    private fun queueLink(text: String?) {
        text ?: return
        val link = LINK.find(text)?.value?.let { runCatching { parseLink(it) }.getOrNull() }
        scope.launch {
            if (link == null || app.account.current() == null) {
                Toast.makeText(context, R.string.share_invalid, Toast.LENGTH_SHORT).show()
                return@launch
            }
            val path = "${link.kind.name.lowercase()}/${link.id}"
            app.downloads.enqueue(path, listOf(path))
            Toast.makeText(context, context.getString(R.string.share_queued, path), Toast.LENGTH_SHORT).show()
        }
    }

    private companion object {
        const val DEBUG_UPDATE = "debug_update"
        val LOCAL_SCHEMES = setOf("content", "file")
        val LINK = Regex("""https?://\S*tidal\.com/\S+""")
    }
}
