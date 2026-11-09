package com.perseusoft.homefi.ui.exercise

import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Canvas
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.Paint
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.CanvasDrawScope
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipPath
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.sp
import com.perseusoft.homefi.core.CourseLook
import kotlin.math.PI
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.floor
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt
import kotlin.math.sin

// The course mountain's painter (docs/MOUNTAIN.md; mirrors apps/desktop/src/lib/course-render.ts). Three expensive
// bitmaps (backdrop, ridden, to-ride) are built once per size, course and look ([buildCourseLayers]); a frame is
// a few drawImage calls plus the overlay ([drawCourse]): rider, beacon, trail, pin, ticks. Geometry is in dp, the
// bitmaps are at the screen's density.

private class LookPreset(
    val ground: List<Pair<Float, Color>>,
    /** Alpha of the zone wash at the ridge and at the foot. */
    val wash: Pair<Float, Float>,
    /** The ridge is a lit rim over the zone colour (alpine, stage) instead of the zone colour itself (ribbon). */
    val lightRidge: Boolean,
    val snow: Float,
    val sky: List<Pair<Float, Color>>,
    val glow: Color,
    val currentGlow: Float,
    val currentRim: Float,
)

private val PRESETS: Map<CourseLook, LookPreset> = mapOf(
    CourseLook.RIBBON to LookPreset(
        ground = listOf(0f to Color(0xFF0A2740), 0.25f to Color(0xFF143F6E), 0.5f to Color(0xFF35409A), 0.75f to Color(0xFF74408F), 0.92f to Color(0xFFB2527F), 1f to Color(0xFFE58A66)),
        wash = 0.16f to 0f, lightRidge = false, snow = 0.7f,
        sky = listOf(0f to Color(0xFF060A1A), 0.45f to Color(0xFF101847), 0.78f to Color(0xFF38285F), 1f to Color(0xFF7D4A78)),
        glow = Color(0xFFFFB48A), currentGlow = 0.34f, currentRim = 0.12f,
    ),
    CourseLook.ALPINE to LookPreset(
        ground = listOf(0f to Color(0xFF0A1322), 0.5f to Color(0xFF1C2B47), 1f to Color(0xFF4A6288)),
        wash = 0.24f to 0f, lightRidge = true, snow = 1f,
        sky = listOf(0f to Color(0xFF06111D), 0.5f to Color(0xFF0F2C47), 0.82f to Color(0xFF265673), 1f to Color(0xFF6C97A6)),
        glow = Color(0xFFBFE6FF), currentGlow = 0.2f, currentRim = 0.5f,
    ),
    CourseLook.STAGE to LookPreset(
        ground = listOf(0f to Color(0xFF080E22), 1f to Color(0xFF1C2754)),
        wash = 0.6f to 0.1f, lightRidge = true, snow = 0f,
        sky = listOf(0f to Color(0xFF060A1A), 0.5f to Color(0xFF111A44), 0.8f to Color(0xFF2C2660), 1f to Color(0xFF6A4678)),
        glow = Color(0xFFFF9FB0), currentGlow = 0.2f, currentRim = 0.5f,
    ),
)

private val INK = Color(4 / 255f, 6 / 255f, 18 / 255f)
private val FOOTER = Color(7 / 255f, 10 / 255f, 20 / 255f).copy(alpha = 0.9f)
private val TICK_TEXT = Color(205 / 255f, 213 / 255f, 230 / 255f).copy(alpha = 0.55f)
private val PIN_TEXT = Color(0xFF06080F)
private val WARM_WHITE = Color(255 / 255f, 244 / 255f, 230 / 255f)
private val SNOW_LIT = Color(232 / 255f, 240 / 255f, 255 / 255f)
private val SNOW_SHADE = Color(128 / 255f, 150 / 255f, 214 / 255f)
private const val TAU = (2 * PI).toFloat()
private const val BREATH_NEXT_RAD_S = 2.4f
private const val BEACON_HZ = 1f
private const val MAX_BITMAP_DENSITY = 3f

/** The cached layers of one (size, course, look); rebuilt only when one of them changes. */
internal class CourseLayers(
    val g: CourseLayout,
    val look: CourseLook,
    val density: Float,
    val terrain: Path,
    val back: ImageBitmap,
    val dim: ImageBitmap,
    val lit: ImageBitmap,
    /** Tick labels with their x (centre, or left for the first, right for the last). */
    val ticks: List<Tick>,
)

