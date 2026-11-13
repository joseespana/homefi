package com.perseusoft.homefi.ui.exercise

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.drawscope.translate
import com.perseusoft.homefi.core.SegmentKind
import kotlin.math.PI
import kotlin.math.acos
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.hypot
import kotlin.math.min
import kotlin.math.sin

// HomeFito on a bike: the mascot (a white house with headphones) riding the course, pedalling with the part. One spec
// with the desktop (`lib/homefito-rider.ts`) and the TV (`ride.html`): keep the numbers in sync.
//
// Units: the rear wheel touches the ground at (REAR_X, 0), x forward, y down (up is negative), HEIGHT units tall.
// The legs follow the crank (two-bone reach to the pedals), the body leans and bobs with the effort, and on hard
// parts the eyes squeeze and sweat flies back; in easy ones he smiles upright.

internal const val HOMEFITO_HEIGHT = 30f
internal const val HOMEFITO_REAR_X = 4.5f

/** How the rider feels a part. */
internal enum class Strain(val cadenceHz: Float, val lean: Float) {
    EASY(1.1f, 0f),
    NORMAL(1.45f, 0.1f),
    HARD(1.2f, 0.24f),
}

internal fun SegmentKind?.strain(): Strain = when (this) {
    SegmentKind.SEATED_CLIMB, SegmentKind.STANDING_CLIMB, SegmentKind.SPRINT, SegmentKind.JUMPS -> Strain.HARD
    SegmentKind.WARM_UP, SegmentKind.RECOVERY, SegmentKind.COOL_DOWN -> Strain.EASY
    else -> Strain.NORMAL
}

private const val WHEEL_R = 4.5f
private val REAR = Offset(4.5f, -4.5f)
private val FRONT = Offset(19.5f, -4.5f)
private val CRANK = Offset(11f, -4.5f)
private const val CRANK_R = 2.2f
private val SEAT = Offset(9.2f, -11.6f)
private val HEAD = Offset(16.6f, -10.6f)
private val BAR = Offset(17.6f, -12.8f)
private val HIP = Offset(9.8f, -12.9f)
private const val THIGH = 5f
private const val SHIN = 5f
private val SHOULDER = Offset(12.6f, -15.4f)
private const val WALL_L = 4.6f
private const val WALL_R = 14.6f
private const val WALL_TOP = -20.5f
private const val EAVE = 1f
private const val RIDGE_Y = -25.5f
private const val CUP_RX = 1.3f
private const val CUP_RY = 2f
private const val CUP_Y = -17.6f
private const val BAND_TOP = -30f
private const val EYE_Y = -16.6f
private const val EYE_L = 9.4f
private const val EYE_R = 12.4f
private const val EYE_SIZE = 0.95f
private const val STILL_CRANK = (PI / 4).toFloat()
private const val BOB = 0.35f
private const val FAR_ALPHA = 0.55f
private const val PAUSED_ALPHA = 0.6f
private val SWEAT = Color(0xFF7FD3FF)
private const val TAU = (2 * PI).toFloat()

/** The knee of a leg from [hip] to [foot]: the two-bone solution that bends forward (towards +x). */
private fun knee(hip: Offset, foot: Offset): Offset {
    val dx = foot.x - hip.x
    val dy = foot.y - hip.y
    val d = min(hypot(dx, dy), THIGH + SHIN - 0.01f)
    val a = atan2(dy, dx)
    val b = acos(((THIGH * THIGH + d * d - SHIN * SHIN) / (2 * THIGH * d)).coerceIn(-1f, 1f))
    val k1 = Offset(hip.x + THIGH * cos(a - b), hip.y + THIGH * sin(a - b))
    val k2 = Offset(hip.x + THIGH * cos(a + b), hip.y + THIGH * sin(a + b))
    return if (k1.x >= k2.x) k1 else k2
}

/** [p] turned by [ang] (radians) around [o]. */
private fun turn(p: Offset, o: Offset, ang: Float): Offset {
    val c = cos(ang)
    val s = sin(ang)
    val d = p - o
    return Offset(o.x + d.x * c - d.y * s, o.y + d.x * s + d.y * c)
}

private val HOUSE: Path by lazy {
    Path().apply {
        moveTo(WALL_L, HIP.y + 0.4f)
        lineTo(WALL_L, WALL_TOP)
        lineTo(WALL_L - EAVE, WALL_TOP)
        lineTo((WALL_L + WALL_R) / 2, RIDGE_Y)
        lineTo(WALL_R + EAVE, WALL_TOP)
        lineTo(WALL_R, WALL_TOP)
        lineTo(WALL_R, HIP.y + 0.4f)
        close()
    }
}

private val BAND: Path by lazy {
    Path().apply {
        moveTo(WALL_L - 0.3f, CUP_Y - CUP_RY)
        quadraticTo((WALL_L + WALL_R) / 2, BAND_TOP, WALL_R + 0.3f, CUP_Y - CUP_RY)
    }
}

/**
 * Draws HomeFito riding with the scope already placed: (0, 0) is where the rear wheel meets the ridge, scaled to
 * units and turned to the slope. [clock] is the animation clock in seconds (null: a still picture).
 */
