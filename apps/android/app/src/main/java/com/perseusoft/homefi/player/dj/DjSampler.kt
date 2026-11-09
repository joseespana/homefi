package com.perseusoft.homefi.player.dj

import android.content.Context
import android.media.AudioAttributes
import android.media.SoundPool
import android.util.Log
import com.perseusoft.homefi.core.SfxKind
import com.perseusoft.homefi.core.djSfxWav
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import java.util.concurrent.ConcurrentHashMap

/**
 * The pads: the core synthesizes each sound once into a small WAV in the cache and
 * `SoundPool` plays it (low latency, no decoder at tap time). Only the sounds the pads
 * use are loaded.
 */
class DjSampler(context: Context, private val scope: CoroutineScope) {
    private val dir = File(context.cacheDir, "dj-sfx").also { it.mkdirs() }
    private val ids = ConcurrentHashMap<SfxKind, Int>()
    private val ready = ConcurrentHashMap.newKeySet<Int>()
    private var pool: SoundPool? = SoundPool.Builder()
        .setMaxStreams(MAX_STREAMS)
        .setAudioAttributes(
            AudioAttributes.Builder()
                .setUsage(AudioAttributes.USAGE_MEDIA)
                .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
                .build(),
        )
        .build()
        .also { sp -> sp.setOnLoadCompleteListener { _, id, status -> if (status == 0) ready += id } }

    /** Loads the sounds the pads use now (those already loaded stay). */
    fun prepare(kinds: Collection<SfxKind>) {
        val missing = kinds.filter { !ids.containsKey(it) }
        if (missing.isEmpty()) return
        scope.launch {
            for (kind in missing) {
                val file = withContext(Dispatchers.IO) {
                    runCatching {
                        File(dir, "${kind.name.lowercase()}.wav").also { it.writeBytes(djSfxWav(kind, SAMPLE_RATE_HZ)) }
                    }.onFailure { Log.w(TAG, "could not write the $kind sound", it) }.getOrNull()
                } ?: continue
                pool?.load(file.absolutePath, 1)?.let { ids[kind] = it }
            }
        }
    }

    /** Plays a pad's sound at [volume] (0..1); a sound that is not loaded yet is skipped. */
    fun play(kind: SfxKind, volume: Float) {
        val id = ids[kind] ?: return
        if (id !in ready) return
        val v = volume.coerceIn(0f, 1f)
        pool?.play(id, v, v, 1, 0, 1f)
    }

    fun release() {
        pool?.release()
        pool = null
        ids.clear()
        ready.clear()
    }

    private companion object {
        const val TAG = "HomeFiDjSampler"
        const val MAX_STREAMS = 4
        const val SAMPLE_RATE_HZ = 44_100u
    }
}
