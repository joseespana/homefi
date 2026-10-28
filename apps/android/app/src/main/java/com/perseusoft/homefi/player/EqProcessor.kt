package com.perseusoft.homefi.player

import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.audio.AudioProcessor
import androidx.media3.common.audio.BaseAudioProcessor
import androidx.media3.common.util.UnstableApi
import com.perseusoft.homefi.core.Biquad
import com.perseusoft.homefi.core.EqSettings
import com.perseusoft.homefi.core.NowPlayingContext
import com.perseusoft.homefi.core.TasteSettings
import com.perseusoft.homefi.core.soundBiquads
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.concurrent.atomic.AtomicInteger
import kotlin.math.abs

/**
 * Everything that decides one track's filters: the equalizer, the taste rules, the output,
 * what plays (for the rules) and the loudness gain. [held] ("hold to hear without EQ")
 * mutes the equalizer and the taste layers but keeps the loudness, so the comparison is at
 * the same level.
 */
data class SoundInput(
    val eq: EqSettings,
    val taste: TasteSettings,
    val outputId: String,
    val now: NowPlayingContext,
    val loudnessDb: Float,
    val held: Boolean,
) {
    /** The filters at [sampleRate] (see `soundBiquads`); empty = bit-perfect. */
    fun biquads(sampleRate: Int): List<Biquad> = soundBiquads(
        if (held) eq.copy(enabled = false) else eq,
        if (held) taste.copy(enabled = false) else taste,
        outputId,
        now,
        loudnessDb,
        sampleRate.toUInt(),
    )
}

/**
 * One player's filters and crossfade level. The service runs two players ("decks", see
 * [Decks]) so the next song can start while this one fades out; each has its own slot, so
 * each song keeps its own taste and loudness while they overlap.
 */
class EqSlot internal constructor() {
    @Volatile private var input: SoundInput? = null

    /** Bumped on every change; unique across slots (see [EqRuntime.nextVersion]). */
    @Volatile var version: Int = 0
        private set

    /** Crossfade level of this deck (0..1); the processor ramps to it across each buffer. */
    @Volatile var fadeGain: Float = 1f

    /** Measure the peak level (smart crossfade: is the music over?). */
    @Volatile var measure: Boolean = false
    @Volatile private var peak: Float = -1f

    /** Highest level (0..1) since the last call while [measure] is on; -1 when no audio went through. */
    fun takePeak(): Float = peak.also { peak = -1f }

    internal fun notePeak(p: Float) {
        if (p > peak) peak = p
    }

    /** Same input as before: nothing to do (no glide, no recomputing on the audio thread). */
    fun configure(next: SoundInput) {
        if (next == input) return
        input = next
        version = EqRuntime.nextVersion()
    }

    /**
     * Coefficients for [sampleRate], five per stage (`b0 b1 b2 a1 a2`, `a0` = 1), or `null`
     * when nothing should run: the samples then pass untouched.
     * Called on the audio thread only when [version] or the rate changes.
     */
    internal fun coefficients(sampleRate: Int): DoubleArray? {
        val p = input ?: return null
        if (sampleRate <= 0) return null
        val stages = runCatching { p.biquads(sampleRate) }.getOrNull()
        if (stages.isNullOrEmpty()) return null
        val out = DoubleArray(stages.size * 5)
        stages.forEachIndexed { i, b ->
            out[i * 5] = b.b0
            out[i * 5 + 1] = b.b1
            out[i * 5 + 2] = b.b2
            out[i * 5 + 3] = b.a1
            out[i * 5 + 4] = b.a2
        }
        return out
    }
}

/**
 * What the audio threads should run, set from the app ([SoundLayers]) and read by every
 * [EqAudioProcessor] (the app's two decks and a party guest's player). [configure] sets the
 * active deck's slot: the one the media session shows. Each change gets a new version;
 * a processor picks the new filters up at its next buffer and glides into them.
 */
object EqRuntime {
    private val counter = AtomicInteger()

    internal fun nextVersion(): Int = counter.incrementAndGet()

    /** One slot per deck of the playback service. */
    val decks: List<EqSlot> = listOf(EqSlot(), EqSlot())

    /** The deck the media session plays (its slot follows the current track). */
    @Volatile var activeDeck: Int = 0

    val active: EqSlot get() = decks[activeDeck]

    val version: Int get() = active.version

    fun configure(next: SoundInput) = active.configure(next)
}

/** Biquads in series, transposed direct form II, with their own state per channel. */
private class Chain(val coef: DoubleArray, private val channels: Int) {
    val stages = coef.size / 5
    /** `s1 s2` per stage per channel. */
    val state = DoubleArray(stages * channels * 2)

    fun run(input: Double, channel: Int): Double {
        var x = input
        var s = channel * stages * 2
        var c = 0
        for (i in 0 until stages) {
            val y = coef[c] * x + state[s]
            state[s] = coef[c + 1] * x - coef[c + 3] * y + state[s + 1]
            state[s + 1] = coef[c + 2] * x - coef[c + 4] * y
            x = y
            s += 2
            c += 5
        }
        return x
    }

    /** Same shape (a band moved): carry the filter memory over, so there is no restart thump. */
    fun takeStateOf(other: Chain?) {
        if (other != null && other.stages == stages && other.state.size == state.size) other.state.copyInto(state)
    }
}

