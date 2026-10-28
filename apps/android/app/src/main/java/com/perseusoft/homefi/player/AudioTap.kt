package com.perseusoft.homefi.player

import androidx.media3.common.C
import androidx.annotation.OptIn
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.audio.TeeAudioProcessor
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.hypot
import kotlin.math.ln
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.sin
import kotlin.math.sqrt

/** One frame for the meters: channel levels (0..1) and log-spaced spectrum bands (0..1). */
class AudioLevels(val left: Float, val right: Float, val bands: FloatArray) {
    companion object {
        val Silent = AudioLevels(0f, 0f, FloatArray(AudioTap.BANDS))
    }
}

/**
 * What the current track sounds like so far, for the mascot: loudness averaged
 * over the track (0..1), estimated tempo and seconds analysed.
 */
data class Listening(val energy: Float = 0f, val bpm: Float = 120f, val heard: Float = 0f)

/**
 * Reads the PCM ExoPlayer is about to play (a [TeeAudioProcessor] in the
 * service's audio sink) and turns it into levels and a spectrum for the
 * player's visualizer, VU meters and mascot. No RECORD_AUDIO permission,
 * unlike android.media.audiofx.Visualizer. Work only happens while a view
 * watches ([watchers] > 0).
 */
@OptIn(UnstableApi::class)
object AudioTap : TeeAudioProcessor.AudioBufferSink {
    const val BANDS = 32
    private const val N = 1024
    private const val FRAME_NS = 33_000_000L // ~30 fps

    private val _levels = MutableStateFlow(AudioLevels.Silent)
    val levels: StateFlow<AudioLevels> = _levels.asStateFlow()

    private val _listening = MutableStateFlow(Listening())
    /** Tempo and long-term energy of the current track (see [track]). */
    val listening: StateFlow<Listening> = _listening.asStateFlow()

    @Volatile var watchers = 0

    /** Smoothed loudness 0..1 (mean of the spectrum), what the mascot dances to. */
    @Volatile var energy = 0f
        private set

    // tempo from bass kicks, like the desktop's `visual.svelte.ts` (audio thread only)
    @Volatile private var trackId: String? = null
    @Volatile private var forget = false
    private var bassAvg = 0f
    private var lastKick = 0L
    private var lastFrame = 0L
    private var frames = 0
    private var trackEnergy = 0f
    private var heard = 0f
    private var bpm = 120f
    private val intervals = ArrayDeque<Float>()
    private val sorted = FloatArray(16)

    private var channels = 2
    private var encoding = C.ENCODING_PCM_16BIT
    private var sampleRate = 44_100
    private val mono = FloatArray(N)
    private var filled = 0
    private var sumL = 0.0
    private var sumR = 0.0
    private var count = 0
    private var lastEmit = 0L
    private val re = FloatArray(N)
    private val im = FloatArray(N)
    private val window = FloatArray(N) { i -> (0.5 - 0.5 * cos(2 * PI * i / (N - 1))).toFloat() }
    private val smooth = FloatArray(BANDS)

    override fun flush(sampleRateHz: Int, channelCount: Int, encoding: Int) {
        sampleRate = sampleRateHz
        channels = channelCount.coerceAtLeast(1)
        this.encoding = encoding
        filled = 0
        sumL = 0.0; sumR = 0.0; count = 0
    }

    override fun handleBuffer(buffer: ByteBuffer) {
        if (watchers <= 0) return
        val data = buffer.duplicate().order(ByteOrder.nativeOrder())
        val bytes = when (encoding) {
            C.ENCODING_PCM_16BIT -> 2
            C.ENCODING_PCM_FLOAT -> 4
            else -> return // other encodings are converted before this point in practice
        }
        val frame = bytes * channels
        while (data.remaining() >= frame) {
            var l = 0f
            var r = 0f
            for (c in 0 until channels) {
                val v = if (bytes == 2) data.short / 32768f else data.float
                if (c == 0) l = v else if (c == 1) r = v
            }
            if (channels == 1) r = l
            sumL += l * l; sumR += r * r; count++
            if (filled < N) mono[filled++] = (l + r) / 2
        }
        val now = System.nanoTime()
        if (now - lastEmit >= FRAME_NS && filled == N && count > 0) {
            lastEmit = now
            emit()
        }
    }

    private fun emit() {
        val left = level(sqrt(sumL / count))
        val right = level(sqrt(sumR / count))
        sumL = 0.0; sumR = 0.0; count = 0

        for (i in 0 until N) { re[i] = mono[i] * window[i]; im[i] = 0f }
        filled = 0
        fft(re, im)
        val bins = N / 2
        val bands = FloatArray(BANDS)
        val minHz = 40.0
        val maxHz = min(16_000.0, sampleRate / 2.0)
        for (b in 0 until BANDS) {
            val lo = minHz * (maxHz / minHz).pow(b.toDouble() / BANDS)
            val hi = minHz * (maxHz / minHz).pow((b + 1).toDouble() / BANDS)
            val from = (lo / sampleRate * N).toInt().coerceIn(1, bins - 1)
            val to = (hi / sampleRate * N).toInt().coerceIn(from + 1, bins)
            var peak = 0f
            for (k in from until to) peak = max(peak, hypot(re[k], im[k]))
            val db = 20 * ln(peak / (N / 4f) + 1e-6f) / ln(10f) // ~0 dB at full scale
            val v = ((db + 60f) / 60f).coerceIn(0f, 1f)
            // fast attack, slow release, like a real analyser
            smooth[b] = if (v > smooth[b]) v else smooth[b] * 0.82f + v * 0.18f
            bands[b] = smooth[b]
        }
        _levels.value = AudioLevels(left, right, bands)
        track(bands)
    }