internal class Tick(val layout: TextLayoutResult, val x: Float, val align: Float)

private val OVERLAY_DENSITY = Density(1f, 1f)

private fun TextMeasurer.label(text: String, size: Float, weight: FontWeight): TextLayoutResult =
    measure(text, TextStyle(fontSize = size.sp, fontWeight = weight), density = OVERLAY_DENSITY)

private inline fun paintBitmap(widthPx: Int, heightPx: Int, density: Float, block: DrawScope.() -> Unit): ImageBitmap {
    val bitmap = ImageBitmap(widthPx, heightPx)
    CanvasDrawScope().draw(Density(density), LayoutDirection.Ltr, Canvas(bitmap), Size(widthPx.toFloat(), heightPx.toFloat())) {
        scale(density, density, pivot = Offset.Zero) { block() }
    }
    return bitmap
}

private fun vertical(stops: List<Pair<Float, Color>>, y0: Float, y1: Float): Brush =
    Brush.verticalGradient(colorStops = stops.toTypedArray(), startY = y0, endY = if (y1 > y0 + 0.5f) y1 else y0 + 0.5f)

private fun fade(c: Color, a: Float) = c.copy(alpha = a)

internal fun buildCourseLayers(model: CourseModel, look: CourseLook, w: Float, h: Float, density: Float, measurer: TextMeasurer): CourseLayers {
    val g = layoutCourse(model, w, h)
    val d = min(density, MAX_BITMAP_DENSITY)
    val wPx = max(1, (w * d).roundToInt())
    val hPx = max(1, (h * d).roundToInt())
    val terrain = Path().apply {
        moveTo(0f, g.floor)
        for (i in g.xs.indices) lineTo(g.xs[i], g.ys[i])
        lineTo(g.w, g.floor)
        close()
    }
    val preset = PRESETS.getValue(look)
    val back = paintBitmap(wPx, hPx, d) { paintBackdrop(g, preset) }
    val dim = paintBitmap(wPx, hPx, d) { paintTerrain(g, model, preset, terrain, dim = true, dpr = d) }
    val lit = paintBitmap(wPx, hPx, d) { paintTerrain(g, model, preset, terrain, dim = false, dpr = d) }
    return CourseLayers(g, look, d, terrain, back, dim, lit, if (g.mini) emptyList() else tickLabels(g, model, measurer))
}

private fun tickLabels(g: CourseLayout, model: CourseModel, measurer: TextMeasurer): List<Tick> {
    val total = model.totalS
    val step = tickStep(total, g.w)
    val size = clamp(g.h * 0.05f, 9f, 14f)
    val out = ArrayList<Tick>()
    var t = 0
    while (t < total) {
        if (!(t > 0 && total - t < step * 0.5f)) {
            out += Tick(measurer.label(mmss(t.toFloat()), size, FontWeight.SemiBold), if (t == 0) g.padX else g.xOfT(t.toFloat(), total), if (t == 0) 0f else 0.5f)
        }
        t += step
    }
    out += Tick(measurer.label(mmss(total), size, FontWeight.SemiBold), g.w - g.padX, 1f)
    return out
}

