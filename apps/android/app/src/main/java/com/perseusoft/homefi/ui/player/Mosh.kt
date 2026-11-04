package com.perseusoft.homefi.ui.player

import com.perseusoft.homefi.player.BeatTuning
import com.perseusoft.homefi.player.Listening
import com.perseusoft.homefi.player.Section
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.floor
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.sin
import kotlin.random.Random

// Mosh mode: the choreography for heavy music, driven by the beat follower
// ([Listening]: the beat, the section and the drops). Headbanging on the beat,
// gestures picked on the bar lines, a jump now and then and a windmill when the
// song drops. The desktop's `lib/mosh.ts` must keep the same numbers.

/** How wildly it headbangs: scales every amplitude; the windmill only comes with NORMAL and WILD. */
enum class Headbang(val scale: Double) { GENTLE(0.6), NORMAL(1.0), WILD(1.4) }

/** The beat-related choices from Settings, plus whether motion is reduced. */
data class BeatOptions(
    /** Lock the dance to the real beat; off keeps the free tempo from the estimate. */
    val follow: Boolean = true,
    val headbang: Headbang = Headbang.NORMAL,
    /** Halves the amplitudes and drops the windmill. */
    val reduce: Boolean = false,
)

/** Numbers of the mosh choreography. */
private object MoshTuning {
    /** Head tilt on the beat, in degrees, at full strength. */
    const val HEAD_DEG = 32.0
    const val TORSO_SHARE = 0.3
    /** The torso follows the head this late, in beats. */
    const val TORSO_DELAY = 0.1
    const val KNEE = 12.0
    const val BASE_AMPLITUDE = 0.4
    const val AMPLITUDE_SPAN = 0.6
    const val REDUCED = 0.5
    /** Power of the nod on the beat: sharper when locked to a real beat. */
    const val HIT_LOCKED = 2.0
    const val HIT_FREE = 1.0
    const val QUIET_ARMS = 28.0
    const val QUIET_LIDS = 0.5
    const val CALM_RATE_PER_S = 3.0
    const val JUMP_HEIGHT = 90.0
    const val JUMP_BARS_MIN = 8
    const val JUMP_BARS_SPAN = 9
    const val WIND_BEATS_MIN = 4
    const val WIND_BEATS_SPAN = 5
    const val WIND_DEG = 40.0
    const val WIND_REACH = 50.0
    const val WIND_DEPTH = 24.0
    const val WIND_FADE_BEATS = 0.5
    const val SHAKE_BEATS = 0.4
    /** Shake of the mascot box on the drop beat, in art units (about 3 px). */
    const val SHAKE = 8.0
    const val PUNCH_BEATS = 0.6
    const val GESTURE_BARS_MIN = 1
    const val GESTURE_BARS_SPAN = 3
}

private enum class Gesture { HORNS_BOTH, HORNS_ONE, GUITAR, FIST }

/** Decides what the mascot does in mosh mode, one frame at a time. */
internal class MoshDirector(private val random: Random = Random.Default) {
    private var gesture = Gesture.HORNS_BOTH
    private var gestureBars = 1
    private var seenBar = Int.MIN_VALUE
    private var seenDrops = -1
    private var seenAccents = -1
    private var barsToJump = MoshTuning.JUMP_BARS_MIN
    private var jumpAt = Double.NEGATIVE_INFINITY
    private var windAt = Double.NEGATIVE_INFINITY
    private var windBeats = 0.0
    private var dropAt = Double.NEGATIVE_INFINITY
    private var punchAt = Double.NEGATIVE_INFINITY
    private var calm = 0.0

    fun reset() {
        seenBar = Int.MIN_VALUE
        seenDrops = -1
        seenAccents = -1
        jumpAt = Double.NEGATIVE_INFINITY
        windAt = Double.NEGATIVE_INFINITY
        dropAt = Double.NEGATIVE_INFINITY
        punchAt = Double.NEGATIVE_INFINITY
        calm = 0.0
    }

    /** 1 on the beat, 0 half way: the nod, sharper when the beat is real. */
    private fun hit(b: Double, locked: Boolean) =
        (0.5 + 0.5 * cos(2 * PI * b)).pow(if (locked) MoshTuning.HIT_LOCKED else MoshTuning.HIT_FREE)

    private fun pick() {
        var next: Gesture
        do next = Gesture.entries[random.nextInt(Gesture.entries.size)] while (next == gesture)
        gesture = next
        gestureBars = MoshTuning.GESTURE_BARS_MIN + random.nextInt(MoshTuning.GESTURE_BARS_SPAN)
    }

    private fun onBar(beat: Double, loud: Boolean) {
        if (--gestureBars <= 0) pick()
        if (--barsToJump <= 0 && loud) {
            jumpAt = beat
            barsToJump = MoshTuning.JUMP_BARS_MIN + random.nextInt(MoshTuning.JUMP_BARS_SPAN)
        }
    }

