package com.perseusoft.homefi.ui.eq

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChanged
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.perseusoft.homefi.core.EqBandKind
import com.perseusoft.homefi.core.EqProfile
import com.perseusoft.homefi.core.eqLogFreqs
import com.perseusoft.homefi.core.eqResponse
import com.perseusoft.homefi.ui.haptics
import com.perseusoft.homefi.ui.theme.LocalMotion
import kotlin.math.abs
import kotlin.math.ln
import kotlin.math.max
import kotlin.math.pow
import kotlin.math.roundToInt

private const val MIN_HZ = 20f
private const val MAX_HZ = 20_000f
private const val CURVE_RATE = 48_000u

/** Points of the drawn curve, log-spaced (computed once by the core). */
private val curveFreqs: List<Float> by lazy { runCatching { eqLogFreqs(160u) }.getOrDefault(emptyList()) }

/** 0..1 along a log axis from 20 Hz to 20 kHz. */
fun freqToFraction(hz: Float): Float = (ln(hz / MIN_HZ) / ln(MAX_HZ / MIN_HZ)).coerceIn(0f, 1f)

fun fractionToFreq(f: Float): Float = MIN_HZ * (MAX_HZ / MIN_HZ).pow(f.coerceIn(0f, 1f))

/** The profile's response in dB at [curveFreqs] (no preamp), empty for none. */
fun responseOf(profile: EqProfile?): List<Float> =
    if (profile == null || profile.bands.isEmpty()) {
        List(curveFreqs.size) { 0f }
    } else {
        runCatching { eqResponse(profile, curveFreqs, CURVE_RATE) }.getOrDefault(emptyList())
    }

/** `1.2k`, `250`, `12k` */
fun formatHz(hz: Float): String = when {
    hz >= 10_000 -> "${(hz / 1000).roundToInt()}k"
    hz >= 1_000 -> "%.1fk".format(hz / 1000).replace(".0k", "k")
    else -> "${hz.roundToInt()}"
}

/** Bands that move vertically (pass filters have no gain). */
fun EqBandKind.hasGain(): Boolean = this != EqBandKind.LOW_PASS && this != EqBandKind.HIGH_PASS

/**
 * The frequency response on a log axis. With [onDrag] the band points can be dragged
 * (sideways = frequency, up and down = gain); a touch away from every point scrolls the
 * page as usual. [compare] draws a second, dashed curve (the "before" of a proposal, or
 * what plays with a taste layer). [curve] draws the main line from another profile than
 * the points' (a taste layer's tilt and bands), and [background] a faint line behind it
 * all (the output's own profile under a layer).
 */
