package com.perseusoft.homefi.ui.player

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.DrawScope
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.sin
import kotlin.math.sqrt

// The mascot's long hair: a few strands from the roof, each a chain of points
// with verlet physics (gravity, damping, a spring back to its resting shape),
// so it lags the head and whips round in a windmill. Drawn as smooth tapered
// shapes. Everything is allocated once; a frame allocates nothing. The desktop's
// `lib/hair.ts` must keep the same numbers.

/** How much hair the mascot has: long in mosh mode only (auto), always, or none. */
enum class HairStyle { AUTO, LONG, SHORT }

/** Where the head turns when it nods (the base of the house). */
internal val HEAD_PIVOT = Offset(512f, 790f)

private const val STRANDS = 8
private const val POINTS = 6

/** Fixed physics step, and the most steps a frame may take. */
private const val STEP_S = 1.0 / 120
private const val MAX_STEPS = 4
private const val GRAVITY = 2600f // units per second squared
/** Share of the speed kept every 1/60 s. */
private const val DAMPING_PER_FRAME = 0.86
/** Share of the way back to its resting shape that a point goes every step (less towards the tips). */
private const val STIFFNESS_ROOT = 0.055f
private const val STIFFNESS_TIP = 0.018f
private const val SHOULDER_Y = 730f
private const val SHOULDER_FRICTION = 0.5f
private const val ROOF_BASE_X = 275.9f
private const val ROOF_BASE_Y = 466.2f
private const val ROOF_APEX_X = 481.9f
private const val ROOF_APEX_Y = 288f
private const val HAIR_WIDTH = 40f
private const val HAIR_TIP = 0.2f

/** One strand's shape: where it grows on the roof (0 base .. 1 apex), which way it flares, how long. */
private class Strand(val t: Float, val flareDeg: Float, val segment: Float, val front: Boolean)

private val LAYOUT = arrayOf(
    Strand(0.18f, 26f, 62f, false),
    Strand(0.50f, 14f, 60f, false),
    Strand(0.08f, 16f, 58f, true),
    Strand(0.28f, 9f, 52f, true),
)

/** A 2D affine transform without allocating: `x' = a x + c y + e`, `y' = b x + d y + f`. */
internal class Affine {
    var a = 1.0; var b = 0.0; var c = 0.0; var d = 1.0; var e = 0.0; var f = 0.0

    fun identity() {
        a = 1.0; b = 0.0; c = 0.0; d = 1.0; e = 0.0; f = 0.0
    }

    /** this = this * translate (the new step applies first, like a canvas). */
    fun translate(tx: Double, ty: Double) {
        e += a * tx + c * ty
        f += b * tx + d * ty
    }

    fun rotate(degrees: Double, px: Double, py: Double) {
        translate(px, py)
        val r = degrees * PI / 180
        val cs = cos(r)
        val sn = sin(r)
        val na = a * cs + c * sn
        val nb = b * cs + d * sn
        val nc = -a * sn + c * cs
        val nd = -b * sn + d * cs
        a = na; b = nb; c = nc; d = nd
        translate(-px, -py)
    }

    fun scale(sx: Double, sy: Double, px: Double, py: Double) {
        translate(px, py)
        a *= sx; b *= sx; c *= sy; d *= sy
        translate(-px, -py)
    }

    fun x(x: Double, y: Double) = a * x + c * y + e
    fun y(x: Double, y: Double) = b * x + d * y + f

    /** Writes the inverse into [out]; false when the transform is flat. */
    fun invertTo(out: Affine): Boolean {
        val det = a * d - b * c
        if (abs(det) < 1e-9) return false
        out.a = d / det
        out.b = -b / det
        out.c = -c / det
        out.d = a / det
        out.e = -(out.a * e + out.c * f)
        out.f = -(out.b * e + out.d * f)
        return true
    }
}

/**
 * The strands' physics, in world units. Set the roots with [root], then call
 * [advance] once a frame. Points live in flat arrays: strand `s`, point `p` is
 * at `s * POINTS + p`.
 */
internal class HairSim {
    val x = FloatArray(STRANDS * POINTS)
    val y = FloatArray(STRANDS * POINTS)
    private val prevX = FloatArray(STRANDS * POINTS)
    private val prevY = FloatArray(STRANDS * POINTS)
    private val rootX = FloatArray(STRANDS)
    private val rootY = FloatArray(STRANDS)
    private val startX = FloatArray(STRANDS)
    private val startY = FloatArray(STRANDS)
    private val restX = FloatArray(STRANDS)
    private val restY = FloatArray(STRANDS)
    private val length = FloatArray(STRANDS)
    private val damping = DAMPING_PER_FRAME.pow(STEP_S * 60).toFloat()
    private var pending = 0.0

