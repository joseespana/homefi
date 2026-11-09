package com.perseusoft.homefi.data

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * The mounted storage volumes and their free space, kept up to date when a USB drive or an SD card
 * comes or goes. Downloads read it to pause on a removal; the TV's Downloads screen draws it.
 */
class VolumeMonitor(private val context: Context, private val scope: CoroutineScope) {
    private val _volumes = MutableStateFlow<List<VolumeInfo>>(emptyList())
    val volumes: StateFlow<List<VolumeInfo>> = _volumes.asStateFlow()

    private val receiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) = refresh()
    }

    init {
        // mounts and removals of media arrive as "file" scheme broadcasts
        val filter = IntentFilter().apply {
            listOf(
                Intent.ACTION_MEDIA_MOUNTED,
                Intent.ACTION_MEDIA_UNMOUNTED,
                Intent.ACTION_MEDIA_EJECT,
                Intent.ACTION_MEDIA_REMOVED,
                Intent.ACTION_MEDIA_BAD_REMOVAL,
            ).forEach(::addAction)
            addDataScheme("file")
        }
        context.registerReceiver(receiver, filter)
        refresh()
    }

    /** Reads the volumes again (also for the free space, which changes as downloads write). */
    fun refresh() {
        scope.launch {
            val now = withContext(Dispatchers.IO) { runCatching { TvStorage.volumes(context) }.getOrDefault(_volumes.value) }
            _volumes.value = now
        }
    }

    /** The mounted removable volume with the most room, if any: where "USB" downloads go. */
    fun usb(): VolumeInfo? = _volumes.value.filter { it.removable }.maxByOrNull { it.freeBytes }
}