@Composable
fun EqCurve(
    profile: EqProfile?,
    modifier: Modifier = Modifier,
    compare: EqProfile? = null,
    curve: EqProfile? = null,
    background: EqProfile? = null,
    maxRange: Float = 24f,
    selected: Int? = null,
    dimmed: Boolean = false,
    showPoints: Boolean = true,
    onSelect: (Int) -> Unit = {},
    onDrag: ((index: Int, freqHz: Float, gainDb: Float) -> Unit)? = null,
    onDragEnd: () -> Unit = {},
) {
    val scheme = MaterialTheme.colorScheme
    val response = remember(profile, curve) { responseOf(curve ?: profile) }
    val before = remember(compare) { compare?.let(::responseOf) }
    val behind = remember(background) { background?.let(::responseOf) }
    val peak = remember(response, before) {
        val all = response + before.orEmpty() + profile?.bands.orEmpty().map { it.gainDb }
        all.maxOfOrNull(::abs) ?: 0f
    }
    // the scale grows for big boosts and settles back smoothly
    val range by animateFloatAsState(max(12f, (peak + 3f).coerceAtMost(max(12f, maxRange))), LocalMotion.current.tween(250), label = "eq-range")
    val alpha by animateFloatAsState(if (dimmed) 0.4f else 1f, LocalMotion.current.tween(200), label = "eq-dim")
    val measurer = rememberTextMeasurer()
    val labelStyle = TextStyle(color = scheme.onSurfaceVariant, fontSize = 10.sp)
    val haptics = haptics()
    val latest by rememberUpdatedState(profile)
    val latestRange by rememberUpdatedState(range)
    val drag by rememberUpdatedState(onDrag)
    val select by rememberUpdatedState(onSelect)
    val end by rememberUpdatedState(onDragEnd)

    val gestures = if (onDrag == null) {
        Modifier
    } else {
        Modifier.pointerInput(Unit) {
            val reach = 28.dp.toPx()
            awaitEachGesture {
                val down = awaitFirstDown(requireUnconsumed = false)
                val bands = latest?.bands.orEmpty()
                val h = size.height.toFloat()
                val w = size.width.toFloat()
                val i = bands.indices
                    .map { idx ->
                        val b = bands[idx]
                        val p = Offset(freqToFraction(b.freqHz) * w, gainToY(if (b.kind.hasGain()) b.gainDb else 0f, h, latestRange))
                        idx to (p - down.position).getDistance()
                    }
                    .filter { it.second <= reach }
                    .minByOrNull { it.second }
                    ?.first ?: return@awaitEachGesture
                down.consume()
                select(i)
                haptics.tick()
                var moved = false
                while (true) {
                    val event = awaitPointerEvent()
                    val change = event.changes.firstOrNull { it.id == down.id } ?: break
                    if (!change.pressed) break
                    if (change.positionChanged()) {
                        change.consume()
                        moved = true
                        val freq = fractionToFreq(change.position.x / w)
                        val gain = yToGain(change.position.y, h, latestRange).coerceIn(-maxRange, maxRange)
                        drag?.invoke(i, freq, gain)
                    }
                }
                if (moved) end()
            }
        }
    }

    Canvas(modifier.then(gestures)) {
        val w = size.width
        val h = size.height
        // grid: decades and dB steps
        listOf(100f, 1_000f, 10_000f).forEach { hz ->
            val x = freqToFraction(hz) * w
            drawLine(scheme.outlineVariant, Offset(x, 0f), Offset(x, h), 1f)
            drawText(measurer, formatHz(hz), Offset(x + 3.dp.toPx(), h - 14.dp.toPx()), labelStyle)
        }
        val step = if (range > 15f) 12f else 6f
        var db = -((range / step).toInt() * step)
        while (db <= range) {
            val y = gainToY(db, h, range)
            drawLine(if (db == 0f) scheme.outline else scheme.outlineVariant, Offset(0f, y), Offset(w, y), if (db == 0f) 1.5f else 1f)
            if (db != 0f) drawText(measurer, "%+d".format(db.toInt()), Offset(3.dp.toPx(), y - 13.dp.toPx()), labelStyle)
            db += step
        }
        if (behind != null) {
            drawPath(curvePath(behind, w, h, range), scheme.onSurfaceVariant.copy(alpha = 0.35f), style = Stroke(1.5.dp.toPx()))
        }
        if (before != null) {
            drawPath(curvePath(before, w, h, range), scheme.onSurfaceVariant.copy(alpha = 0.7f), style = Stroke(2.dp.toPx(), pathEffect = PathEffect.dashPathEffect(floatArrayOf(10f, 8f))))
        }
        val path = curvePath(response, w, h, range)
        val zero = gainToY(0f, h, range)
        // the area between the curve and 0 dB, faint
        val fill = Path().apply {
            addPath(path)
            lineTo(w, zero)
            lineTo(0f, zero)
            close()
        }
        drawPath(fill, Brush.verticalGradient(listOf(scheme.primary.copy(alpha = 0.22f * alpha), scheme.primary.copy(alpha = 0.04f * alpha))))
        drawPath(path, scheme.primary.copy(alpha = alpha), style = Stroke(2.5.dp.toPx(), cap = StrokeCap.Round))
        if (showPoints) profile?.bands?.forEachIndexed { i, b -> drawPoint(b.freqHz, if (b.kind.hasGain()) b.gainDb else 0f, b.enabled, i == selected, w, h, range, scheme.primary, scheme.surface, alpha) }
    }
}

private fun DrawScope.drawPoint(
    freq: Float, gain: Float, enabled: Boolean, selected: Boolean,
    w: Float, h: Float, range: Float, color: Color, surface: Color, alpha: Float,
) {
    val c = Offset(freqToFraction(freq) * w, gainToY(gain, h, range))
    val r = (if (selected) 9 else 6).dp.toPx()
    if (selected) drawCircle(color.copy(alpha = 0.25f * alpha), r * 1.9f, c)
    drawCircle(if (enabled) color.copy(alpha = alpha) else surface, r, c)
    drawCircle(color.copy(alpha = alpha), r, c, style = Stroke(2.dp.toPx()))
}

private fun gainToY(db: Float, h: Float, range: Float): Float = h / 2f - (db / range).coerceIn(-1f, 1f) * (h / 2f - 6f)

private fun yToGain(y: Float, h: Float, range: Float): Float = ((h / 2f - y) / (h / 2f - 6f) * range).coerceIn(-range, range)

private fun curvePath(points: List<Float>, w: Float, h: Float, range: Float): Path {
    val path = Path()
    val n = points.size
    if (n == 0) return path
    points.forEachIndexed { i, db ->
        val x = i / (n - 1f) * w
        val y = gainToY(db, h, range)
        if (i == 0) path.moveTo(x, y) else path.lineTo(x, y)
    }
    return path
}
