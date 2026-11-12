package com.perseusoft.homefi.player.exercise

import android.content.Context
import android.media.AudioAttributes
import android.media.SoundPool
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.util.Log
import com.perseusoft.homefi.core.CueKind
import com.perseusoft.homefi.core.RideCue
import com.perseusoft.homefi.core.exerciseCueWav
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import java.util.concurrent.ConcurrentHashMap

/**
 * The ride's sounds: the core synthesizes each cue once into a small WAV in the cache and `SoundPool`
 * plays it (low latency, no decoder at the moment of the cue), like the DJ's pads. [schedule] is
 * handed every view's cues: each one (identified by its stable id) sounds once, at its moment.
 * Main thread only, except [prepare].
 */
class RideCues(context: Context, private val scope: CoroutineScope) {
    private val dir = File(context.cacheDir, "ride-cues").also { it.mkdirs() }
    private val ids = ConcurrentHashMap<CueKind, Int>()
    private val ready = ConcurrentHashMap.newKeySet<Int>()
    private val main = Handler(Looper.getMainLooper())
    private val pool: SoundPool = SoundPool.Builder()
        .setMaxStreams(MAX_STREAMS)
        .setAudioAttributes(
            AudioAttributes.Builder()
                .setUsage(AudioAttributes.USAGE_MEDIA)
                .setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION)
                .build(),
        )
        .build()
        .also { sp -> sp.setOnLoadCompleteListener { _, id, status -> if (status == 0) ready += id } }

    /** Cue ids already sounded or waiting for their moment, so a view repeating a cue never doubles it. */
    private val handled = HashSet<UInt>()
    private val waiting = HashMap<UInt, Runnable>()

    /** Writes and loads every cue (once per process); a cue that fails to load is skipped when it is due. */
    fun prepare() {
        val missing = CueKind.entries.filter { !ids.containsKey(it) }
        if (missing.isEmpty()) return
        scope.launch {
            for (kind in missing) {
                val file = withContext(Dispatchers.IO) {
                    runCatching {
                        File(dir, "${kind.name.lowercase()}.wav").also { it.writeBytes(exerciseCueWav(kind, SAMPLE_RATE_HZ)) }
                    }.onFailure { Log.w(TAG, "could not write the $kind cue", it) }.getOrNull()
                } ?: continue
                ids[kind] = pool.load(file.absolutePath, 1)
            }
        }
    }

    /** Sounds the cues of the latest view at their moments (louder or quieter with [volume], 0..1). */
    fun schedule(cues: List<RideCue>, volume: Float) {
        for (cue in cues) {
            if (cue.id in handled) continue
            // a cue a little late still sounds; one far in the past belongs to a view that arrived late
            if (cue.inMs < -LATE_TOLERANCE_MS) continue
            handled += cue.id
            val delay = cue.inMs.coerceAtLeast(0)
            if (delay == 0L) {
                play(cue.kind, volume)
            } else {
                val task = Runnable {
                    waiting.remove(cue.id)
                    play(cue.kind, volume)
                }
                waiting[cue.id] = task
                main.postDelayed(task, delay)
            }
        }
    }

    /** One click of the metronome, [inMs] from now. */
    fun click(inMs: Long, volume: Float) {
        if (inMs <= 0) play(CueKind.CLICK, volume) else main.postDelayed({ play(CueKind.CLICK, volume) }, inMs)
    }

    private var clickTrain: Runnable? = null

    /**
     * The bike calibration's metronome: one click per pedal turn at [rpm] (80 RPM = 750 ms) for [seconds], at
     * [volume] (0..1), then [onDone]. Each click is scheduled from the start time, so the beat does not drift.
     * A train already running is replaced.
     */
    fun playClickTrain(seconds: Int, rpm: Int, volume: Float, onDone: () -> Unit = {}) {
        stopClickTrain()
        val intervalMs = MS_PER_MINUTE / rpm.coerceIn(MIN_TRAIN_RPM, MAX_TRAIN_RPM)
        val total = (seconds.coerceIn(1, MAX_TRAIN_SECONDS) * 1000L / intervalMs).toInt().coerceAtLeast(1)
        val startMs = SystemClock.uptimeMillis()
        var played = 0
        val step = object : Runnable {
            override fun run() {
                play(CueKind.CLICK, volume)
                if (++played >= total) {
                    // the last click is heard before the sheet says it is over
                    val done = Runnable {
                        clickTrain = null
                        onDone()
                    }
                    clickTrain = done
                    main.postDelayed(done, intervalMs)
                    return
                }
                main.postAtTime(this, startMs + played * intervalMs)
            }
        }
        clickTrain = step
        main.post(step)
    }

    /** Stops the calibration clicks at once. */
    fun stopClickTrain() {
        clickTrain?.let(main::removeCallbacks)
        clickTrain = null
    }

    /** Playback paused: cues still waiting will be offered again by the next views, at their new moments. */
    fun hold() {
        waiting.forEach { (id, task) ->
            main.removeCallbacks(task)
            handled -= id
        }
        waiting.clear()
    }

    /** A new ride starts: no cue of the last one counts. */
    fun reset() {
        hold()
        handled.clear()
    }

    fun play(kind: CueKind, volume: Float) {
        val id = ids[kind] ?: return
        if (id !in ready) return
        val v = volume.coerceIn(0f, 1f)
        pool.play(id, v, v, 1, 0, 1f)
    }

    private companion object {
        const val TAG = "HomeFiRideCues"
        const val MAX_STREAMS = 3
        const val SAMPLE_RATE_HZ = 44_100u
        const val LATE_TOLERANCE_MS = 400L
        const val MS_PER_MINUTE = 60_000L
        const val MIN_TRAIN_RPM = 20
        const val MAX_TRAIN_RPM = 200
        const val MAX_TRAIN_SECONDS = 120
    }
}