    fun shape(strand: Int, flareDeg: Float, segment: Float, outward: Float) {
        val r = flareDeg * PI.toFloat() / 180
        restX[strand] = outward * sin(r)
        restY[strand] = cos(r)
        length[strand] = segment
    }

    /** Where strand [s] grows from, this frame. */
    fun root(s: Int, wx: Float, wy: Float) {
        rootX[s] = wx
        rootY[s] = wy
    }

    /** Lets every strand hang straight from its root, at rest. */
    fun hang() {
        for (s in 0 until STRANDS) {
            val base = s * POINTS
            x[base] = rootX[s]; y[base] = rootY[s]
            for (p in 1 until POINTS) {
                x[base + p] = x[base + p - 1] + restX[s] * length[s]
                y[base + p] = y[base + p - 1] + restY[s] * length[s]
            }
            startX[s] = rootX[s]; startY[s] = rootY[s]
        }
        System.arraycopy(x, 0, prevX, 0, x.size)
        System.arraycopy(y, 0, prevY, 0, y.size)
        pending = 0.0
    }

    /** Moves the physics on by [dt] seconds in fixed steps (at most [MAX_STEPS]; the rest is dropped). */
    fun advance(dt: Double) {
        pending += min(dt, STEP_S * MAX_STEPS * 2)
        var steps = (pending / STEP_S).toInt()
        if (steps > MAX_STEPS) {
            steps = MAX_STEPS
            pending = 0.0
        } else {
            pending -= steps * STEP_S
        }
        for (i in 1..steps) substep(i.toFloat() / steps)
        if (steps > 0) {
            System.arraycopy(rootX, 0, startX, 0, STRANDS)
            System.arraycopy(rootY, 0, startY, 0, STRANDS)
        }
    }

    private fun substep(share: Float) {
        val g = (GRAVITY * STEP_S * STEP_S).toFloat()
        for (s in 0 until STRANDS) {
            val base = s * POINTS
            // the root follows the head, moving a little further each step
            x[base] = startX[s] + (rootX[s] - startX[s]) * share
            y[base] = startY[s] + (rootY[s] - startY[s]) * share
            prevX[base] = x[base]
            prevY[base] = y[base]
            val seg = length[s]
            for (p in 1 until POINTS) {
                val i = base + p
                val vx = (x[i] - prevX[i]) * damping
                val vy = (y[i] - prevY[i]) * damping
                prevX[i] = x[i]
                prevY[i] = y[i]
                x[i] += vx
                y[i] += vy + g
                // back towards the resting shape, stiffer near the head
                val pull = STIFFNESS_ROOT + (STIFFNESS_TIP - STIFFNESS_ROOT) * (p - 1f) / (POINTS - 2f)
                x[i] += (x[i - 1] + restX[s] * seg - x[i]) * pull
                y[i] += (y[i - 1] + restY[s] * seg - y[i]) * pull
                // keep the segment its length (only the child moves)
                val dx = x[i] - x[i - 1]
                val dy = y[i] - y[i - 1]
                val d = sqrt(dx * dx + dy * dy)
                if (d > 1e-4f) {
                    val fix = seg / d
                    x[i] = x[i - 1] + dx * fix
                    y[i] = y[i - 1] + dy * fix
                }
                // the shoulders are a floor: hair lies on them
                if (y[i] > SHOULDER_Y) {
                    y[i] = SHOULDER_Y
                    prevX[i] = x[i] - (x[i] - prevX[i]) * SHOULDER_FRICTION
                    prevY[i] = y[i]
                }
            }
        }
    }
}

/**
 * The hair on the mascot: places the roots from the pose (head, body and tilt
 * transforms), steps [HairSim] and keeps each point in the mascot's drawing
 * frame, ready for [drawHair].
 */
internal class HairRig {
    private val sim = HairSim()
    val localX = FloatArray(STRANDS * POINTS)
    val localY = FloatArray(STRANDS * POINTS)
    private val body = Affine()
    private val head = Affine()
    private val inverse = Affine()
    private var ready = false
    private val rootsX = FloatArray(STRANDS)
    private val rootsY = FloatArray(STRANDS)

