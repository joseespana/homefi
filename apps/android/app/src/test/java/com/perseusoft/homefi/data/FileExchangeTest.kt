package com.perseusoft.homefi.data

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Test
import java.io.ByteArrayInputStream
import java.io.InputStream

class FileExchangeTest {
    @Test
    fun `a small file is read whole`() {
        val bytes = "hello".toByteArray()
        assertArrayEquals(bytes, FileExchange.readAtMost(ByteArrayInputStream(bytes), 10))
    }

    @Test
    fun `a big file stops at the limit`() {
        val bytes = ByteArray(100_000) { it.toByte() }
        val read = FileExchange.readAtMost(ByteArrayInputStream(bytes), 20_001)
        assertEquals(20_001, read.size)
        assertArrayEquals(bytes.copyOf(20_001), read)
    }

    @Test
    fun `a stream that hands out a few bytes at a time is still read to the end`() {
        val bytes = ByteArray(30_000) { (it * 7).toByte() }
        val trickle = object : InputStream() {
            val inner = ByteArrayInputStream(bytes)
            override fun read(): Int = inner.read()
            override fun read(b: ByteArray, off: Int, len: Int): Int = inner.read(b, off, minOf(len, 3))
        }
        assertArrayEquals(bytes, FileExchange.readAtMost(trickle, 1_000_000))
    }
}