    /**
     * Levels from elsewhere (0..1), when this phone has no audio of its own to read: a
     * guest in remote control gets the hub's. Not while a local player is being read.
     */
    fun feed(left: Float, right: Float, bands: FloatArray) {
        val fitted = if (bands.size == BANDS) bands else FloatArray(BANDS) { i ->
            if (bands.isEmpty()) 0f else bands[i * bands.size / BANDS]
        }
        _levels.value = AudioLevels(left, right, fitted)
        track(fitted)
    }

    /** Forgets the tempo and energy of the previous track when [mediaId] changes. */
    fun listenTo(mediaId: String?) {
        if (mediaId == trackId) return
        trackId = mediaId
        forget = true
        _listening.value = Listening(bpm = bpm)
    }

    /** Loudness, kicks and the tempo they make, once per spectrum frame. */
    private fun track(bands: FloatArray) {
        if (forget) {
            forget = false
            intervals.clear()
            lastKick = 0L
            frames = 0
            trackEnergy = 0f
            heard = 0f
        }
        val t = System.nanoTime() / 1_000_000L
        heard += if (lastFrame == 0L) 0.033f else ((t - lastFrame) / 1000f).coerceIn(0f, 0.1f)
        lastFrame = t
        frames++
        energy += (bands.average().toFloat() - energy) * 0.25f
        trackEnergy += (energy - trackEnergy) / min(frames, 600)
        // a bass hit: well above its running average
        val bass = (bands[2] + bands[3] + bands[4] + bands[5]) / 4
        val kick = bass > bassAvg * 1.18f && bass > 0.45f
        bassAvg = bassAvg * 0.9f + bass * 0.1f
        if (kick && t - lastKick >= 250) {
            val gap = t - lastKick
            lastKick = t
            if (gap <= 1500) {
                // fold into 80..160 BPM so half/double-time kicks agree
                var beats = 60_000f / gap
                while (beats < 80) beats *= 2
                while (beats > 160) beats /= 2
                if (intervals.size >= 16) intervals.removeFirst()
                intervals.addLast(beats)
                if (intervals.size >= 6) {
                    val n = intervals.size
                    intervals.forEachIndexed { i, v -> sorted[i] = v }
                    sorted.sort(0, n)
                    val median = Math.round(sorted[n shr 1]).toFloat()
                    // only move on real changes, so the animation doesn't stutter
                    if (abs(median - bpm) / bpm > 0.05f) bpm = median
                }
            }
        }
        _listening.value = Listening(trackEnergy, bpm, heard)
    }

    /** RMS → 0..1 over a 36 dB range (typical music sits in the upper half). */
    private fun level(rms: Double): Float {
        val db = 20 * ln(rms + 1e-9) / ln(10.0)
        return ((db + 36) / 36).toFloat().coerceIn(0f, 1f)
    }

    /** Called when playback stops so the meters fall back to rest. */
    fun reset() {
        smooth.fill(0f)
        energy = 0f
        lastFrame = 0L
        _levels.value = AudioLevels.Silent
    }

    /** In-place radix-2 FFT. */
    private fun fft(re: FloatArray, im: FloatArray) {
        val n = re.size
        var j = 0
        for (i in 1 until n) {
            var bit = n shr 1
            while (j and bit != 0) { j = j xor bit; bit = bit shr 1 }
            j = j xor bit
            if (i < j) {
                var t = re[i]; re[i] = re[j]; re[j] = t
                t = im[i]; im[i] = im[j]; im[j] = t
            }
        }
        var len = 2
        while (len <= n) {
            val ang = -2 * PI / len
            val wr = cos(ang).toFloat()
            val wi = sin(ang).toFloat()
            var i = 0
            while (i < n) {
                var cr = 1f
                var ci = 0f
                for (k in 0 until len / 2) {
                    val ur = re[i + k]; val ui = im[i + k]
                    val vr = re[i + k + len / 2] * cr - im[i + k + len / 2] * ci
                    val vi = re[i + k + len / 2] * ci + im[i + k + len / 2] * cr
                    re[i + k] = ur + vr; im[i + k] = ui + vi
                    re[i + k + len / 2] = ur - vr; im[i + k + len / 2] = ui - vi
                    val nr = cr * wr - ci * wi
                    ci = cr * wi + ci * wr
                    cr = nr
                }
                i += len
            }
            len = len shl 1
        }
    }
}
