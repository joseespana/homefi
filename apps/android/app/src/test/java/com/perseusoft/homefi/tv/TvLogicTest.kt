package com.perseusoft.homefi.tv

import android.view.KeyEvent
import com.perseusoft.homefi.data.PlayerLayout
import com.perseusoft.homefi.data.TvDjEntry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class TvLogicTest {
    @Test
    fun `controls hidden - arrows and OK are the player's, panel and controls keys too`() {
        assertEquals(PlayerKeyAction.PLAY_PAUSE, playerKeyAction(KeyEvent.KEYCODE_DPAD_CENTER, controlsVisible = false))
        assertEquals(PlayerKeyAction.LEFT, playerKeyAction(KeyEvent.KEYCODE_DPAD_LEFT, controlsVisible = false))
        assertEquals(PlayerKeyAction.RIGHT, playerKeyAction(KeyEvent.KEYCODE_DPAD_RIGHT, controlsVisible = false))
        assertEquals(PlayerKeyAction.CYCLE_PANEL, playerKeyAction(KeyEvent.KEYCODE_DPAD_UP, controlsVisible = false))
        assertEquals(PlayerKeyAction.SHOW_CONTROLS, playerKeyAction(KeyEvent.KEYCODE_DPAD_DOWN, controlsVisible = false))
    }

    @Test
    fun `controls visible - the focus system owns the arrows and OK`() {
        listOf(KeyEvent.KEYCODE_DPAD_CENTER, KeyEvent.KEYCODE_DPAD_LEFT, KeyEvent.KEYCODE_DPAD_UP, KeyEvent.KEYCODE_DPAD_DOWN).forEach {
            assertEquals(PlayerKeyAction.NONE, playerKeyAction(it, controlsVisible = true))
        }
    }

    @Test
    fun `fast forward and rewind seek in every state`() {
        for (visible in listOf(true, false)) {
            assertEquals(PlayerKeyAction.SEEK_FORWARD, playerKeyAction(KeyEvent.KEYCODE_MEDIA_FAST_FORWARD, visible))
            assertEquals(PlayerKeyAction.SEEK_BACK, playerKeyAction(KeyEvent.KEYCODE_MEDIA_REWIND, visible))
        }
    }

    @Test
    fun `a held arrow seeks on the first repeat and then every few`() {
        assertFalse(holdSeekStep(0))
        assertTrue(holdSeekStep(1))
        assertFalse(holdSeekStep(2))
        assertTrue(holdSeekStep(1 + HOLD_SEEK_EVERY))
    }

    @Test
    fun `seek stays inside the song`() {
        assertEquals(0L, seekTarget(5_000, 200_000, -10))
        assertEquals(200_000L, seekTarget(195_000, 200_000, 10))
        assertEquals(25_000L, seekTarget(15_000, 200_000, 10))
        assertEquals(0L, seekTarget(3_000, 0, -10))
    }

    @Test
    fun `ambient mode swallows the waking key and its release`() {
        val idle = TvIdle()
        assertFalse(idle.onKey(KeyEvent.KEYCODE_DPAD_LEFT, isDown = true))
        idle.enterAmbient()
        assertTrue(idle.onKey(KeyEvent.KEYCODE_DPAD_LEFT, isDown = true))
        assertFalse(idle.ambient.value)
        assertTrue(idle.onKey(KeyEvent.KEYCODE_DPAD_LEFT, isDown = false))
        // the next press is a normal one
        assertFalse(idle.onKey(KeyEvent.KEYCODE_DPAD_LEFT, isDown = true))
        assertFalse(idle.onKey(KeyEvent.KEYCODE_DPAD_LEFT, isDown = false))
    }

    @Test
    fun `drawer sections follow the account and the DJ setting`() {
        val signedOut = visibleSections(signedIn = false, djEntry = TvDjEntry.AUTO)
        assertFalse(TvSection.TIDAL in signedOut)
        assertTrue(TvSection.TIDAL in visibleSections(true, TvDjEntry.AUTO))
        assertFalse(TvSection.DJ in visibleSections(true, TvDjEntry.OFF))
        assertEquals(TvSection.HOME, signedOut.first())
        assertEquals(TvSection.SETTINGS, signedOut.last())
    }

    @Test
    fun `DJ mode needs a mouse unless forced on`() {
        assertFalse(djAvailable(TvDjEntry.AUTO, mouse = false))
        assertTrue(djAvailable(TvDjEntry.AUTO, mouse = true))
        assertTrue(djAvailable(TvDjEntry.ON, mouse = false))
        assertFalse(djAvailable(TvDjEntry.OFF, mouse = true))
    }

    @Test
    fun `only readable skins reach the TV`() {
        assertEquals(PlayerLayout.CLASSIC, tvSkinLayout(PlayerLayout.LUNA))
        assertEquals(PlayerLayout.CLASSIC, tvSkinLayout(PlayerLayout.IMMERSIVE))
        assertEquals(PlayerLayout.VINYL, tvSkinLayout(PlayerLayout.VINYL))
    }

    @Test
    fun `login code groups and link host`() {
        assertEquals(listOf("ABCD", "EFGH"), codeGroups("ABCD-EFGH"))
        assertEquals(listOf("KHANH"), codeGroups("KHANH"))
        assertEquals("link.tidal.com", linkHost("https://link.tidal.com/KHANH"))
        assertEquals("link.tidal.com", linkHost("link.tidal.com/KHANH"))
    }

    @Test
    fun `focus scale follows the motion level`() {
        assertEquals(1f, focusScale(1.08f, motionEnabled = false, motionMoves = false), 0f)
        assertEquals(1.08f, focusScale(1.08f, motionEnabled = true, motionMoves = true), 0f)
        assertEquals(1.04f, focusScale(1.08f, motionEnabled = true, motionMoves = false), 0.0001f)
    }

    @Test
    fun `the ambient group walks through every spot`() {
        var spot = 0
        val seen = HashSet<Int>()
        repeat(DRIFT_SPOTS.size) { seen += spot; spot = nextDriftSpot(spot) }
        assertEquals(DRIFT_SPOTS.size, seen.size)
        assertEquals(0, spot)
    }
}