private fun DrawScope.paintBackdrop(g: CourseLayout, p: LookPreset) {
    drawRect(vertical(p.sky, 0f, g.floor), size = Size(g.w, g.h))
    if (g.mini) return
    // a few seeded stars
    var seed = 7
    fun rnd(): Float {
        seed = seed * 1103515245 + 12345
        return ((seed ushr 8) and 0xFFFF) / 65536f
    }
    repeat((g.w / 14).roundToInt()) {
        val sx = rnd() * g.w
        val sy = rnd() * g.floor * 0.5f
        val sr = 0.4f + rnd() * (if (g.h > 200) 0.9f else 0.5f)
        drawCircle(Color.White.copy(alpha = 0.18f + rnd() * 0.4f), sr, Offset(sx, sy))
    }
    // the glow behind the highest summit
    val px = g.xs[g.peak]
    val py = g.ys[g.peak] + g.r * 0.15f
    val gr = max(g.r * 1.1f, g.w * 0.12f)
    drawRect(
        Brush.radialGradient(0f to fade(p.glow, 0.34f), 0.45f to fade(p.glow, 0.1f), 1f to fade(p.glow, 0f), center = Offset(px, py), radius = gr),
        size = Size(g.w, g.h),
    )
    // two far ranges, hazier with distance, shaped from the same course so they rhyme with it
    val haze = p.sky.last().second
    val e = g.eh.copyOfRange(1, g.m + 1)
    listOf(
        FarRange(0.04f, 21, 0.31f, 0.55f, Color(86 / 255f, 96 / 255f, 160 / 255f), 0.42f),
        FarRange(0.2f, 33, 0.62f, 0.5f, Color(52 / 255f, 64 / 255f, 130 / 255f), 0.62f),
    ).forEachIndexed { li, l ->
        val topY = g.yHi + l.top * g.r
        val steps = (g.w / 3).roundToInt()
        val path = Path().apply {
            moveTo(0f, g.floor)
            for (i in 0..steps) {
                val u = i / steps.toFloat()
                // mirrored where it would wrap, so the range has no cliff
                val wv = (u * 0.85f + l.shift) % 2f
                val pe = e[((if (wv > 1) 2 - wv else wv) * (g.m - 1)).roundToInt().coerceIn(0, g.m - 1)]
                val v = clamp(l.k * pe + (1 - l.k) * fbm(u * 6 + l.shift * 9, l.seed) * 1.25f, 0f, 1f)
                lineTo(u * g.w, g.floor - (0.28f + 0.72f * v) * (g.floor - topY))
            }
            lineTo(g.w, g.floor)
            close()
        }
        drawPath(path, vertical(listOf(0f to fade(l.c, l.a), 1f to fade(mix(l.c, haze, 0.7f), l.a * 0.6f)), topY, g.floor))
        drawPath(path, fade(mix(l.c, Color.White, 0.45f), 0.16f + li * 0.06f), style = Stroke(1f))
        val mistTop = g.floor - g.r * 0.6f
        drawRect(vertical(listOf(0f to fade(haze, 0f), 1f to fade(haze, 0.32f)), mistTop, g.floor), Offset(0f, mistTop), Size(g.w, g.r * 0.6f))
    }
}

private class FarRange(val top: Float, val seed: Int, val shift: Float, val k: Float, val c: Color, val a: Float)

