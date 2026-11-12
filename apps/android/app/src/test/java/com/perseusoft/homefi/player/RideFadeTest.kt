package com.perseusoft.homefi.player

import com.perseusoft.homefi.player.exercise.rideFadeMs
import org.junit.Assert.assertEquals
import org.junit.Test

class RideFadeTest {
    @Test
    fun `the setting is the length when the song has room`() {
        assertEquals(3_000L, rideFadeMs(3f, durationMs = 240_000, positionMs = 120_000))
    }

    @Test
    fun `it ends before the file does`() {
        // 2 s left, 0.5 s guard: 1.5 s
        assertEquals(1_500L, rideFadeMs(3f, durationMs = 120_000, positionMs = 118_000))
    }

    @Test
    fun `no room or off is a cut`() {
        assertEquals(0L, rideFadeMs(3f, durationMs = 120_000, positionMs = 119_700))
        assertEquals(0L, rideFadeMs(0f, durationMs = 240_000, positionMs = 1_000))
        assertEquals(0L, rideFadeMs(Float.NaN, durationMs = 240_000, positionMs = 1_000))
        assertEquals(0L, rideFadeMs(3f, durationMs = -1, positionMs = 1_000))
    }

    @Test
    fun `a fade shorter than half a second is a cut`() {
        assertEquals(0L, rideFadeMs(3f, durationMs = 10_000, positionMs = 9_100))
        assertEquals(500L, rideFadeMs(3f, durationMs = 10_000, positionMs = 9_000))
    }
}
