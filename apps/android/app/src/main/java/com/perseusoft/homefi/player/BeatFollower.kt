package com.perseusoft.homefi.player

import kotlin.math.abs
import kotlin.math.exp
import kotlin.math.floor
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

// Follows the beat of the music for the mascot: a phase-locked beat counter fed
// by onsets (kicks), plus how loud the song is right now. Pure Kotlin (no
// Android, no clock of its own: every call carries the time) so it is unit
// tested with synthetic onset streams. The desktop's `lib/beat.ts` must keep
// the same constants and behaviour.

/** How hard the music is hitting, from the short-term vs long-term energy. */
enum class Section { QUIET, GROOVE, LOUD }

/** Tunable numbers, shared with the desktop. */
object BeatTuning {
    /** An onset counts as "on the grid" within this share of a beat from a predicted beat. */
    const val GRID_WINDOW = 0.30
    const val PHASE_GAIN = 0.20
    const val TEMPO_GAIN = 0.05
    const val MIN_BPM = 60.0
    const val MAX_BPM = 200.0
    const val DEFAULT_BPM = 120.0

    /** Below this the mascot free-runs instead of hitting hard on the beat. */
    const val LOCK_CONFIDENCE = 0.35
    /** Share of the confidence moved towards each onset (1 on the grid, 0 off it). */
    const val CONFIDENCE_RATE = 0.15
    /** For the confidence an onset only counts as a hit this close to a beat (share of a beat). */
    const val HIT_WINDOW = 0.15
    /** Confidence lost per second without any onset. */
    const val CONFIDENCE_DECAY_PER_S = 0.25

    /** Onset gaps (as tempo) fold into this range so half and double time agree. */
    const val FOLD_MIN_BPM = 80.0
    const val FOLD_MAX_BPM = 160.0
    const val MIN_GAP_S = 0.25
    const val MAX_GAP_S = 1.5
    const val GAPS_KEPT = 16
    const val GAPS_NEEDED = 6
    /** While unlocked the tempo only moves on real changes (share of the tempo). */
    const val TEMPO_HYSTERESIS = 0.05

    const val BEATS_PER_BAR = 4
    /** Slots of the bar keep the strongest onsets; older ones fade by this per onset. */
    const val SLOT_FADE = 0.98

    const val SHORT_ENERGY_S = 0.3
    const val LONG_ENERGY_S = 8.0
    /** Short/long energy ratio that maps to intensity 0 and 1. */
    const val RATIO_LOW = 0.6
    const val RATIO_HIGH = 1.6
    /** Below this short-term energy the intensity fades to 0 (silence is quiet whatever the song was). */
    const val ENERGY_FLOOR = 0.1

    const val QUIET_BELOW = 0.25
    const val LOUD_ABOVE = 0.7

    /** Intensity history, one sample every [SAMPLE_S], for spotting drops. */
    const val SAMPLE_S = 0.1
    const val HISTORY = 64
    const val DROP_JUMP = 0.35
    /** Intensity under this counts as a quiet stretch before a drop. */
    const val LOW_LEVEL = 0.45
    const val QUIET_BARS = 2.0
    const val DROP_COOLDOWN_BARS = 8.0
    /** Onsets at least this strong are accents (fist pumps). */
    const val ACCENT = 0.8
}

/**
 * Phase-locked beat counter. Call [update] once per analysis frame (about 30 Hz)
 * with the time, the energy of that frame and the strength of the onset found in
 * it (0 when none). Read the state from the same thread, or copy it out.
 */
class BeatFollower {
    /** Tempo in beats per minute. */
    var bpm = BeatTuning.DEFAULT_BPM
        private set

    /** Beats since the start, advancing with [bpm] and nudged by every onset on the grid. */
    var beat = 0.0
        private set

    /** 0..1: how many of the recent onsets landed on the grid (smoothed). */
    var confidence = 0.0
        private set

    /** 0..1: short-term energy against the long-term one. */
    var intensity = 0.0
        private set

    var section = Section.GROOVE
        private set

    /** Position of the current beat in the bar (0 is the downbeat). */
    var barBeat = 0
        private set

    /** Bars gone by; changes on every bar line. */
    var bars = 0
        private set

    /** Counts drops; changes when one happens. */
    var drops = 0
        private set

    /** Counts strong onsets on the grid; changes on every accent. */
    var accents = 0
        private set

    val locked get() = confidence >= BeatTuning.LOCK_CONFIDENCE

    private var time = Double.NaN
    private var shortEnergy = 0.0
    private var longEnergy = 0.0
    private var energySeen = false