/** The near terrain; [dim] is the "already ridden" version. */
private fun DrawScope.paintTerrain(g: CourseLayout, model: CourseModel, p: LookPreset, terrain: Path, dim: Boolean, dpr: Float) {
    val n = g.xs.size
    fun c(col: Color) = if (dim) dimmed(col) else col
    // strips snap to whole device pixels, so neighbours never overlap (no hairline seams) or leave gaps
    fun colX(i: Int) = floor(g.xs[i] * dpr) / dpr
    clipPath(terrain) {
        // 1) ground, by elevation: cool at the foot, warm on the summits (ribbon) or a calm slate
        val span = g.floor - g.yHi
        val stops = ArrayList<Pair<Float, Color>>()
        for (k in p.ground.indices.reversed()) {
            val (e, col) = p.ground[k]
            stops += clamp((g.yLo - e * g.r - g.yHi) / span, 0f, 1f) to c(col)
        }
        stops += 1f to c(p.ground.first().second)
        drawRect(vertical(stops, g.yHi, g.floor), size = Size(g.w, g.floor))
        // 2) the zone washes the slope, strongest at the ridge and fading to the foot
        if (p.wash.first > 0f) {
            for (i in 0 until n - 1) {
                val x0 = colX(i)
                val x1 = colX(i + 1)
                if (x1 <= x0) continue
                val y = g.ys0[i]
                val col = c(g.zc[i])
                drawRect(
                    vertical(listOf(0f to fade(col, p.wash.first * (if (dim) 0.8f else 1f)), 1f to fade(col, p.wash.second)), y, g.floor),
                    Offset(x0, y - 5), Size(x1 - x0, g.floor - y + 5),
                )
            }
        }
        // 3) sun from the left: slopes that rise to the right catch light, the others sink
        for (i in 0 until n - 1) {
            val x0 = colX(i)
            val x1 = colX(i + 1)
            if (x1 <= x0) continue
            val l = g.light[i]
            val y = g.ys0[i]
            val depth = (g.floor - y) * 0.62f
            val top = if (l >= 0) fade(WARM_WHITE, 0.2f * l * (if (dim) 0.45f else 1f)) else fade(INK, 0.34f * -l)
            val bottom = fade(top, 0f)
            drawRect(vertical(listOf(0f to top, 1f to bottom), y, y + depth), Offset(x0, y - 5), Size(x1 - x0, depth + 5))
        }
        // 4) snow on the high ground (above 80 % of the ride's own elevation range)
        if (p.snow > 0f && g.emax > 0.7f) {
            val th = g.emin + (g.emax - g.emin) * 0.8f
            for (i in 0 until n - 1) {
                val eh = g.eh[i]
                if (eh <= th) continue
                val depth = (eh - th) / (g.emax - th) * g.r * 0.3f + (fbm(i * 0.06f, 9) - 0.4f) * g.r * 0.08f
                val x0 = colX(i)
                val x1 = colX(i + 1)
                if (depth < 1f || x1 <= x0) continue
                val sc = mix(SNOW_LIT, SNOW_SHADE, clamp(0.5f - g.light[i] * 0.5f, 0f, 1f))
                val sa = p.snow * (if (dim) 0.3f else 1f)
                drawRect(
                    vertical(listOf(0f to fade(sc, 0.95f * sa), 0.6f to fade(sc, 0.5f * sa), 1f to fade(sc, 0f)), g.ys0[i], g.ys0[i] + depth),
                    Offset(x0, g.ys0[i] - 5), Size(x1 - x0, depth + 5),
                )
            }
        }
    }
    // 5) the ridge: a dark under-line for contrast, then the zone colour (ribbon) or a lit rim (alpine, stage)
    val rw = clamp(g.h * 0.016f, 2f, 5f)
    val line = Path().apply {
        moveTo(g.xs[0], g.ys[0])
        for (q in 1 until n) lineTo(g.xs[q], g.ys[q])
    }
    drawPath(line, fade(INK, if (p.lightRidge) 0.35f else 0.55f), style = Stroke(rw + (if (p.lightRidge) 3f else 2.6f), cap = StrokeCap.Round, join = StrokeJoin.Round))
    for (i in 0 until n - 1) {
        val zc = c(g.zc[i])
        val col = if (p.lightRidge) fade(mix(zc, Color.White, if (dim) 0.25f else 0.55f), if (dim) 0.8f else 0.95f) else fade(zc, if (dim) 0.8f else 1f)
        val width = if (p.lightRidge) max(1.2f, rw * 0.55f) else rw
        drawLine(col, Offset(g.xs[i], g.ys[i]), Offset(g.xs[i + 1], g.ys[i + 1]), width, StrokeCap.Round)
    }
    if (g.mini) return
    // the tick strip is a calm footer, not sky
    drawRect(FOOTER, Offset(0f, g.floor), Size(g.w, g.h - g.floor))
    // 6) the base ribbon: one capsule per part, hard-edged so the zones stay countable
    val by = g.floor - g.band
    val bh = g.band - 1
    drawRect(Color.White.copy(alpha = 0.05f), Offset(0f, by - 1), Size(g.w, g.band + 1))
    for (part in model.parts) {
        val bx0 = g.xOfT(part.startS, model.totalS) + 1
        val bx1 = g.xOfT(part.startS + part.seconds, model.totalS) - 1
        if (bx1 - bx0 < 1f) continue
        drawRoundRect(fade(c(part.kind.rgb()), if (dim) 0.85f else 1f), Offset(bx0, by), Size(bx1 - bx0, bh), CornerRadius(min(bh / 2, (bx1 - bx0) / 2)))
    }
}

/** What changes from frame to frame (the course, the size and the look are in [CourseLayers]). */
internal class CourseScene(
    /** Seconds ridden: before it the terrain is dimmed. Null = nothing ridden (all lit); at the end = all dim. */
    val ridden: Float?,
    /** The current part, whose slope glows. */
    val current: Int?,
    /** The next different part: a dashed ridge and a pin with [pinText]. */
    val pinPart: Int?,
    val pinText: String,
    val summits: Boolean,
    /** Animation clock in seconds; null = calm: steady dot with a static double ring, halo at its middle value. */
    val clock: Float?,
)

