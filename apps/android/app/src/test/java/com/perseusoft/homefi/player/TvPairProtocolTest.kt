package com.perseusoft.homefi.player

import com.google.zxing.BarcodeFormat
import com.google.zxing.EncodeHintType
import com.google.zxing.qrcode.QRCodeWriter
import com.perseusoft.homefi.pair.PairReply
import com.perseusoft.homefi.pair.PairRequest
import com.perseusoft.homefi.pair.QrDecoder
import com.perseusoft.homefi.pair.TvPairLink
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test

class TvPairProtocolTest {
    @Test
    fun `the link round trips`() {
        val link = TvPairLink("192.168.1.30", 47810, "482913")
        assertEquals("homefi://tv?host=192.168.1.30&port=47810&code=482913", link.toLink())
        assertEquals(link, TvPairLink.parse(link.toLink()))
    }

    @Test
    fun `rubbish is not a pairing link`() {
        assertNull(TvPairLink.parse(null))
        assertNull(TvPairLink.parse("https://example.com/?host=1.2.3.4&port=1&code=123456"))
        assertNull(TvPairLink.parse("homefi://join?host=192.168.1.30&port=47810&code=482913"))
        assertNull(TvPairLink.parse("homefi://tv?host=192.168.1.30&port=99999&code=482913"))
        assertNull(TvPairLink.parse("homefi://tv?host=bad host&port=1&code=482913"))
        assertNull(TvPairLink.parse("homefi://tv?host=192.168.1.30&port=47810"))
    }

    @Test
    fun `request and reply json`() {
        val request = PairRequest("123456", null, "http://192.168.1.20:47801/", "Pixel")
        assertEquals(request, PairRequest.parse(request.toJson()))
        assertNull(PairRequest.parse("""{"rideUrl":"x"}""")) // neither code nor token
        assertNull(PairRequest.parse("not json"))
        assertEquals(PairReply("TV", "abc"), PairReply.parse(PairReply("TV", "abc").toJson()))
        assertNull(PairReply.parse("""{"ok":false}"""))
    }

    @Test
    fun `a rendered qr is read back from luminance`() {
        val text = TvPairLink("10.0.0.7", 47810, "000123").toLink()
        val matrix = QRCodeWriter().encode(text, BarcodeFormat.QR_CODE, 300, 300, mapOf(EncodeHintType.MARGIN to 4))
        val luma = ByteArray(matrix.width * matrix.height) { i -> if (matrix.get(i % matrix.width, i / matrix.width)) 0 else 255.toByte() }
        val read = QrDecoder().decode(luma, matrix.width, matrix.width, matrix.height)
        assertNotNull(read)
        assertEquals(text, read)
    }
}