    private val gaps = DoubleArray(BeatTuning.GAPS_KEPT)
    private val sorted = DoubleArray(BeatTuning.GAPS_KEPT)
    private var gapCount = 0
    private var gapNext = 0
    private var lastOnset = Double.NaN

    private val slots = DoubleArray(BeatTuning.BEATS_PER_BAR)
    private var downSlot = 0
    private var lastBeatIndex = 0

    private val history = DoubleArray(BeatTuning.HISTORY)
    private var historyFilled = 0
    private var historyNext = 0
    private var sampleAt = 0.0
    private var lowSince = Double.NaN
    private var lastStretch = 0.0
    private var stretchEnded = Double.NEGATIVE_INFINITY
    private var lastDrop = Double.NEGATIVE_INFINITY

    /** Starts over for a new track; the tempo is kept as a first guess. */
    fun reset() {
        beat = 0.0
        confidence = 0.0
        intensity = 0.0
        section = Section.GROOVE
        barBeat = 0
        time = Double.NaN
        energySeen = false
        shortEnergy = 0.0
        longEnergy = 0.0
        gapCount = 0
        gapNext = 0
        lastOnset = Double.NaN
        slots.fill(0.0)
        downSlot = 0
        lastBeatIndex = 0
        historyFilled = 0
        historyNext = 0
        sampleAt = 0.0
        lowSince = Double.NaN
        lastStretch = 0.0
        stretchEnded = Double.NEGATIVE_INFINITY
        lastDrop = Double.NEGATIVE_INFINITY
        // bars, drops and accents keep counting: watchers compare them with what they saw
    }

    /** Advances to [timeS] seconds; [energy] is 0..1 loudness, [onset] the kick strength 0..1 (0 = none). */
    fun update(timeS: Double, energy: Double, onset: Double = 0.0) {
        val dt = if (time.isNaN()) 0.0 else (timeS - time).coerceIn(0.0, MAX_STEP_S)
        time = timeS
        beat += dt * bpm / 60.0
        if (onset > 0.0) onOnset(timeS, onset)
        if (!lastOnset.isNaN() && timeS - lastOnset > 1.0) {
            confidence = max(0.0, confidence - dt * BeatTuning.CONFIDENCE_DECAY_PER_S)
        }
        followBars()
        followEnergy(timeS, dt, energy)
    }

    private fun onOnset(timeS: Double, strength: Double) {
        collectGap(timeS)
        val nearest = beat.roundToInt()
        val error = nearest - beat // beats we are behind: positive means the onset came early
        if (abs(error) <= BeatTuning.GRID_WINDOW) {
            beat += BeatTuning.PHASE_GAIN * error
            bpm = (bpm * (1.0 + BeatTuning.TEMPO_GAIN * error)).coerceIn(BeatTuning.MIN_BPM, BeatTuning.MAX_BPM)
            val hit = if (abs(error) <= BeatTuning.HIT_WINDOW) 1.0 else 0.0
            confidence += (hit - confidence) * BeatTuning.CONFIDENCE_RATE
            val slot = nearest.mod(BeatTuning.BEATS_PER_BAR)
            for (i in slots.indices) slots[i] *= BeatTuning.SLOT_FADE
            slots[slot] += strength
            var best = 0
            for (i in 1 until slots.size) if (slots[i] > slots[best]) best = i
            downSlot = best
            if (strength > BeatTuning.ACCENT) accents++
        } else {
            confidence -= confidence * BeatTuning.CONFIDENCE_RATE
        }
        lastOnset = timeS
    }

    /** Median tempo from the gaps between onsets; it seeds the tempo until the beat locks. */
    private fun collectGap(timeS: Double) {
        if (!lastOnset.isNaN()) {
            val gap = timeS - lastOnset
            if (gap in BeatTuning.MIN_GAP_S..BeatTuning.MAX_GAP_S) {
                var tempo = 60.0 / gap
                while (tempo < BeatTuning.FOLD_MIN_BPM) tempo *= 2
                while (tempo > BeatTuning.FOLD_MAX_BPM) tempo /= 2
                gaps[gapNext] = tempo
                gapNext = (gapNext + 1) % gaps.size
                gapCount = min(gapCount + 1, gaps.size)
                if (gapCount >= BeatTuning.GAPS_NEEDED && !locked) {
                    for (i in 0 until gapCount) sorted[i] = gaps[i]
                    java.util.Arrays.sort(sorted, 0, gapCount)
                    val median = Math.round(sorted[gapCount shr 1]).toDouble()
                    if (abs(median - bpm) / bpm > BeatTuning.TEMPO_HYSTERESIS) bpm = median
                }
            }
        }
    }