    init {
        for (s in 0 until STRANDS) {
            val strand = LAYOUT[s % LAYOUT.size]
            val right = s >= LAYOUT.size
            sim.shape(s, strand.flareDeg, strand.segment, if (right) 1f else -1f)
            val t = strand.t
            val lx = ROOF_BASE_X + (ROOF_APEX_X - ROOF_BASE_X) * t
            rootsX[s] = if (right) 1024f - lx else lx
            rootsY[s] = ROOF_BASE_Y + (ROOF_APEX_Y - ROOF_BASE_Y) * t
        }
    }

    /** Forget where the hair was (it hung somewhere else): it drops straight from the head next frame. */
    fun reset() {
        ready = false
    }

    /** Moves the hair on by [dt] seconds with the mascot in pose [p]. */
    fun update(dt: Double, p: Pose) {
        body.identity()
        body.translate(p.x, p.y)
        body.rotate(p.rot, FEET.x.toDouble(), FEET.y.toDouble())
        body.scale(p.sx * (if (p.flip) -1.0 else 1.0), p.sy, FEET.x.toDouble(), FEET.y.toDouble())
        body.rotate(TILT.toDouble(), TILT_PIVOT.x.toDouble(), TILT_PIVOT.y.toDouble())
        if (!body.invertTo(inverse)) return
        head.identity()
        head.translate(p.headX, p.headY)
        head.rotate(p.head, HEAD_PIVOT.x.toDouble(), HEAD_PIVOT.y.toDouble())
        // seen from the side the house is narrower, and the hair grows with it
        val th = p.turn * PI / 2
        val squeeze = (1 - 0.2 * abs(sin(th))) * cos(th)
        for (s in 0 until STRANDS) {
            val hx = 512.0 + (rootsX[s] - 512.0) * squeeze
            val hy = rootsY[s].toDouble()
            val wx = head.x(hx, hy)
            val wy = head.y(hx, hy)
            sim.root(s, body.x(wx, wy).toFloat(), body.y(wx, wy).toFloat())
        }
        if (!ready) {
            sim.hang()
            ready = true
        } else {
            sim.advance(dt)
        }
        for (i in sim.x.indices) {
            localX[i] = inverse.x(sim.x[i].toDouble(), sim.y[i].toDouble()).toFloat()
            localY[i] = inverse.y(sim.x[i].toDouble(), sim.y[i].toDouble()).toFloat()
        }
    }

    // reused by [drawHair]
    private val edgeLX = FloatArray(POINTS)
    private val edgeLY = FloatArray(POINTS)
    private val edgeRX = FloatArray(POINTS)
    private val edgeRY = FloatArray(POINTS)
    private val path = Path()

    /** Draws the strands behind the house ([front] false, in [dark]) or over it ([front] true, in [base]). */
    fun draw(scope: DrawScope, front: Boolean, base: Color, dark: Color) {
        for (s in 0 until STRANDS) {
            if (LAYOUT[s % LAYOUT.size].front != front) continue
            outline(s)
            scope.drawPath(path, if (front) base else dark)
        }
    }

    /** Builds the tapered outline of strand [s] into [path]. */
    private fun outline(s: Int) {
        val first = s * POINTS
        for (p in 0 until POINTS) {
            val before = first + max(p - 1, 0)
            val after = first + min(p + 1, POINTS - 1)
            var dx = localX[after] - localX[before]
            var dy = localY[after] - localY[before]
            val d = max(sqrt(dx * dx + dy * dy), 1e-4f)
            dx /= d; dy /= d
            val half = HAIR_WIDTH * (1f - (1f - HAIR_TIP) * p / (POINTS - 1)) / 2
            val i = first + p
            edgeLX[p] = localX[i] - dy * half
            edgeLY[p] = localY[i] + dx * half
            edgeRX[p] = localX[i] + dy * half
            edgeRY[p] = localY[i] - dx * half
        }
        path.rewind()
        path.moveTo(edgeLX[0], edgeLY[0])
        for (p in 1 until POINTS - 1) {
            path.quadraticTo(edgeLX[p], edgeLY[p], (edgeLX[p] + edgeLX[p + 1]) / 2, (edgeLY[p] + edgeLY[p + 1]) / 2)
        }
        path.lineTo(edgeLX[POINTS - 1], edgeLY[POINTS - 1])
        path.lineTo(edgeRX[POINTS - 1], edgeRY[POINTS - 1])
        for (p in POINTS - 2 downTo 1) {
            path.quadraticTo(edgeRX[p], edgeRY[p], (edgeRX[p] + edgeRX[p - 1]) / 2, (edgeRY[p] + edgeRY[p - 1]) / 2)
        }
        path.lineTo(edgeRX[0], edgeRY[0])
        path.close()
    }
}
