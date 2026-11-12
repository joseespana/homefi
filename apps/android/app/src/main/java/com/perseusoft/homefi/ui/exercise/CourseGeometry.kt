package com.perseusoft.homefi.ui.exercise

import androidx.compose.ui.graphics.Color
import com.perseusoft.homefi.core.SegmentKind
import kotlin.math.abs
import kotlin.math.ceil
import kotlin.math.exp
import kotlin.math.floor
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt
import kotlin.math.sin

// The course mountain's pure geometry (docs/MOUNTAIN.md; mirrors apps/desktop/src/lib/course-render.ts): resampling,
// smoothing, the ridge's texture, zone colours and light. Units are dp; nothing here touches Compose drawing, so it
// is unit-tested.

/** One elevation sample of the course: second and height (0..1). */
class CoursePoint(val tS: Float, val elevation: Float)

/** A part of the course; [kind] is null for a saved ride whose parts are not stored. */
class CoursePart(val kind: SegmentKind?, val startS: Float, val seconds: Float)

/** What the course is: the core's profile and parts; [totalS] never below 1. */
class CourseModel(val profile: List<CoursePoint>, val parts: List<CoursePart>) {
    val totalS: Float = max(1f, profile.lastOrNull()?.tS ?: parts.sumOf { it.seconds.toDouble() }.toFloat())
}

internal const val MINI_H = 72f // under this height: no ribbon, ticks or pin
private val TICK_STEPS_S = intArrayOf(60, 120, 300, 600, 900, 1200, 1800)
private const val TICKS_MOST = 9
private const val TICKS_MOST_NARROW = 4
private const val NARROW_W = 450f
private const val SUMMIT_MAX = 3
private const val SUMMIT_PROMINENCE = 0.16f
private const val MIN_SAMPLES = 160
private const val MAX_SAMPLES = 700
private const val PX_PER_SAMPLE = 2f
private const val SMOOTH_SIGMA = 0.0055f
private const val NOISE_SEED = 5

internal fun clamp(v: Float, a: Float, b: Float): Float = if (v < a) a else if (v > b) b else v
internal fun smooth01(t: Float): Float {
    val u = clamp(t, 0f, 1f)
    return u * u * (3 - 2 * u)
}

internal fun mix(a: Color, b: Color, t: Float): Color =
    Color(a.red + (b.red - a.red) * t, a.green + (b.green - a.green) * t, a.blue + (b.blue - a.blue) * t, 1f)

/** Past (ridden) colours: desaturated toward slate blue, not toward black or grey. */
internal fun dimmed(c: Color): Color {
    val l = 0.3f * c.red + 0.59f * c.green + 0.11f * c.blue
    return mix(mix(Color(l, l, l), c, 0.25f), SLATE, 0.52f)
}

internal val SLATE = Color(52 / 255f, 66 / 255f, 118 / 255f)

/** The zones, fixed: the mountain is a night scene in every theme. */
internal val ZONE_COLOR: Map<SegmentKind, Color> = mapOf(
    SegmentKind.WARM_UP to Color(0xFF5EB1FF),
    SegmentKind.SEATED_FLAT to Color(0xFF3DDC97),
    SegmentKind.SEATED_CLIMB to Color(0xFFFFC233),
    SegmentKind.STANDING_CLIMB to Color(0xFFFF8A3D),
    SegmentKind.JUMPS to Color(0xFFB983FF),
    SegmentKind.SPRINT to Color(0xFFFF4D70),
    SegmentKind.RECOVERY to Color(0xFF2FD6C4),
    SegmentKind.COOL_DOWN to Color(0xFF9AA8FF),
)
private val NEUTRAL = Color(0xFFB9C6E6)

internal fun SegmentKind?.rgb(): Color = if (this == null) NEUTRAL else ZONE_COLOR.getValue(this)

private fun hnoise(n: Int, seed: Int): Float {
    val r = sin(n * 127.1 + seed * 311.7) * 43758.5453
    return (r - floor(r)).toFloat()
}

private fun vnoise(x: Float, seed: Int): Float {
    val i = floor(x)
    val f = smooth01(x - i)
    return hnoise(i.toInt(), seed) * (1 - f) + hnoise(i.toInt() + 1, seed) * f
}

internal fun fbm(x: Float, seed: Int): Float =
    vnoise(x, seed) * 0.62f + vnoise(x * 2.3f + 11, seed + 3) * 0.28f + vnoise(x * 5.1f + 29, seed + 7) * 0.1f

