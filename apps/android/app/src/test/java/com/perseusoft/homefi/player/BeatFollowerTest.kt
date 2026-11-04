package com.perseusoft.homefi.player

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.math.abs
import kotlin.math.floor
import kotlin.random.Random

/** The follower against synthetic onset streams, at the ~30 Hz the analysis runs at. */
class BeatFollowerTest {
    private val frame = 1.0 / 30

    /** Onset times of a steady [bpm] from [from] to [to] seconds, each moved by up to [jitter] seconds. */
    private fun steady(bpm: Double, from: Double, to: Double, jitter: Double, random: Random): List<Double> {
        val gap = 60.0 / bpm
        val times = ArrayList<Double>()
        var t = from
        while (t < to) {
            times += t + (random.nextDouble() * 2 - 1) * jitter
            t += gap
        }
        return times
    }

    /** Plays [onsets] into [f] from [from] to [to]; [energy] gives the loudness at a time. */
    private fun play(f: BeatFollower, onsets: List<Double>, from: Double, to: Double, energy: (Double) -> Double = { 0.4 }) {
        var next = 0
        var t = from
        while (t < to) {
            var strength = 0.0
            while (next < onsets.size && onsets[next] <= t) {
                if (onsets[next] > t - frame) strength = 0.7
                next++
            }
            f.update(t, energy(t), strength)
            t += frame
        }
    }

    @Test
    fun locksOntoSteady120WithJitter() {
        val f = BeatFollower()
        val onsets = steady(120.0, 0.0, 30.0, 0.02, Random(1))
        play(f, onsets, 0.0, 30.0)
        assertEquals(120.0, f.bpm, 4.0)
        assertTrue("confidence ${f.confidence}", f.confidence > 0.6)
        assertTrue(f.locked)
        // the predicted beat lands on the onsets: the phase error at an onset is small
        val last = onsets.last { it < 29.0 }
        val g = BeatFollower()
        play(g, onsets, 0.0, last)
        val phase = g.beat - floor(g.beat + 0.5)
        assertTrue("phase $phase", abs(phase) < 0.2)
    }

    @Test
    fun followsATempoChange() {
        val f = BeatFollower()
        val random = Random(2)
        val onsets = steady(120.0, 0.0, 20.0, 0.015, random) + steady(140.0, 20.0, 50.0, 0.015, random)
        play(f, onsets, 0.0, 50.0)
        assertEquals(140.0, f.bpm, 7.0)
        assertTrue("confidence ${f.confidence}", f.confidence > 0.35)
    }

    @Test
    fun ignoresOnsetsOffTheGrid() {
        val f = BeatFollower()
        val random = Random(3)
        val onsets = steady(120.0, 0.0, 30.0, 0.01, random)
        play(f, onsets, 0.0, 15.0)
        val before = f.bpm
        // loud noise halfway between the beats must not drag the tempo or the phase
        val noise = onsets.filter { it in 15.0..30.0 }.map { it + 0.25 }
        play(f, (onsets.filter { it > 15.0 } + noise).sorted(), 15.0, 30.0)
        assertEquals(before, f.bpm, 6.0)
        assertTrue("confidence ${f.confidence}", f.confidence > 0.35)
    }

    @Test
    fun pureNoiseNeverLocks() {
        val f = BeatFollower()
        val random = Random(4)
        val times = ArrayList<Double>()
        var t = 0.0
        while (t < 40.0) {
            t += 0.25 + random.nextDouble() * 0.9 // irregular gaps
            times += t
        }
        play(f, times, 0.0, 40.0)
        assertTrue("confidence ${f.confidence}", f.confidence < 0.6)
    }

    @Test
    fun silenceLosesTheLock() {
        val f = BeatFollower()
        play(f, steady(120.0, 0.0, 20.0, 0.01, Random(5)), 0.0, 20.0)
        assertTrue(f.locked)
        play(f, emptyList(), 20.0, 30.0) { 0.0 }
        assertTrue("confidence ${f.confidence}", f.confidence < BeatTuning.LOCK_CONFIDENCE)
        assertEquals(Section.QUIET, f.section)
        assertEquals(0.0, f.intensity, 0.05)
    }

    @Test
    fun downbeatIsTheStrongestOnsetOfTheBar() {
        val f = BeatFollower()
        val gap = 0.5 // 120 bpm; every fourth kick is the accent
        var kick = 0
        var t = 0.0
        var accentAt = Double.NaN
        val barBeats = ArrayList<Int>()
        while (t < 30.0) {
            val due = t >= kick * gap
            val strong = kick % 4 == 0
            f.update(t, 0.4, if (due) (if (strong) 1.0 else 0.4) else 0.0)
            if (due) {
                if (strong) accentAt = t
                kick++
            }
            // a little after each accent (late enough for the beat to have passed it) it is the bar's first beat
            if (!accentAt.isNaN() && t >= accentAt + 0.2) {
                if (kick > 20) barBeats += f.barBeat
                accentAt = Double.NaN
            }
            t += frame
        }
        assertTrue("barBeats $barBeats", barBeats.isNotEmpty() && barBeats.all { it == 0 })
        assertTrue("bars ${f.bars}", f.bars in 12..16)
    }

    @Test
    fun sectionsFollowTheEnergy() {
        val f = BeatFollower()
        // quiet for a while, then a loud part
        play(f, emptyList(), 0.0, 20.0) { 0.12 }
        play(f, emptyList(), 20.0, 21.0) { 0.8 }
        assertEquals(Section.LOUD, f.section)
        assertTrue(f.intensity > BeatTuning.LOUD_ABOVE)
    }

    @Test
    fun aDropAfterAQuietStretchIsFoundOnceAndThrottled() {
        val f = BeatFollower()
        val onsets = steady(120.0, 0.0, 60.0, 0.01, Random(6))
        // 12 s quiet (6 bars), a loud jump, quiet again, and another jump within 8 bars
        val energy = { t: Double ->
            when {
                t < 12.0 -> 0.05
                t < 20.0 -> 0.8
                t < 26.0 -> 0.05
                t < 30.0 -> 0.8
                else -> 0.4
            }
        }
        play(f, onsets, 0.0, 14.0, energy)
        assertEquals("first drop", 1, f.drops)
        play(f, onsets, 14.0, 60.0, energy)
        // the second jump at 26 s is inside the cool-down of 8 bars (16 s) from 12 s
        assertEquals("throttled", 1, f.drops)
    }

    @Test
    fun noDropWithoutAQuietStretch() {
        val f = BeatFollower()
        val energy = { t: Double -> if (t < 1.0) 0.3 else 0.8 }
        play(f, steady(120.0, 0.0, 10.0, 0.01, Random(7)), 0.0, 10.0, energy)
        assertEquals(0, f.drops)
    }

    @Test
    fun onsetDetectorFindsKicksAndNotAHum() {
        val d = OnsetDetector()
        var found = 0
        var t = 0.0
        while (t < 10.0) {
            val phase = (t % 0.5)
            val low = 0.3 + (if (phase < 0.2) 0.5 * (1 - phase / 0.2) else 0.0)
            if (d.feed(t, low) > 0) found++
            t += frame
        }
        assertTrue("found $found", found in 17..21)
        val hum = OnsetDetector()
        var wrong = 0
        t = 0.0
        while (t < 10.0) {
            if (hum.feed(t, 0.5) > 0) wrong++
            t += frame
        }
        assertEquals(0, wrong)
    }
}
