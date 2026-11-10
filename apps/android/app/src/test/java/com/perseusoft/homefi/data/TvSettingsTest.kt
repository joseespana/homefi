package com.perseusoft.homefi.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class TvSettingsTest {
    @Test
    fun defaultsAreThePlansDefaults() {
        val d = TvSettings()
        assertEquals(UiForm.AUTO, d.uiForm)
        assertEquals(60, d.ambientSeconds)
        assertEquals(AmbientStyle.COVER, d.ambientStyle)
        assertTrue(d.keepAwakeWhilePlaying)
        assertEquals(5, d.awakeAfterPauseMinutes)
        assertEquals(0, d.controlsHideSeconds)
        assertEquals(10, d.dpadSeekSeconds)
        assertEquals(PlayerPanel.LYRICS, d.playerPanel)
        assertTrue(d.playerLayoutColours)
        assertTrue(d.safeMargins)
        assertEquals(1.0f, d.textScale, 0f)
        assertEquals(TvDjEntry.AUTO, d.djEntry)
        assertTrue(d.voiceInput)
        assertEquals(DownloadTarget.ASK, d.downloadTarget)
        assertEquals(500, d.minFreeMb)
        assertEquals(1024, d.streamCacheMb)
        assertTrue(d.partyAutoApprove)
        assertEquals(1.0f, d.eqStepDb, 0f)
        assertTrue(d.libraryVolumes.isEmpty())
    }

    @Test
    fun clampBringsEveryNumberInsideItsRange() {
        val low = TvSettings(
            ambientSeconds = -5, awakeAfterPauseMinutes = 0, controlsHideSeconds = -3, dpadSeekSeconds = 1,
            textScale = 0.1f, minFreeMb = 1, streamCacheMb = 1, eqStepDb = 0f,
        ).clamped()
        assertEquals(0, low.ambientSeconds)
        assertEquals(1, low.awakeAfterPauseMinutes)
        assertEquals(0, low.controlsHideSeconds)
        assertEquals(5, low.dpadSeekSeconds)
        assertEquals(0.9f, low.textScale, 0f)
        assertEquals(100, low.minFreeMb)
        assertEquals(256, low.streamCacheMb)
        assertEquals(0.5f, low.eqStepDb, 0f)

        val high = TvSettings(
            ambientSeconds = 9999, awakeAfterPauseMinutes = 99, controlsHideSeconds = 99, dpadSeekSeconds = 999,
            textScale = 9f, minFreeMb = 999_999, streamCacheMb = 999_999, eqStepDb = 50f,
        ).clamped()
        assertEquals(600, high.ambientSeconds)
        assertEquals(30, high.awakeAfterPauseMinutes)
        assertEquals(15, high.controlsHideSeconds)
        assertEquals(60, high.dpadSeekSeconds)
        assertEquals(1.3f, high.textScale, 0f)
        assertEquals(5000, high.minFreeMb)
        assertEquals(4096, high.streamCacheMb)
        assertEquals(3.0f, high.eqStepDb, 0f)
    }

    @Test
    fun roundTripsThroughJson() {
        val s = TvSettings(
            uiForm = UiForm.TV, ambientSeconds = 120, ambientStyle = AmbientStyle.CLOCK, playerPanel = PlayerPanel.QUEUE,
            djEntry = TvDjEntry.ON, downloadTarget = DownloadTarget.USB, textScale = 1.2f, minFreeMb = 800,
            libraryVolumes = listOf("/storage/ABCD-1234/Music"), partyAutoApprove = false, voiceInput = false, eqStepDb = 2.5f,
        )
        assertEquals(s, TvSettings.fromJson(s.toJson()))
    }

    @Test
    fun oldPartialOrBrokenJsonGivesDefaults() {
        assertEquals(TvSettings(), TvSettings.fromJson(null))
        assertEquals(TvSettings(), TvSettings.fromJson(""))
        assertEquals(TvSettings(), TvSettings.fromJson("{not json"))
        assertEquals(TvSettings(), TvSettings.fromJson("[1,2]"))
        // only what is present changes; unknown keys are ignored
        val partial = TvSettings.fromJson("""{"ambientSeconds":90,"future":{"x":1}}""")
        assertEquals(TvSettings(ambientSeconds = 90), partial)
    }

    @Test
    fun wrongTypesAndUnknownNamesFallBackPerField() {
        val s = TvSettings.fromJson(
            """{"uiForm":"hologram","ambientSeconds":"soon","voiceInput":"yes","djEntry":"on","dpadSeekSeconds":7777,
                "textScale":null,"libraryVolumes":["/a"," ",5,"/a"]}""",
        )
        assertEquals(UiForm.AUTO, s.uiForm)
        assertEquals(60, s.ambientSeconds)
        assertTrue(s.voiceInput)
        assertEquals(TvDjEntry.ON, s.djEntry) // enum names are case-insensitive
        assertEquals(60, s.dpadSeekSeconds) // clamped
        assertEquals(1.0f, s.textScale, 0f)
        assertEquals(listOf("/a"), s.libraryVolumes)
    }
}