private fun DrawScope.strokeRidge(g: CourseLayout, xa: Float, xb: Float, brush: Brush?, color: Color, width: Float, dash: PathEffect? = null) {
    var started = false
    val path = Path()
    for (i in g.xs.indices) {
        if (g.xs[i] < xa - 1 || g.xs[i] > xb + 1) continue
        if (!started) {
            path.moveTo(xa, g.ridgeY(xa))
            started = true
        }
        path.lineTo(g.xs[i], g.ys[i])
    }
    if (!started) return
    path.lineTo(xb, g.ridgeY(xb))
    val style = Stroke(width, cap = StrokeCap.Round, join = StrokeJoin.Round, pathEffect = dash)
    if (brush != null) drawPath(path, brush, style = style) else drawPath(path, color, style = style)
}

/** One frame, in dp coordinates of [layers]; call it inside the canvas of that size. */
internal fun DrawScope.drawCourse(layers: CourseLayers, model: CourseModel, scene: CourseScene, measurer: TextMeasurer) {
    val g = layers.g
    val preset = PRESETS.getValue(layers.look)
    val total = model.totalS
    val target = IntSize(size.width.roundToInt(), size.height.roundToInt())
    drawImage(layers.back, dstSize = target)

    val ridden = scene.ridden?.let { clamp(it, 0f, total) } ?: 0f
    val xr = g.xOfT(ridden, total)
    val allDim = scene.ridden != null && scene.ridden >= total
    if (allDim) {
        drawImage(layers.dim, dstSize = target)
    } else if (scene.ridden == null || scene.ridden <= 0f) {
        drawImage(layers.lit, dstSize = target)
    } else {
        drawImage(layers.dim, dstSize = target)
        // the lit (still to ride) terrain, with a soft edge at the rider
        val fadeW = clamp(g.w * 0.02f, 10f, 34f)
        val x0 = (xr - fadeW * 0.25f) * size.width / g.w
        val x1 = (xr + fadeW) * size.width / g.w
        clipRect(left = x1) { drawImage(layers.lit, dstSize = target) }
        drawIntoCanvas { canvas ->
            canvas.saveLayer(androidx.compose.ui.geometry.Rect(x0, 0f, x1, size.height), Paint())
            clipRect(left = x0, right = x1) { drawImage(layers.lit, dstSize = target) }
            drawRect(
                Brush.horizontalGradient(listOf(Color.Transparent, Color.Black), startX = x0, endX = x1),
                topLeft = Offset(x0, 0f), size = Size(x1 - x0, size.height), blendMode = BlendMode.DstIn,
            )
            canvas.restore()
        }
    }

    scale(size.width / g.w, size.height / g.h, pivot = Offset.Zero) {
        drawOverlay(layers, model, scene, preset, xr, measurer)
    }
}

