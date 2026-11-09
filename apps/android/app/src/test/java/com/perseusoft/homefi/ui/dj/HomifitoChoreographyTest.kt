package com.perseusoft.homefi.ui.dj

import com.perseusoft.homefi.core.SfxKind
import com.perseusoft.homefi.player.dj.DjDeckUi
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class HomifitoChoreographyTest {
    private fun input(motion: StageMotion, phase: Float = 0.1f, playing: Boolean = true, reaction: Reaction? = null) = StageInput(
        motion, phase, 0, booleanArrayOf(playing, false), booleanArrayOf(false, false), null, 0.5f, null, reaction, 0.5f, 1.0,
    )

    @Test
    fun `the dip peaks just after the beat and eases back`() {
        assertEquals(0f, beatDip(0f), 1e-6f)
        assertEquals(1f, beatDip(0.12f), 1e-6f)
        assertTrue(beatDip(0.5f) < beatDip(0.2f))
    }

    @Test
    fun `off is still and subtle sways less than full`() {
        assertEquals(0f, aim(input(StageMotion.OFF)).bodyY, 0f)
        assertTrue(aim(input(StageMotion.SUBTLE)).bodyY < aim(input(StageMotion.FULL)).bodyY)
    }

    @Test
    fun `a pad reaction makes him happy, and only the show moves the hands`() {
        assertTrue(aim(input(StageMotion.FULL, reaction = Reaction.HANDS_UP)).happy)
        val subtle = aim(input(StageMotion.SUBTLE, reaction = Reaction.HANDS_UP))
        assertTrue(subtle.happy)
        assertTrue(subtle.handL.y > 200f)
        assertTrue(aim(input(StageMotion.FULL, reaction = Reaction.HANDS_UP)).handL.y < 100f)
    }

    @Test
    fun `sounds map to reactions`() {
        assertEquals(Reaction.HANDS_UP, Reaction.of(SfxKind.AIR_HORN))
        assertEquals(Reaction.JUMP, Reaction.of(SfxKind.DROP))
        assertEquals(Reaction.SCRATCH, Reaction.of(SfxKind.REWIND))
    }

    @Test
    fun `cueing is the idle loaded deck while the other plays`() {
        val playing = DjDeckUi(loaded = true, playing = true)
        val idle = DjDeckUi(loaded = true)
        assertEquals(1, cueingDeck(listOf(playing, idle)))
        assertNull(cueingDeck(listOf(idle, idle)))
        assertEquals(0, masterDeck(listOf(playing, playing), 0.2f))
    }

    @Test
    fun `scratching is a paused playhead that moves a little`() {
        val paused = DjDeckUi(loaded = true)
        assertTrue(isScratching(paused, 40))
        assertFalse(isScratching(paused, 5000))
        assertFalse(isScratching(paused.copy(playing = true), 40))
    }

    @Test
    fun `the strip shrinks and hides with the height`() {
        assertNull(homifitoStripHeightDp(400, big = true))
        val tightBig = homifitoStripHeightDp(640, big = true)!!
        assertEquals(homifitoStripHeightDp(640, big = false)!!, tightBig)
        assertTrue(homifitoStripHeightDp(900, big = true)!! > homifitoStripHeightDp(900, big = false)!!)
    }

    @Test
    fun `an instant step jumps to the target`() {
        val t = aim(input(StageMotion.FULL, reaction = Reaction.JUMP))
        assertEquals(t.handL, approach(REST_POSE, t, Float.POSITIVE_INFINITY).handL)
    }
}
