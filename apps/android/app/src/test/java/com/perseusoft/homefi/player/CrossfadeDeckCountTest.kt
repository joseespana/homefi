package com.perseusoft.homefi.player

import org.junit.Assert.assertEquals
import org.junit.Test

class CrossfadeDeckCountTest {
    @Test
    fun `gapless playback keeps one deck`() {
        assertEquals(1, crossfadeDeckCount(crossfadeOn = false, lowPowerFallback = true, lowRam = false))
        assertEquals(1, crossfadeDeckCount(crossfadeOn = false, lowPowerFallback = false, lowRam = true))
    }

    @Test
    fun `crossfade builds the second deck`() {
        assertEquals(2, crossfadeDeckCount(crossfadeOn = true, lowPowerFallback = true, lowRam = false))
        assertEquals(2, crossfadeDeckCount(crossfadeOn = true, lowPowerFallback = false, lowRam = true))
    }

    @Test
    fun `a low-memory phone with the low-power fallback never builds it`() {
        assertEquals(1, crossfadeDeckCount(crossfadeOn = true, lowPowerFallback = true, lowRam = true))
    }
}
