package com.perseusoft.homefi.tv

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class TvKeyboardTest {
    @Test
    fun `both pages fill the same grid, 9 keys wide and 5 rows high`() {
        KeyPage.entries.forEach { page ->
            assertEquals(page.name, 45, keyboardKeys(page, shift = false).size)
            assertEquals(page.name, 45, keyboardKeys(page, shift = true).size)
        }
    }

    @Test
    fun `the letters page has the alphabet, the digits and the punctuation of everyday text`() {
        val keys = keyboardKeys(KeyPage.LETTERS, shift = false)
        ('a'..'z').forEach { assertTrue("$it", it.toString() in keys) }
        ('0'..'9').forEach { assertTrue("$it", it.toString() in keys) }
        listOf(".", ",", "-", "_", "@", "!", "?", "'", ":").forEach { assertTrue(it, it in keys) }
    }

    @Test
    fun `the symbols page has what keys, links and Spanish need`() {
        val keys = keyboardKeys(KeyPage.SYMBOLS, shift = false)
        listOf("/", "\\", "=", "+", "#", "\$", "%", "&", "*", "(", ")", "[", "]", "{", "}", "<", ">", "\"", ";", "¿", "¡", "ñ", "é", "ü").forEach {
            assertTrue(it, it in keys)
        }
    }

    @Test
    fun `no key is on both pages, so every character has one place`() {
        val letters = keyboardKeys(KeyPage.LETTERS, false).toSet()
        val symbols = keyboardKeys(KeyPage.SYMBOLS, false)
        assertTrue(symbols.none { it in letters })
        assertEquals(symbols.size, symbols.toSet().size)
    }

    @Test
    fun `shift gives capitals, accented ones too, and leaves symbols alone`() {
        val letters = keyboardKeys(KeyPage.LETTERS, shift = true)
        ('A'..'Z').forEach { assertTrue("$it", it.toString() in letters) }
        assertFalse("a" in letters)
        assertTrue("5" in letters && "@" in letters)
        val symbols = keyboardKeys(KeyPage.SYMBOLS, shift = true)
        assertTrue("Ñ" in symbols && "É" in symbols && "/" in symbols)
    }

    @Test
    fun `typing a key keeps its case, so an API key comes out as typed`() {
        var text = ""
        listOf("s", "k", "-", "A", "b", "9", "_", "Z").forEach { text = typed(text, it) }
        assertEquals("sk-Ab9_Z", text)
        assertEquals("sk-Ab9_", typed(text, "⌫"))
        assertEquals("", typed(text, "✕"))
    }

    @Test
    fun `a secret shows one dot per character`() {
        assertEquals("•••••", masked("sk-ab"))
        assertEquals("", masked(""))
    }
}