private fun DrawScope.drawOverlay(layers: CourseLayers, model: CourseModel, scene: CourseScene, preset: LookPreset, xr: Float, measurer: TextMeasurer) {
    val g = layers.g
    val total = model.totalS
    val parts = model.parts
    val h = g.h
    val clock = scene.clock
    val live = scene.current != null
    val rw = clamp(h * 0.016f, 2f, 5f)
    val glowPart = scene.current?.let { parts.getOrNull(it) }

    // the current part: its slope glows in its own colour, and its stretch of ridge is thicker
    if (glowPart != null) {
        val zc = glowPart.kind.rgb()
        val px0 = g.xOfT(glowPart.startS, total)
        val px1 = g.xOfT(glowPart.startS + glowPart.seconds, total)
        clipPath(layers.terrain) {
            drawRect(vertical(listOf(0f to fade(zc, preset.currentGlow), 1f to fade(zc, 0f)), g.yHi, g.floor), Offset(px0, 0f), Size(px1 - px0, g.floor))
        }
        strokeRidge(g, px0, px1, null, fade(zc, 0.28f), rw * 3)
        strokeRidge(g, px0, px1, null, mix(zc, Color.White, preset.currentRim), if (preset.lightRidge) max(1.4f, rw * 0.7f) else rw)
    }

    // the trail: a bright thread along the ridge that fades out behind the rider
    val trailW = min(xr, g.w * 0.28f)
    if (live && trailW > 4f) {
        strokeRidge(g, xr - trailW, xr, Brush.horizontalGradient(listOf(Color.White.copy(alpha = 0f), Color.White.copy(alpha = 0.95f)), startX = xr - trailW, endX = xr), Color.Unspecified, max(1.4f, h * 0.009f))
    }

    // the next different part: dashed bright ridge over it, a pin with the name and the countdown
    var pinBox: androidx.compose.ui.geometry.Rect? = null
    val next = scene.pinPart?.let { parts.getOrNull(it) }
    if (next != null && !g.mini) {
        val nc = next.kind.rgb()
        val nx0 = g.xOfT(next.startS, total)
        val nx1 = g.xOfT(next.startS + next.seconds, total)
        val pulse = if (clock != null) 0.5f + 0.5f * sin(clock * BREATH_NEXT_RAD_S) else 0.6f
        val dash = PathEffect.dashPathEffect(floatArrayOf(clamp(h * 0.035f, 5f, 10f), clamp(h * 0.025f, 4f, 7f)))
        strokeRidge(g, nx0, nx1, null, fade(nc, 0.25f + 0.2f * pulse), rw * 3, dash)
        strokeRidge(g, nx0, nx1, null, mix(nc, Color.White, 0.2f), max(1.6f, rw * 0.8f), dash)
        val fs = clamp(h * 0.058f, 10f, 19f)
        val ph = fs * 1.7f
        val stem = clamp(h * 0.05f, 5f, 14f)
        val text = measurer.label(scene.pinText, fs, FontWeight.Bold)
        val pw = text.size.width + fs * 1.5f
        val ry = g.ridgeY(nx0)
        val pxl = clamp(nx0 - fs * 0.75f, 3f, max(3f, g.w - pw - 3f))
        // lift the pin above every ridge point under it, so it never hides the climb it names
        var hiY = ry
        var qx = pxl
        while (qx <= pxl + pw) {
            hiY = min(hiY, g.ridgeY(qx))
            qx += 4f
        }
        val py0 = max(2f, hiY - stem * 0.6f - ph)
        drawLine(fade(nc, 0.9f), Offset(nx0, ry), Offset(nx0, py0 + ph), 1.4f)
        drawRoundRect(fade(nc, 0.3f), Offset(pxl - 2, py0 - 2), Size(pw + 4, ph + 4), CornerRadius(ph / 2 + 2))
        drawRoundRect(nc, Offset(pxl, py0), Size(pw, ph), CornerRadius(ph / 2))
        pinBox = androidx.compose.ui.geometry.Rect(pxl, py0, pxl + pw, py0 + ph)
        drawText(text, PIN_TEXT, Offset(pxl + fs * 0.75f, py0 + (ph - text.size.height) / 2))
        drawCircle(nc, max(2.2f, rw * 0.9f), Offset(nx0, ry))
    }

    // summit labels (a setting): a small triangle and the time, only when there is width
    if (scene.summits && g.w >= 500f && !g.mini) {
        val text = clamp(h * 0.05f, 10f, 14f)
        for (s in g.summits) {
            val sx = g.xs[s]
            val sy = g.ys[s]
            val nearRider = live && kotlin.math.abs(sx - xr) < 34
            val underPin = pinBox != null && sx + 20 > pinBox.left && sx - 20 < pinBox.right && sy - 28 < pinBox.bottom
            if (nearRider || underPin) continue
            val col = if (scene.ridden != null && sx < xr) Color(190 / 255f, 200 / 255f, 225 / 255f).copy(alpha = 0.5f) else Color.White.copy(alpha = 0.9f)
            drawPath(Path().apply { moveTo(sx, sy - 5); lineTo(sx - 3.5f, sy - 11); lineTo(sx + 3.5f, sy - 11); close() }, col)
            val label = measurer.label(mmss((s - 1f) / (g.m - 1) * total), text, FontWeight.SemiBold)
            drawText(label, col, Offset(sx - label.size.width / 2f, sy - 15 - label.firstBaseline))
        }
    }

    if (live) drawRider(g, xr, glowPart?.kind.rgb(), clock)
    if (!g.mini) drawTicks(layers)
}