    private fun followBars() {
        val index = floor(beat).toInt()
        barBeat = (index - downSlot).mod(BeatTuning.BEATS_PER_BAR)
        if (index != lastBeatIndex) {
            lastBeatIndex = index
            if (barBeat == 0) bars++
        }
    }

    private fun followEnergy(timeS: Double, dt: Double, energy: Double) {
        if (!energySeen) {
            energySeen = true
            shortEnergy = energy
            longEnergy = energy
        } else {
            shortEnergy += (energy - shortEnergy) * (1 - exp(-dt / BeatTuning.SHORT_ENERGY_S))
            longEnergy += (energy - longEnergy) * (1 - exp(-dt / BeatTuning.LONG_ENERGY_S))
        }
        val ratio = shortEnergy / max(longEnergy, 1e-3)
        val relative = ((ratio - BeatTuning.RATIO_LOW) / (BeatTuning.RATIO_HIGH - BeatTuning.RATIO_LOW)).coerceIn(0.0, 1.0)
        intensity = relative * min(1.0, shortEnergy / BeatTuning.ENERGY_FLOOR)
        section = when {
            intensity < BeatTuning.QUIET_BELOW -> Section.QUIET
            intensity > BeatTuning.LOUD_ABOVE -> Section.LOUD
            else -> Section.GROOVE
        }
        followDrops(timeS)
    }

    private fun barSeconds() = BeatTuning.BEATS_PER_BAR * 60.0 / bpm

    /** A drop: the intensity jumps by [BeatTuning.DROP_JUMP] within a bar, after a quiet stretch. */
    private fun followDrops(timeS: Double) {
        if (intensity < BeatTuning.LOW_LEVEL) {
            if (lowSince.isNaN()) lowSince = timeS
        } else if (!lowSince.isNaN()) {
            lastStretch = timeS - lowSince
            stretchEnded = timeS
            lowSince = Double.NaN
        }
        if (timeS >= sampleAt) {
            sampleAt = timeS + BeatTuning.SAMPLE_S
            history[historyNext] = intensity
            historyNext = (historyNext + 1) % history.size
            historyFilled = min(historyFilled + 1, history.size)
        }
        val bar = barSeconds()
        val window = min(historyFilled, max(1, (bar / BeatTuning.SAMPLE_S).toInt()))
        var floor = intensity
        for (i in 1..window) floor = min(floor, history[(historyNext - i).mod(history.size)])
        if (intensity - floor <= BeatTuning.DROP_JUMP) return
        val quietBefore = when {
            !lowSince.isNaN() -> timeS - lowSince
            timeS - stretchEnded <= bar -> lastStretch
            else -> 0.0
        }
        if (quietBefore >= BeatTuning.QUIET_BARS * bar && timeS - lastDrop >= BeatTuning.DROP_COOLDOWN_BARS * bar) {
            lastDrop = timeS
            drops++
        }
    }

    private companion object {
        /** A gap in the calls longer than this is a pause, not time passing in the music. */
        const val MAX_STEP_S = 0.25
    }
}

/**
 * Finds kicks in the low band: a peak of the energy flux (how much it rose since
 * the last frame) well above its usual size. Returns the strength 0..1, or 0.
 */
class OnsetDetector {
    private var previous = Double.NaN
    private var fluxAverage = 0.0
    private var fluxPeak = MIN_PEAK
    private var lastAt = Double.NEGATIVE_INFINITY

    fun reset() {
        previous = Double.NaN
        fluxAverage = 0.0
        fluxPeak = MIN_PEAK
        lastAt = Double.NEGATIVE_INFINITY
    }

    fun feed(timeS: Double, low: Double): Double {
        val flux = if (previous.isNaN()) 0.0 else max(0.0, low - previous)
        previous = low
        val threshold = max(MIN_FLUX, fluxAverage * RELATIVE)
        fluxAverage += (flux - fluxAverage) * AVERAGE_RATE
        fluxPeak = max(flux, fluxPeak * PEAK_DECAY)
        if (flux < threshold || timeS - lastAt < BeatTuning.MIN_GAP_S) return 0.0
        lastAt = timeS
        return (flux / fluxPeak).coerceIn(0.05, 1.0)
    }

    private companion object {
        const val MIN_FLUX = 0.04
        const val RELATIVE = 2.2
        const val AVERAGE_RATE = 0.1
        const val PEAK_DECAY = 0.995
        const val MIN_PEAK = 0.1
    }
}
