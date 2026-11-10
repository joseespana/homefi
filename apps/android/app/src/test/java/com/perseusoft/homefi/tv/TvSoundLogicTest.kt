package com.perseusoft.homefi.tv

import com.perseusoft.homefi.core.EqBand
import com.perseusoft.homefi.core.EqBandKind
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class TvSoundLogicTest {
    private fun band(hz: Float) = EqBand(kind = EqBandKind.PEAK, freqHz = hz, gainDb = 0f, q = 1f, enabled = true)

    // list order differs from frequency order on purpose
    private val bands = listOf(band(1_000f), band(100f), band(8_000f))

    @Test
    fun rightMovesToTheNextHigherFrequencyAndStopsAtTheEnd() {
        assertEquals(0, neighbourBand(bands, 1, 1))
        assertEquals(2, neighbourBand(bands, 0, 1))
        assertEquals(2, neighbourBand(bands, 2, 1))
    }

    @Test
    fun leftMovesDownAndStopsAtTheStart() {
        assertEquals(0, neighbourBand(bands, 2, -1))
        assertEquals(1, neighbourBand(bands, 1, -1))
    }

    @Test
    fun noSelectionStartsAtTheLowestBandAndNoBandsGivesNull() {
        assertEquals(1, neighbourBand(bands, null, 1))
        assertNull(neighbourBand(emptyList(), null, 1))
    }

    @Test
    fun gainStepsByTheSettingAndStaysInRange() {
        assertEquals(1f, steppedGain(0f, 1, 1f), 0f)
        assertEquals(-1.5f, steppedGain(0f, -3, 0.5f), 0f)
        assertEquals(24f, steppedGain(23.5f, 1, 3f), 0f)
        assertEquals(-24f, steppedGain(-23f, -1, 3f), 0f)
        assertEquals(0.3f, steppedGain(0.1f, 1, 0.2f), 0f)
    }
}