    /**
     * The pose [dt] seconds on, at [beat] beats. [locked] says the beat is the
     * real one; otherwise it free-runs and the hits are soft.
     */
    fun pose(dt: Double, beat: Double, sound: Listening, locked: Boolean, options: BeatOptions): Pose {
        val reduce = options.reduce
        val amplitude = (MoshTuning.BASE_AMPLITUDE + MoshTuning.AMPLITUDE_SPAN * sound.intensity) *
            options.headbang.scale * (if (reduce) MoshTuning.REDUCED else 1.0)
        val quiet = sound.section == Section.QUIET
        val loud = sound.section == Section.LOUD
        calm += ((if (quiet) 1.0 else 0.0) - calm) * min(1.0, dt * MoshTuning.CALM_RATE_PER_S)

        // events: bar lines, drops, accents (the counters only move up while one track plays)
        val bar = if (locked) sound.bar else floor(beat / BeatTuning.BEATS_PER_BAR).toInt()
        if (bar != seenBar) {
            if (seenBar != Int.MIN_VALUE) onBar(beat, loud)
            seenBar = bar
        }
        if (seenDrops in 0 until sound.drops) {
            dropAt = beat
            if (!reduce && options.headbang != Headbang.GENTLE) {
                windAt = beat
                windBeats = (MoshTuning.WIND_BEATS_MIN + random.nextInt(MoshTuning.WIND_BEATS_SPAN)).toDouble()
            }
        }
        seenDrops = sound.drops
        if (seenAccents in 0 until sound.accents && locked) punchAt = beat
        seenAccents = sound.accents

        // the nod: every beat, or every two while it is quiet
        val nod = hit(beat, locked) * (1 - calm) + hit(beat / 2, locked) * 0.7 * calm
        val torsoNod = hit(beat - MoshTuning.TORSO_DELAY, locked) * (1 - calm) + hit(beat / 2 - MoshTuning.TORSO_DELAY / 2, locked) * 0.7 * calm
        var head = amplitude * MoshTuning.HEAD_DEG * nod - 4 * amplitude
        var headX = 0.0
        var headY = 0.0
        var rot = MoshTuning.TORSO_SHARE * MoshTuning.HEAD_DEG * amplitude * torsoNod
        var x = 4 * amplitude * sin(PI * beat)
        var y = MoshTuning.KNEE * amplitude * torsoNod
        val sy = 1 - 0.025 * amplitude * torsoNod
        val legs = 16 + 10 * amplitude * torsoNod
        var liftL = 0.0
        var liftR = 0.0

        // windmill: the head goes round in circles, the hair whips
        val spun = beat - windAt
        if (spun in 0.0..windBeats) {
            val fade = min(1.0, min(spun, windBeats - spun) / MoshTuning.WIND_FADE_BEATS)
            val phase = 2 * PI * spun
            head += (MoshTuning.WIND_DEG * sin(phase) * amplitude - head) * fade
            headX = fade * MoshTuning.WIND_REACH * sin(phase)
            headY = fade * MoshTuning.WIND_DEPTH * cos(phase)
            rot += fade * 8 * sin(phase)
        }
        // the drop beat shakes the whole mascot a little
        val shook = beat - dropAt
        if (shook in 0.0..MoshTuning.SHAKE_BEATS) {
            val fade = 1 - shook / MoshTuning.SHAKE_BEATS
            val k = if (reduce) MoshTuning.REDUCED else 1.0
            x += MoshTuning.SHAKE * k * fade * sin(2 * PI * 6 * shook)
            y += MoshTuning.SHAKE * 0.75 * k * fade * cos(2 * PI * 6 * shook)
        }
        // a jump on a downbeat
        val flown = beat - jumpAt
        if (flown in 0.0..1.0) {
            val air = sin(PI * flown)
            y -= MoshTuning.JUMP_HEIGHT * air * (if (reduce) MoshTuning.REDUCED else 1.0)
            liftL = 18 * air
            liftR = 18 * air
        }

        // arms: the gesture of this stretch of bars, easing to rest while it is quiet
        var armL = 30.0
        var armR = 30.0
        var guitar = 0.0
        var strum = 0.0
        var fret = 0.0
        val raise = 12 * amplitude * torsoNod
        when (gesture) {
            Gesture.HORNS_BOTH -> { armL = 150 + raise; armR = 150 + raise }
            Gesture.HORNS_ONE -> { armL = 30 + 10 * sin(PI * beat); armR = 155 + raise }
            Gesture.FIST -> { armL = 30.0; armR = 100 + 70 * (0.5 + 0.5 * cos(2 * PI * beat)) }
            Gesture.GUITAR -> {
                // strum down on every beat; the fretting hand slides up the neck on alternate bars
                guitar = 1.0
                strum = cos(2 * PI * beat)
                val slide = (floor(beat / BeatTuning.BEATS_PER_BAR).toInt() and 1).toDouble()
                val into = (beat / BeatTuning.BEATS_PER_BAR).let { it - floor(it) }
                val eased = min(1.0, into * 6).let { it * it * (3 - 2 * it) }
                fret = if (slide > 0) eased else 1 - eased
            }
        }
        val punch = 1 - (beat - punchAt) / MoshTuning.PUNCH_BEATS
        if (punch > 0 && punch <= 1 && gesture != Gesture.GUITAR) armR = armR + (175 - armR) * punch
        if (calm > 0) {
            armL += (MoshTuning.QUIET_ARMS - armL) * calm
            armR += (MoshTuning.QUIET_ARMS - armR) * calm
            guitar *= 1 - calm
        }
        return Pose(
            x = x, y = y, rot = rot, sy = sy,
            armL = if (guitar > 0) -60.0 else armL,
            armR = if (guitar > 0) 150.0 else armR,
            legL = legs, legR = legs, liftL = liftL, liftR = liftR,
            guitar = guitar, strum = strum, fret = fret,
            head = head, headX = headX, headY = headY,
            lids = MoshTuning.QUIET_LIDS * calm,
        )
    }
}