/**
 * The equalizer, taste layers and loudness in the player's audio sink: runs [slot]'s filters on 16-bit or float
 * PCM (Media3 hands 16-bit to custom processors unless float output is on, and float output
 * skips them). New filters cross-fade in over [FADE_MS] so moving a band or switching output
 * never clicks; with no filters the bytes are copied as they come (bit-perfect). The slot's
 * crossfade level ramps smoothly across each buffer (a deck fading in or out).
 * No allocation per buffer beyond Media3's own output buffer.
 *
 * [slot] is read per buffer: a deck's own slot, or by default the active deck's (a party
 * guest's player follows whatever the session plays).
 */
@OptIn(UnstableApi::class)
class EqAudioProcessor(private val slot: () -> EqSlot = { EqRuntime.active }) : BaseAudioProcessor() {
    private var channels = 2
    private var rate = 0
    private var float = false
    private var seen = -1
    private var current: Chain? = null
    private var next: Chain? = null
    private var fading = false
    private var fadePos = 0
    private var fadeLen = 1
    private var noise = 0x2545F491
    /** Crossfade level reached at the end of the last buffer. */
    private var gain = 1f

    override fun onConfigure(inputAudioFormat: AudioProcessor.AudioFormat): AudioProcessor.AudioFormat =
        when (inputAudioFormat.encoding) {
            C.ENCODING_PCM_16BIT, C.ENCODING_PCM_FLOAT -> inputAudioFormat
            else -> AudioProcessor.AudioFormat.NOT_SET // other encodings never reach here in practice
        }

    override fun onFlush() {
        channels = inputAudioFormat.channelCount.coerceAtLeast(1)
        float = inputAudioFormat.encoding == C.ENCODING_PCM_FLOAT
        // a seek or a new format: start clean at once (no fade across a discontinuity)
        rate = inputAudioFormat.sampleRate
        val sl = slot()
        seen = sl.version
        current = sl.coefficients(rate)?.let { Chain(it, channels) }
        gain = sl.fadeGain
        next = null
        fading = false
        fadeLen = (rate * FADE_MS / 1000).coerceAtLeast(1)
    }

    override fun onReset() {
        current = null
        next = null
        fading = false
        seen = -1
    }

    override fun queueInput(inputBuffer: ByteBuffer) {
        val size = inputBuffer.remaining()
        if (size == 0) return
        val sl = slot()
        val v = sl.version
        if (!fading && v != seen) {
            seen = v
            val target = sl.coefficients(rate)?.let { Chain(it, channels) }
            if (target != null || current != null) {
                target?.takeStateOf(current)
                next = target
                fading = true
                fadePos = 0
            }
        }
        val target = sl.fadeGain.coerceIn(0f, 1f)
        val from = gain
        gain = target
        val measure = sl.measure
        val out = replaceOutputBuffer(size)
        if (!fading && current == null && from == 1f && target == 1f) {
            if (measure) notePeak(inputBuffer)
            out.put(inputBuffer) // bypass: the samples go through untouched
            out.flip()
            return
        }
        val input = inputBuffer.order(ByteOrder.nativeOrder())
        val bytes = if (float) 4 else 2
        val frames = size / (bytes * channels)
        var peak = 0.0
        for (f in 0 until frames) {
            val g = if (fading) fadePos.toDouble() / fadeLen else 1.0
            // the deck's crossfade level, ramped across the buffer (no steps, no clicks)
            val level = if (from == target) target.toDouble() else from + (target - from) * (f + 1).toDouble() / frames
            for (ch in 0 until channels) {
                val x = if (float) input.float.toDouble() else input.short / 32768.0
                if (measure && abs(x) > peak) peak = abs(x)
                val y = if (fading) {
                    val a = current?.run(x, ch) ?: x
                    val b = next?.run(x, ch) ?: x
                    a + (b - a) * g
                } else {
                    // a fade to bypass can end mid-buffer: the rest passes through
                    current?.run(x, ch) ?: x
                }
                val z = y * level
                if (float) out.putFloat(z.toFloat()) else out.putShort(toShort(z))
            }
            if (fading && ++fadePos >= fadeLen) {
                current = next
                next = null
                fading = false
            }
        }
        // a partial frame (never in practice): pass it through
        while (input.hasRemaining()) out.put(input.get())
        out.flip()
        if (measure) sl.notePeak(peak.toFloat())
    }

    /** Peak of a buffer that passes untouched (read without moving its position). */
    private fun notePeak(buffer: ByteBuffer) {
        val b = buffer.duplicate().order(ByteOrder.nativeOrder())
        var peak = 0f
        if (float) {
            while (b.remaining() >= 4) peak = maxOf(peak, abs(b.float))
        } else {
            while (b.remaining() >= 2) peak = maxOf(peak, abs(b.short / 32768f))
        }
        slot().notePeak(peak)
    }

    /** Back to 16 bits with TPDF dither (two cheap xorshifts per sample), clipped. */
    private fun toShort(y: Double): Short {
        var n = noise
        n = n xor (n shl 13); n = n xor (n ushr 17); n = n xor (n shl 5)
        val r1 = (n ushr 8) * INV_24
        n = n xor (n shl 13); n = n xor (n ushr 17); n = n xor (n shl 5)
        val r2 = (n ushr 8) * INV_24
        noise = n
        val v = Math.round(y * 32768.0 + (r1 - r2)).toInt()
        return v.coerceIn(-32768, 32767).toShort()
    }

    private companion object {
        const val FADE_MS = 12
        const val INV_24 = 1.0 / (1 shl 24)
    }
}
