package com.perseusoft.homefi.ui.exercise

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test

class RideChoicesTest {
    @Test fun genreToggleAddsAndRemoves() {
        assertEquals(setOf("rock"), toggleGenre(emptySet(), "rock"))
        assertEquals(setOf("pop"), toggleGenre(setOf("rock", "pop"), "rock"))
    }

    @Test fun decadeToggleKeepsOrder() {
        assertEquals(listOf<UShort>(1980u, 2000u), toggleDecade(listOf(2000u), 1980u))
        assertEquals(listOf<UShort>(2000u), toggleDecade(listOf<UShort>(1980u, 2000u), 1980u))
        assertEquals(emptyList<UShort>(), toggleDecade(listOf<UShort>(1990u), 1990u))
    }

    @Test fun decadeLabels() {
        assertEquals("80s", decadeLabel(1980u))
        assertEquals("2000s", decadeLabel(2000u))
        assertEquals("2020s", decadeLabel(2020u))
    }

    @Test fun genreTokens() {
        assertNotNull(Genre.ofToken("hip-hop"))
        assertNull(Genre.ofToken("polka"))
    }
}