internal fun elevationAt(points: List<CoursePoint>, t: Float): Float {
    for (i in 1 until points.size) {
        val c = points[i]
        if (t <= c.tS) {
            val a = points[i - 1]
            val span = c.tS - a.tS
            return if (span <= 0f) c.elevation else a.elevation + (c.elevation - a.elevation) * (t - a.tS) / span
        }
    }
    return points.lastOrNull()?.elevation ?: 0f
}

internal fun partIndexAt(parts: List<CoursePart>, t: Float): Int {
    for (i in parts.indices.reversed()) if (t >= parts[i].startS) return i
    return 0
}

/** The first later part of a different kind than the current one: split parts of one kind are not "next". */
internal fun nextDifferent(parts: List<CoursePart>, current: Int): Int {
    val kind = parts.getOrNull(current)?.kind
    for (i in current + 1 until parts.size) if (parts[i].kind != kind) return i
    return -1
}

private fun gaussSmooth(raw: FloatArray, sigma: Float): FloatArray {
    val n = raw.size
    val r = max(1, ceil(sigma * 3).toInt())
    val k = FloatArray(2 * r + 1) { exp(-((it - r) * (it - r)) / (2 * sigma * sigma)) }
    return FloatArray(n) { i ->
        var acc = 0f
        var wsum = 0f
        for (j in -r..r) {
            val w = k[j + r]
            acc += raw[(i + j).coerceIn(0, n - 1)] * w
            wsum += w
        }
        acc / wsum
    }
}

/** The zone colour at [t]: every boundary is a short smoothstep blend, so washes and ridges show no seams. */
private fun zoneColourAt(parts: List<CoursePart>, t: Float, total: Float): Color {
    if (parts.isEmpty()) return NEUTRAL
    val p = partIndexAt(parts, t)
    val part = parts[p]
    var c = part.kind.rgb()
    val bw = total * 0.006f
    val ds = t - part.startS
    val de = part.startS + part.seconds - t
    if (ds < bw && p > 0) c = mix(parts[p - 1].kind.rgb(), c, 0.5f + 0.5f * smooth01(ds / bw))
    else if (de < bw && p < parts.size - 1) c = mix(parts[p + 1].kind.rgb(), c, 0.5f + 0.5f * smooth01(de / bw))
    return c
}

/** Local maxima over 6 % of the ride with enough prominence: tallest first, at most three, 12 % apart. */
internal fun summitsOf(e: FloatArray): List<Int> {
    val m = e.size
    val w = (m * 0.06f).roundToInt()
    val candidates = ArrayList<Int>()
    for (i in w until m - w) {
        if ((-w..w).any { e[i + it] > e[i] }) continue
        var minL = 9f
        var minR = 9f
        var j = 0
        while (j < m * 0.3f && i - j >= 0) minL = min(minL, e[i - j++])
        j = 0
        while (j < m * 0.3f && i + j < m) minR = min(minR, e[i + j++])
        if (e[i] - max(minL, minR) > SUMMIT_PROMINENCE) candidates += i
    }
    candidates.sortByDescending { e[it] }
    val out = ArrayList<Int>()
    for (c in candidates) {
        if (out.size >= SUMMIT_MAX) break
        if (out.all { abs(it - c) >= m * 0.12f }) out += c
    }
    return out
}

/** Ticks: the smallest step that gives at most 9 labels (4 on a narrow card). */
internal fun tickStep(total: Float, w: Float): Int {
    val most = if (w < NARROW_W) TICKS_MOST_NARROW else TICKS_MOST
    return TICK_STEPS_S.firstOrNull { total / it <= most } ?: TICK_STEPS_S.last()
}

/** Where the plot starts on the left (and ends on the right): room for the rider glyph at both ends (thumbnails need none). */
internal fun plotPad(w: Float, h: Float): Float = if (h >= MINI_H) clamp(h * 0.1f, 18f, 40f) else clamp(w * 0.012f, 8f, 16f)

