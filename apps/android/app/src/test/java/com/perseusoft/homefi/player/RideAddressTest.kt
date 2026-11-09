package com.perseusoft.homefi.player

import com.perseusoft.homefi.player.exercise.asTypedAddress
import com.perseusoft.homefi.player.exercise.parseRideAddress
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class RideAddressTest {
    @Test
    fun `host and port`() {
        val e = parseRideAddress("192.168.1.20:5000", 47801)!!
        assertEquals("192.168.1.20", e.host)
        assertEquals(5000.toUShort(), e.port)
        assertEquals("http://192.168.1.20:5000/ride.json", e.rideUrl)
    }

    @Test
    fun `default port and the phone's full url`() {
        assertEquals(47801.toUShort(), parseRideAddress(" 10.0.0.5 ", 47801)!!.port)
        assertEquals("10.0.0.5", parseRideAddress("http://10.0.0.5:47802/", 47801)!!.host)
    }

    @Test
    fun `rubbish is not an address`() {
        assertNull(parseRideAddress("", 47801))
        assertNull(parseRideAddress("a b", 47801))
        assertNull(parseRideAddress("1.2.3.4:99999", 47801))
        assertNull(parseRideAddress("1.2.3.4:x", 47801))
    }

    @Test
    fun `typed form`() = assertEquals("192.168.1.5:47801", "http://192.168.1.5:47801/".asTypedAddress())
}
