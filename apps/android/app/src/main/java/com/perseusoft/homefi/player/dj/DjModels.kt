package com.perseusoft.homefi.player.dj

import androidx.annotation.StringRes
import androidx.compose.runtime.Immutable
import com.perseusoft.homefi.core.DjAnalysis
import kotlin.math.max

/** Decks of the simple DJ (the core has the same count). */
const val DJ_DECKS = 2

/** Hot cues per deck on the phone (the core allows eight). */
const val DJ_HOT_CUES = 4

/** Loop lengths offered, in beats. */
val DJ_LOOP_BEATS = listOf(4, 8)

/** Lowest a band can be turned down with its slider; the kill switch goes below it. */
const val DJ_EQ_MIN_DB = -24f

/** Bars the overview waveform is reduced to, whatever the song's length. */
const val DJ_OVERVIEW_BARS = 480

private const val SLICE_BYTES = 4

fun deckLetter(deck: Int): Char = 'A' + deck

/** Where a deck's song comes from. */
sealed interface DjSource {
    val title: String
    val artist: String

    data class Local(val path: String, override val title: String, override val artist: String) : DjSource
    data class Tidal(val id: String, override val title: String, override val artist: String) : DjSource
}

/** One deck's channel EQ (dB per band, kill switches, the bipolar filter -1..1). */
@Immutable
data class DjEq(
    val highDb: Float = 0f,
    val midDb: Float = 0f,
    val lowDb: Float = 0f,
    val killHigh: Boolean = false,
    val killMid: Boolean = false,
    val killLow: Boolean = false,
    val filter: Float = 0f,
)

/** The overview waveform reduced to [DJ_OVERVIEW_BARS] bars; every array is `0..1`. */
@Immutable
class DjOverview(val peak: FloatArray, val low: FloatArray, val mid: FloatArray, val high: FloatArray) {
    val bars: Int get() = peak.size

    companion object {
        /** Max-pools the core's slices (4 bytes each: peak, low, mid, high) into [bars] bars. */
        fun from(analysis: DjAnalysis, bars: Int = DJ_OVERVIEW_BARS): DjOverview? {
            val slices = analysis.overview.size / SLICE_BYTES
            if (slices == 0) return null
            val n = minOf(bars, slices)
            val out = Array(SLICE_BYTES) { FloatArray(n) }
            for (bar in 0 until n) {
                val from = bar * slices / n
                val to = max(from + 1, (bar + 1) * slices / n)
                for (s in from until to) for (band in 0 until SLICE_BYTES) {
                    val v = (analysis.overview[s * SLICE_BYTES + band].toInt() and 0xFF) / 255f
                    if (v > out[band][bar]) out[band][bar] = v
                }
            }
            return DjOverview(out[0], out[1], out[2], out[3])
        }
    }
}

@Immutable
data class DjDeckUi(
    val loaded: Boolean = false,
    /** Fetching a TIDAL song or opening the file. */
    val loading: Boolean = false,
    /** Finding the beat. */
    val analyzing: Boolean = false,
    val path: String? = null,
    val title: String = "",
    val artist: String = "",
    val analysis: DjAnalysis? = null,
    val overview: DjOverview? = null,
    val playing: Boolean = false,
    val positionMs: Long = 0,
    val durationMs: Long = 0,
    /** Playback speed (1 = the song's own). */
    val speed: Float = 1f,
    /** This deck follows the other one's tempo and beat. */
    val synced: Boolean = false,
    val cueMs: Long = 0,
    val hotCues: List<Long?> = List(DJ_HOT_CUES) { null },
    val loopBeats: Int? = null,
    val loopStartMs: Long = 0,
    val loopEndMs: Long = 0,
    val eq: DjEq = DjEq(),
    val volume: Float = 1f,
    val trimDb: Float = 0f,
    /** A short explanation to show on the deck (a string resource), or null. */
    @param:StringRes val note: Int? = null,
) {
    /** BPM at the current speed, 0 when unknown. */
    val bpm: Double get() = (analysis?.bpm ?: 0.0) * speed

    /** Percent away from the song's own tempo. */
    val tempoPct: Float get() = (speed - 1f) * 100f

    val hasGrid: Boolean get() = analysis?.grid != null
}

/** The crossfader, master volume and the mix being made. */
@Immutable
data class DjMixerUi(
    /** 0 = all deck A, 1 = all deck B. */
    val crossfader: Float = 0f,
    val master: Float = 1f,
    val mix: DjMixUi? = null,
)

@Immutable
data class DjMixUi(val to: Int, val progress: Float, val beats: Int)

/** A message for a snackbar; [id] makes the same text show again. */
@Immutable
data class DjNotice(@param:StringRes val res: Int, val args: List<String> = emptyList(), val id: Long)

fun formatClock(ms: Long): String {
    val total = (ms / 1000).coerceAtLeast(0)
    return "%d:%02d".format(total / 60, total % 60)
}