internal fun DrawScope.drawHomefito(zone: Color, ink: Color, strain: Strain, clock: Float?, paused: Boolean) {
    val moving = clock != null && !paused
    val crank = if (moving) clock!! * strain.cadenceHz * TAU else STILL_CRANK
    val lean = strain.lean
    val bob = if (moving) BOB * sin(crank * 2) else 0f
    val base = if (paused) PAUSED_ALPHA else 1f
    val inkEdge = ink.copy(alpha = 0.85f * base)
    val inkSolid = ink.copy(alpha = base)
    val white = Color.White.copy(alpha = base)
    val zoneC = zone.copy(alpha = zone.alpha * base)

    fun line(a: Offset, b: Offset, w: Float, c: Color) = drawLine(c, a, b, w, StrokeCap.Round)
    fun dot(p: Offset, r: Float, c: Color) = drawCircle(c, r, p)
    // a limb: white with an ink edge, like the mascot's arms and legs
    fun limb(points: List<Offset>, alpha: Float) {
        val path = Path().apply {
            moveTo(points[0].x, points[0].y)
            for (i in 1 until points.size) lineTo(points[i].x, points[i].y)
        }
        drawPath(path, ink.copy(alpha = 0.85f * alpha), style = Stroke(1.7f, cap = StrokeCap.Round, join = StrokeJoin.Round))
        drawPath(path, Color.White.copy(alpha = alpha), style = Stroke(1.05f, cap = StrokeCap.Round, join = StrokeJoin.Round))
    }
    fun pedal(phase: Float) = Offset(CRANK.x + CRANK_R * cos(crank + phase), CRANK.y + CRANK_R * sin(crank + phase))
    val hip = Offset(HIP.x, HIP.y + bob)

    // the far leg, behind the bike
    val farFoot = pedal(PI.toFloat())
    limb(listOf(hip, knee(hip, farFoot), farFoot), base * FAR_ALPHA)
    dot(farFoot, 0.8f, ink.copy(alpha = base * FAR_ALPHA))

    // wheels and frame, in the zone's colour
    for (hub in listOf(REAR, FRONT)) {
        drawCircle(ink.copy(alpha = 0.9f * base), WHEEL_R - 0.4f, hub, style = Stroke(1.3f))
        drawCircle(zoneC, WHEEL_R - 0.4f, hub, style = Stroke(0.6f))
        dot(hub, 0.6f, zoneC)
    }
    line(REAR, CRANK, 1.1f, zoneC)
    line(CRANK, SEAT, 1.1f, zoneC)
    line(SEAT, REAR, 1f, zoneC)
    line(SEAT, HEAD, 1.1f, zoneC)
    line(CRANK, HEAD, 1.1f, zoneC)
    line(HEAD, FRONT, 1f, zoneC)
    line(HEAD, BAR, 1f, zoneC)
    line(Offset(SEAT.x - 1.4f, SEAT.y - 0.3f), Offset(SEAT.x + 1.4f, SEAT.y - 0.3f), 1.2f, inkSolid)

    // the body: the house leaning from the hip, bobbing with the stroke
    translate(0f, bob) {
        rotate(lean * 180f / PI.toFloat(), pivot = HIP) {
            drawPath(HOUSE, white)
            drawPath(HOUSE, inkEdge, style = Stroke(0.6f, join = StrokeJoin.Round))
            // headphones: the band over the roof, a cup on each side in the zone's colour
            drawPath(BAND, ink.copy(alpha = 0.9f * base), style = Stroke(0.9f, cap = StrokeCap.Round))
            for (cx in floatArrayOf(WALL_L - 0.3f, WALL_R + 0.3f)) {
                val tl = Offset(cx - CUP_RX, CUP_Y - CUP_RY)
                val sz = Size(CUP_RX * 2, CUP_RY * 2)
                drawOval(zoneC, tl, sz)
                drawOval(ink.copy(alpha = 0.9f * base), tl, sz, style = Stroke(0.5f))
            }
            // the face: round eyes and a smile; on a hard part squeezed eyes and a tight mouth
            if (strain == Strain.HARD) {
                for (ex in floatArrayOf(EYE_L, EYE_R)) {
                    line(Offset(ex - 0.8f, EYE_Y - 0.5f), Offset(ex + 0.8f, EYE_Y), 0.55f, inkSolid)
                    line(Offset(ex + 0.8f, EYE_Y), Offset(ex - 0.8f, EYE_Y + 0.5f), 0.55f, inkSolid)
                }
                line(Offset(EYE_L + 0.1f, EYE_Y + 2.4f), Offset(EYE_R - 0.1f, EYE_Y + 2.4f), 0.55f, inkSolid)
            } else {
                dot(Offset(EYE_L, EYE_Y), EYE_SIZE, inkSolid)
                dot(Offset(EYE_R, EYE_Y), EYE_SIZE, inkSolid)
                val r = if (strain == Strain.EASY) 1.2f else 0.9f
                val c = Offset((EYE_L + EYE_R) / 2, EYE_Y + 1.3f)
                drawArc(inkSolid, 45f, 90f, false, Offset(c.x - r, c.y - r), Size(2 * r, 2 * r), style = Stroke(0.55f, cap = StrokeCap.Round))
            }
        }
    }

    // the near arm to the bars (the shoulder moves with the lean and the bob), then the near leg over the frame
    val shoulder = turn(Offset(SHOULDER.x, SHOULDER.y + bob), hip, lean)
    limb(listOf(shoulder, BAR), base)
    dot(BAR, 0.75f, white)
    val foot = pedal(0f)
    line(CRANK, foot, 0.8f, inkSolid)
    limb(listOf(hip, knee(hip, foot), foot), base)
    dot(foot, 0.8f, inkSolid)

    // sweat flying back off the roof on a hard part
    if (strain == Strain.HARD && moving) {
        for (i in 0 until 2) {
            val p = (clock!! * 1.3f + i * 0.5f) % 1f
            dot(Offset(WALL_L - 1 - 4 * p, WALL_TOP - 1 + 3 * p * p), 0.55f, SWEAT.copy(alpha = base * (1 - p)))
        }
    }
}
