package com.perseusoft.homefi.ui.player

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.sin

class HairSimTest {
    private fun sim(): HairSim {
        val s = HairSim()
        for (i in 0 until 8) {
            s.shape(i, 10f, 58f, if (i < 4) -1f else 1f)
            s.root(i, 300f + 20 * i, 400f)
        }
        s.hang()
        return s
    }

    @Test
    fun hangsStillAndKeepsItsLength() {
        val s = sim()
        repeat(240) { s.advance(1.0 / 60) }
        val tipX = s.x[5]
        repeat(60) { s.advance(1.0 / 60) }
        assertEquals(tipX, s.x[5], 0.5f)
        var length = 0.0
        for (p in 1 until 6) length += Math.hypot((s.x[p] - s.x[p - 1]).toDouble(), (s.y[p] - s.y[p - 1]).toDouble())
        assertEquals(5 * 58.0, length, 6.0)
    }

    @Test
    fun theTipLagsTheHead() {
        val s = sim()
        val hz = 2.0
        var t = 0.0
        val dt = 1.0 / 60
        // let it settle into the motion, then find when the tip peaks after the root does
        var bestLag = 0.0
        var best = -1e9
        val tip = ArrayList<Double>()
        val rootPhase = ArrayList<Double>()
        while (t < 6.0) {
            val offset = 40 * sin(2 * PI * hz * t).toFloat()
            s.root(0, 300f + offset, 400f)
            s.advance(dt)
            if (t > 3.0) { tip += s.x[5].toDouble() - 300; rootPhase += 0.0 }
            t += dt
        }
        for (lag in 0..14) { // frames, under half a swing
            var corr = 0.0
            for (i in tip.indices) corr += tip[i] * 40 * sin(2 * PI * hz * ((3.0 + (i - lag) * dt)))
            if (corr > best) { best = corr; bestLag = lag * dt }
        }
        
        assertTrue("lag $bestLag s", bestLag in 0.08..0.12)
    }

    @Test
    fun theShouldersAreAFloor() {
        val s = sim()
        // whip the head down hard
        for (n in 0 until 60) {
            s.root(0, 300f, 400f + 5f * n)
            s.advance(1.0 / 60)
        }
        for (i in 1 until 6) assertTrue(s.y[i] <= 730.01f)
    }

    @Test
    fun affineInvertsWhatItDoes() {
        val m = Affine()
        m.translate(30.0, -12.0)
        m.rotate(25.0, 512.0, 896.0)
        m.scale(1.1, 0.9, 512.0, 896.0)
        val inv = Affine()
        assertTrue(m.invertTo(inv))
        val wx = m.x(100.0, 200.0)
        val wy = m.y(100.0, 200.0)
        assertEquals(100.0, inv.x(wx, wy), 1e-6)
        assertEquals(200.0, inv.y(wx, wy), 1e-6)
        assertTrue(abs(wx - 100.0) > 1)
    }
}