private fun DrawScope.drawTicks(layers: CourseLayers) {
    val g = layers.g
    val tf = clamp(g.h * 0.05f, 9f, 14f)
    val baseline = g.h - (g.tick - tf) / 2 - 1
    for (t in layers.ticks) {
        drawText(t.layout, TICK_TEXT, Offset(t.x - t.layout.size.width * t.align, baseline - t.layout.firstBaseline))
    }
}

/** The "you are here" marker: a still halo, a hairline down to the ribbon, the dot on the ridge and its beacon. */
private fun DrawScope.drawRider(g: CourseLayout, xr: Float, zc: Color, clock: Float?) {
    val yr = g.ridgeY(xr)
    val h = g.h
    val dr = clamp(h * 0.035f, 4f, 9f)
    val hr = clamp(h * 0.13f, 12f, 36f)
    if (!g.mini) drawLine(fade(zc, 0.55f), Offset(xr, yr + dr), Offset(xr, g.floor - g.band), 1f)
    drawCircle(Brush.radialGradient(0f to fade(zc, 0.7f), 0.4f to fade(zc, 0.22f), 1f to fade(zc, 0f), center = Offset(xr, yr), radius = hr), hr, Offset(xr, yr))
    val c = Offset(xr, yr)
    val blink: Float
    if (clock != null) {
        val p = (clock * BEACON_HZ) % 1f
        // the beacon: a ring expanding from the dot, fading as it goes
        val reach = clamp(h * 0.15f, 14f, 40f)
        val a = 0.85f * (1 - p).let { it * kotlin.math.sqrt(it) }
        drawCircle(fade(zc, a), dr + (reach - dr) * p, c, style = Stroke(max(1.5f, 0.3f * dr)))
        blink = 0.775f + 0.225f * cos(TAU * p)
    } else {
        drawCircle(fade(zc, 0.55f), dr * 2f, c, style = Stroke(max(1.5f, 0.3f * dr)))
        drawCircle(fade(zc, 0.28f), dr * 3.2f, c, style = Stroke(max(1.5f, 0.3f * dr)))
        blink = 1f
    }
    if (h >= CYCLIST_H) {
        val ang = clamp(atan2(g.ridgeY(xr + 9) - g.ridgeY(xr - 9), 18f), -0.6f, 0.6f)
        drawCyclist(xr, yr - dr - 3, clamp(h * 0.064f, 8f, 21f), ang, zc)
    }
    drawCircle(fade(INK, 0.55f * blink), dr + 0.75f, c)
    drawCircle(fade(zc, blink), dr, c)
    drawCircle(Color.White.copy(alpha = blink), dr * 0.5f, c)
}

private fun DrawScope.drawCyclist(px: Float, py: Float, s: Float, ang: Float, col: Color) {
    translate(px, py) {
        rotate(ang * 180f / PI.toFloat(), pivot = Offset.Zero) {
            translate(0f, -0.55f * s) {
                val width = max(1.3f, 0.13f * s)
                val st = Stroke(width, cap = StrokeCap.Round, join = StrokeJoin.Round)
                // a soft halo in the zone colour keeps it readable on any ground
                drawCircle(fade(col, 0.35f), s * 1.6f, Offset(0f, -0.4f * s))
                fun line(a: Float, b: Float, c: Float, e: Float) = drawLine(Color.White, Offset(a * s, b * s), Offset(c * s, e * s), width, StrokeCap.Round)
                drawCircle(Color.White, 0.55f * s, Offset(-s, 0f), style = st)
                drawCircle(Color.White, 0.55f * s, Offset(s, 0f), style = st)
                line(-1f, 0f, -0.15f, 0f); line(-0.15f, 0f, -0.35f, -0.75f); line(-0.35f, -0.75f, -1f, 0f)
                line(-0.35f, -0.75f, 0.7f, -0.7f); line(-0.15f, 0f, 0.7f, -0.7f); line(0.7f, -0.7f, 1f, 0f)
                line(-0.4f, -0.85f, 0.3f, -1.4f); line(0.3f, -1.4f, 0.72f, -0.82f)
                line(-0.4f, -0.85f, 0.2f, -0.38f); line(0.2f, -0.38f, 0.02f, 0.12f)
                drawCircle(Color.White, 0.22f * s, Offset(0.5f * s, -1.72f * s))
            }
        }
    }
}
