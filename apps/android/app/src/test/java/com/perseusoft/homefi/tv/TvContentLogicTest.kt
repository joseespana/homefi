package com.perseusoft.homefi.tv

import com.perseusoft.homefi.data.DownloadDestination
import com.perseusoft.homefi.data.DownloadTarget
import com.perseusoft.homefi.data.LikedSongs
import com.perseusoft.homefi.data.RecentSearches
import com.perseusoft.homefi.data.TvStorage
import com.perseusoft.homefi.data.VolumeInfo
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class TvContentLogicTest {
    private val mb = 1024L * 1024L
    private val internal = File("/storage/emulated/0/Music/HomeFi")
    private val usbDir = File("/storage/ABCD-1234/HomeFi")

    @Test
    fun `low space follows the minimum free megabytes`() {
        assertTrue(TvStorage.isLow(499 * mb, 500))
        assertFalse(TvStorage.isLow(500 * mb, 500))
        assertTrue(TvStorage.isLow(0, 100))
    }

    @Test
    fun `download target decides the folder`() {
        assertEquals(DownloadDestination.Dir(internal, false), TvStorage.destination(DownloadTarget.INTERNAL, internal, usbDir))
        assertEquals(DownloadDestination.Dir(usbDir, true), TvStorage.destination(DownloadTarget.USB, internal, usbDir))
        assertEquals(DownloadDestination.UsbMissing, TvStorage.destination(DownloadTarget.USB, internal, null))
        assertEquals(DownloadDestination.Ask, TvStorage.destination(DownloadTarget.ASK, internal, usbDir))
        // nothing to ask about without a drive
        assertEquals(DownloadDestination.Dir(internal, false), TvStorage.destination(DownloadTarget.ASK, internal, null))
    }

    @Test
    fun `a path belongs to the longest volume root above it`() {
        val a = VolumeInfo("Internal", File("/storage/emulated/0"), false, 1, 2)
        val b = VolumeInfo("USB", File("/storage/ABCD-1234"), true, 1, 2)
        assertEquals(b, TvStorage.volumeOf("/storage/ABCD-1234/Music/a.flac", listOf(a, b)))
        assertEquals(a, TvStorage.volumeOf("/storage/emulated/0/Music/a.flac", listOf(a, b)))
        assertNull(TvStorage.volumeOf("/storage/ABCD-12345/a.flac", listOf(a, b)))
    }

    @Test
    fun `songs under a missing root are unavailable`() {
        val missing = TvStorage.missingRoots(listOf("/usb/a", "/here")) { it == "/here" }
        assertEquals(listOf("/usb/a"), missing)
        assertTrue(TvStorage.isUnavailable("/usb/a/x.flac", missing))
        assertFalse(TvStorage.isUnavailable("/usb/ab/x.flac", missing))
        assertFalse(TvStorage.isUnavailable("/here/x.flac", missing))
    }

    @Test
    fun `keyboard keys edit the text`() {
        assertEquals("ab", typed("a", "b"))
        assertEquals("a", typed("ab", "⌫"))
        assertEquals("", typed("abc", "✕"))
        assertEquals("a ", typed("a", " "))
        // no leading or doubled spaces
        assertEquals("", typed("", " "))
        assertEquals("a ", typed("a ", " "))
    }

    @Test
    fun `recent searches keep the newest first without repeats and never grow past the limit`() {
        var list = emptyList<String>()
        list = RecentSearches.remember(list, " Dreams ")
        list = RecentSearches.remember(list, "m83")
        list = RecentSearches.remember(list, "dreams")
        assertEquals(listOf("dreams", "m83"), list)
        assertEquals(list, RecentSearches.remember(list, "a"))
        repeat(20) { list = RecentSearches.remember(list, "query $it") }
        assertEquals(RecentSearches.MAX, list.size)
    }

    @Test
    fun `hearting twice takes the heart back`() {
        val once = LikedSongs.toggled(emptySet(), "x")
        assertEquals(setOf("x"), once)
        assertEquals(emptySet<String>(), LikedSongs.toggled(once, "x"))
    }

    @Test
    fun `sort pill cycles inside the orders a view offers`() {
        assertEquals(TvSort.ARTIST, TvLibraryTab.SONGS.nextSort(TvSort.TITLE))
        assertEquals(TvSort.TITLE, TvLibraryTab.SONGS.nextSort(TvSort.YEAR))
        // an order this view does not know falls back to its default
        assertEquals(TvSort.TITLE, TvLibraryTab.ARTISTS.sortOrDefault(TvSort.YEAR))
        assertTrue(TvLibraryTab.PLAYLISTS.sorts.isEmpty())
        assertEquals(TvSort.TITLE, TvLibraryTab.PLAYLISTS.nextSort(TvSort.TITLE))
    }
}