/** The card's measures and the ridge's samples, with a flat run-out sample at each end. */
internal class CourseLayout(
    val w: Float,
    val h: Float,
    val mini: Boolean,
    val tick: Float,
    val band: Float,
    val floor: Float,
    val yLo: Float,
    val yHi: Float,
    val r: Float,
    val padX: Float,
    val m: Int,
    val emin: Float,
    val emax: Float,
    /** x, textured ridge y, smooth ridge y, elevation, zone colour, light. */
    val xs: FloatArray,
    val ys: FloatArray,
    val ys0: FloatArray,
    val eh: FloatArray,
    val zc: Array<Color>,
    val light: FloatArray,
    val summits: List<Int>,
    val peak: Int,
) {
    fun xOfT(t: Float, total: Float): Float = padX + clamp(t / total, 0f, 1f) * (w - 2 * padX)

    /** The textured ridge's y at [x] (linear between samples). */
    fun ridgeY(x: Float): Float {
        val cx = clamp(x, 0f, w)
        var lo = 0
        var hi = xs.size - 1
        while (hi - lo > 1) {
            val mid = (lo + hi) ushr 1
            if (xs[mid] <= cx) lo = mid else hi = mid
        }
        val span = xs[hi] - xs[lo]
        return if (span <= 0f) ys[lo] else ys[lo] + (ys[hi] - ys[lo]) * (cx - xs[lo]) / span
    }

    /** The highest (smallest y) ridge point between two x positions. */
    fun ridgeTop(x0: Float, x1: Float): Float {
        var top = min(ridgeY(x0), ridgeY(x1))
        for (i in xs.indices) if (xs[i] in x0..x1) top = min(top, ys[i])
        return top
    }
}

internal fun layoutCourse(model: CourseModel, w: Float, h: Float): CourseLayout {
    val mini = h < MINI_H
    val total = model.totalS
    val tick = if (mini) 0f else clamp(h * 0.085f, 11f, 20f)
    val band = if (mini) 0f else clamp(h * 0.05f, 4f, 12f)
    val floor = h - tick
    val yLo = floor - band - h * 0.045f
    val yHi = h * (if (mini) 0.12f else if (h < 130f) 0.2f else 0.21f)
    val range = yLo - yHi
    val padX = plotPad(w, h)
    val m = (w / PX_PER_SAMPLE).roundToInt().coerceIn(MIN_SAMPLES, MAX_SAMPLES)

    val raw = FloatArray(m) { clamp(elevationAt(model.profile, total * it / (m - 1)), 0f, 1f) }
    val sm = gaussSmooth(raw, m * SMOOTH_SIGMA)
    // the gaussian shaves the summits, and the peaks are the point: stretch back to the data's own range
    val rlo = raw.min()
    val rhi = raw.max()
    val slo = sm.min()
    val shi = sm.max()
    val e = FloatArray(m) { if (shi > slo) rlo + (sm[it] - slo) / (shi - slo) * (rhi - rlo) else sm[it] }

    val amp = clamp(h * 0.011f, 0f, 3.6f)
    val n = m + 2
    val xs = FloatArray(n)
    val ys = FloatArray(n)
    val ys0 = FloatArray(n)
    val eh = FloatArray(n)
    val zc = Array(n) { NEUTRAL }
    for (i in 0 until m) {
        val u = i / (m - 1f)
        val noise = (fbm(u * 60, NOISE_SEED) - 0.5f) * 2 * amp * (0.35f + e[i])
        xs[i + 1] = padX + u * (w - 2 * padX)
        ys0[i + 1] = yLo - e[i] * range
        ys[i + 1] = ys0[i + 1] + noise
        eh[i + 1] = e[i]
        zc[i + 1] = zoneColourAt(model.parts, total * u, total)
    }
    // flat run-outs to the card edges, so the terrain is edge to edge
    xs[0] = 0f; ys0[0] = ys0[1]; ys[0] = ys[1]; zc[0] = zc[1]; eh[0] = eh[1]
    xs[n - 1] = w; ys0[n - 1] = ys0[n - 2]; ys[n - 1] = ys[n - 2]; zc[n - 1] = zc[n - 2]; eh[n - 1] = eh[n - 2]

    // light from the left: a ridge rising to the right faces the sun
    val light = FloatArray(n) { i ->
        val a = max(0, i - 2)
        val b = min(n - 1, i + 2)
        val slope = (ys0[b] - ys0[a]) / max(1f, xs[b] - xs[a])
        clamp(-slope * 1.1f, -1f, 1f)
    }
    var peak = 0
    for (i in 1 until m) if (e[i] > e[peak]) peak = i
    return CourseLayout(
        w, h, mini, tick, band, floor, yLo, yHi, range, padX, m, rlo, rhi,
        xs, ys, ys0, eh, zc, light, summitsOf(e).map { it + 1 }, peak + 1,
    )
}

/** m:ss */
internal fun mmss(seconds: Float): String {
    val n = max(0, seconds.roundToInt())
    return "${n / 60}:${(n % 60).toString().padStart(2, '0')}"
}
