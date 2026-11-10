package com.perseusoft.homefi.data

import android.app.Application
import android.content.pm.PackageManager
import android.os.Build
import com.perseusoft.homefi.core.ReleaseNotes
import com.perseusoft.homefi.core.latestReleaseNotes
import com.perseusoft.homefi.core.whatsNew
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.util.Locale
import java.util.concurrent.atomic.AtomicBoolean

/** "What's new" after an update: decided by the core from the embedded CHANGELOG. */
class WhatsNew(private val app: Application, private val settings: SettingsStore, private val scope: CoroutineScope) {
    private val _pending = MutableStateFlow<List<ReleaseNotes>>(emptyList())
    /** Notes waiting to be shown; empty when there is nothing to show. */
    val pending: StateFlow<List<ReleaseNotes>> = _pending.asStateFlow()

    private val checked = AtomicBoolean(false)

    /** Once per process, off Main: decide, persist recordVersion, publish the notes. */
    fun check() {
        if (!checked.compareAndSet(false, true)) return
        scope.launch {
            runCatching {
                val current = settings.current
                val plan = whatsNew(current.lastSeenVersion, wasUpgraded(), current.whatsNew, language())
                if (plan.recordVersion != current.lastSeenVersion) {
                    settings.update { it.copy(lastSeenVersion = plan.recordVersion) }
                }
                _pending.value = plan.notes
            }
        }
    }

    /** Shows [notes] on demand (Settings, About) through the same dialog as after an update. */
    fun show(notes: List<ReleaseNotes>) {
        _pending.value = notes
    }

    fun dismiss() {
        _pending.value = emptyList()
    }

    /** Settings → About → What's new (off Main). */
    suspend fun latest(): ReleaseNotes? = withContext(Dispatchers.Default) {
        runCatching { latestReleaseNotes(language()) }.getOrNull()
    }

    /** The app was installed before and replaced since; any failure counts as a fresh install. */
    private fun wasUpgraded(): Boolean = runCatching {
        val info = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            app.packageManager.getPackageInfo(app.packageName, PackageManager.PackageInfoFlags.of(0))
        } else {
            @Suppress("DEPRECATION")
            app.packageManager.getPackageInfo(app.packageName, 0)
        }
        info.firstInstallTime != info.lastUpdateTime
    }.getOrDefault(false)

    private fun language(): String = Locale.getDefault().language
}
