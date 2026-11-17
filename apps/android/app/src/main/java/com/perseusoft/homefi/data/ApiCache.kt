package com.perseusoft.homefi.data

import com.perseusoft.homefi.core.HomefiEngine
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

/** The saved Tidal answers' sizes (MB). The default and the top match `homefi_core::api::cache`. */
object ApiCacheDefaults {
    const val DEFAULT_MB = 200
    const val MAX_MB = 4096
    /** What the phone and the TV offer; 0 = off. */
    val STEPS_MB = listOf(0, 50, 100, 200, 500, 1024)
}

/**
 * The saved Tidal answers (the core's API cache, apart from downloads and the library): keeps the core's
 * budget in step with [SettingsStore] and tells the settings screens how much it takes. Phone and TV share it.
 */
class ApiCache(private val engine: HomefiEngine, settings: SettingsStore, scope: CoroutineScope) {
    private val _usedBytes = MutableStateFlow<Long?>(null)
    /** Bytes on disk now; null until measured. */
    val usedBytes: StateFlow<Long?> = _usedBytes.asStateFlow()

    /** A new budget, a clear and a measure never run at once, so the number shown is the one after them. */
    private val busy = Mutex()

    init {
        scope.launch {
            settings.settings.map { it.apiCacheMb }.distinctUntilChanged().collect { mb ->
                busy.withLock {
                    // a smaller budget (or off) deletes what no longer fits before answering
                    engine.setApiCacheMb(mb.coerceIn(0, ApiCacheDefaults.MAX_MB).toUInt())
                    measureLocked()
                }
            }
        }
    }

    /** Measures again (a settings screen opens). */
    suspend fun refresh() = busy.withLock { measureLocked() }

    /** Deletes every saved answer; false when the folder couldn't be emptied. */
    suspend fun clear(): Boolean = busy.withLock {
        val cleared = runCatching { engine.clearApiCache() }.isSuccess
        measureLocked()
        cleared
    }

    private suspend fun measureLocked() {
        _usedBytes.value = runCatching { engine.apiCacheUsage().toLong() }.getOrNull()
    }
}
