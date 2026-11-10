package com.perseusoft.homefi.tv

import android.view.InputDevice
import com.perseusoft.homefi.R
import com.perseusoft.homefi.data.AppSettings
import com.perseusoft.homefi.data.TvSettings
import com.perseusoft.homefi.data.TvStorage
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class TvSettingsModelTest {
    private val defaults = AppSettings()
    private val rows get() = TvSettingsCategory.entries.flatMap(::settingsRows)

    @Test
    fun `every row has its own id (a row listed on two pages is the same row)`() {
        rows.groupBy { it.id }.forEach { (id, same) ->
            assertTrue("$id is two different rows", same.all { it === same.first() })
        }
    }

    @Test
    fun `every AppSettings tv field has a row, but the ones with nothing to set`() {
        // `channels` is the home-screen row of a later phase: no feature behind it, so no row yet
        val noRow = setOf("channels", "Companion")
        val fields = TvSettings::class.java.declaredFields.map { it.name }.filter { it !in noRow && !it.startsWith("$") }
        val ids = rows.map { it.id }.toSet()
        fields.forEach { assertTrue("tv.$it has no settings row", "tv.$it" in ids) }
    }

    @Test
    fun `a switch writes the value it reads`() {
        rows.filterIsInstance<TvSettingRow.Switch>().forEach { row ->
            val on = row.set(defaults, true)
            val off = row.set(defaults, false)
            assertTrue(row.id, row.get(on))
            assertFalse(row.id, row.get(off))
        }
    }

    @Test
    fun `a choice shows the option it was set to`() {
        rows.filterIsInstance<TvSettingRow.Choice>().forEach { row ->
            row.options.indices.forEach { i ->
                assertEquals("${row.id} option $i", i, row.selected(row.select(defaults, i)))
            }
        }
    }

    @Test
    fun `a choice ignores an index outside its options`() {
        rows.filterIsInstance<TvSettingRow.Choice>().forEach { row ->
            assertEquals(row.id, defaults, row.select(defaults, row.options.size + 5))
        }
    }

    @Test
    fun `the player controls never hiding is the first option and reads Never`() {
        val row = rows.filterIsInstance<TvSettingRow.Choice>().first { it.id == "tv.controlsHideSeconds" }
        assertEquals(TvLabel.Res(R.string.tv_set_never), row.options.first())
        // the default (0 = never) selects it, and picking it stores 0
        assertEquals(0, row.selected(defaults))
        assertEquals(0, row.select(defaults.copy(tv = defaults.tv.copy(controlsHideSeconds = 8)), 0).tv.controlsHideSeconds)
    }

    @Test
    fun `new TV defaults show as the matching options`() {
        val target = rows.filterIsInstance<TvSettingRow.Choice>().first { it.id == "tv.downloadTarget" }
        assertEquals(TvLabel.Res(R.string.tv_dl_ask), target.options[target.selected(defaults)])
        val auto = rows.filterIsInstance<TvSettingRow.Switch>().first { it.id == "tv.partyAutoApprove" }
        assertTrue(auto.get(defaults))
    }

    @Test
    fun `a stored value between two steps shows the nearest step`() {
        val row = rows.filterIsInstance<TvSettingRow.Choice>().first { it.id == "tv.minFreeMb" }
        val odd = defaults.copy(tv = defaults.tv.copy(minFreeMb = 480))
        assertEquals(TvLabel.Res(R.string.tv_set_mb, listOf(500)), row.options[row.selected(odd)])
        assertEquals(2, nearestIndex(listOf(1, 5, 9), 8))
        assertEquals(0, nearestIndex(emptyList<Int>(), 3))
    }

    @Test
    fun `the three-state signal chain row writes both settings`() {
        val row = rows.filterIsInstance<TvSettingRow.Choice>().first { it.id == "signalChain" }
        val off = row.select(defaults, 2)
        assertFalse(off.signalChain)
        val detailed = row.select(off, 1)
        assertTrue(detailed.signalChain)
        assertNotEquals(off.signalStyle, detailed.signalStyle)
    }

    @Test
    fun `a text row writes what it reads`() {
        rows.filterIsInstance<TvSettingRow.Text>().forEach { row ->
            assertEquals(row.id, "abc", row.get(row.set(defaults, "abc")))
        }
    }

    @Test
    fun `left and right wrap around the options`() {
        assertEquals(0, cycleIndex(2, 3, 1))
        assertEquals(2, cycleIndex(0, 3, -1))
        assertEquals(1, cycleIndex(0, 3, 1))
        assertEquals(0, cycleIndex(0, 0, 1))
    }

    @Test
    fun `only the AI page has no rows of its own`() {
        TvSettingsCategory.entries.forEach { assertEquals(it.name, it == TvSettingsCategory.AI, settingsRows(it).isEmpty()) }
    }

    @Test
    fun `a switch is enabled by default and the dependent ones follow their parent`() {
        val style = rows.filterIsInstance<TvSettingRow.Choice>().first { it.id == "tv.ambientStyle" }
        assertTrue(style.enabled(defaults))
        assertFalse(style.enabled(defaults.copy(tv = defaults.tv.copy(ambientSeconds = 0))))
        val forget = rows.filterIsInstance<TvSettingRow.Action>().first { it.id == "tv.libraryVolumes" }
        assertFalse(forget.enabled(defaults))
        assertTrue(forget.enabled(defaults.copy(tv = defaults.tv.copy(libraryVolumes = listOf("/storage/ABCD/HomeFi")))))
    }

    // ---------------------------------------------------------------- mouse and USB decisions

    @Test
    fun `a real mouse unlocks DJ mode, a remote, a keyboard or a virtual device does not`() {
        assertTrue(isMouseDevice(InputDevice.SOURCE_MOUSE, isVirtual = false))
        assertTrue(isMouseDevice(InputDevice.SOURCE_MOUSE or InputDevice.SOURCE_KEYBOARD, isVirtual = false))
        assertFalse(isMouseDevice(InputDevice.SOURCE_KEYBOARD or InputDevice.SOURCE_DPAD, isVirtual = false))
        assertFalse(isMouseDevice(InputDevice.SOURCE_TOUCHSCREEN, isVirtual = false))
        assertFalse(isMouseDevice(InputDevice.SOURCE_MOUSE, isVirtual = true))
    }

    @Test
    fun `a download to a USB drive asks for All files access every time it is missing`() {
        assertTrue(TvStorage.needsAccessQuestion(onUsb = true, sharedAccess = false, declinedThisQueue = false))
        assertFalse(TvStorage.needsAccessQuestion(onUsb = true, sharedAccess = true, declinedThisQueue = false))
        assertFalse(TvStorage.needsAccessQuestion(onUsb = false, sharedAccess = false, declinedThisQueue = false))
        // a no ("use the app folder") holds for the rest of that queue only
        assertFalse(TvStorage.needsAccessQuestion(onUsb = true, sharedAccess = false, declinedThisQueue = true))
    }
}
