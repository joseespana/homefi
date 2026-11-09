package com.perseusoft.homefi.ui.dj

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.dp
import com.perseusoft.homefi.R
import com.perseusoft.homefi.player.dj.DjDeckUi
import com.perseusoft.homefi.player.dj.DjOverview
import com.perseusoft.homefi.player.dj.formatClock

private val BEAT_MIN_SPACING = 10.dp
private val PLAYHEAD_WIDTH = 2.dp
private val MARKER_WIDTH = 2.dp
private const val UPCOMING_ALPHA = 0.45f
private const val BEAT_ALPHA = 0.25f
private const val LOOP_ALPHA = 0.25f
private const val MIN_BAR_HEIGHT = 0.04f

/**
 * The song's overview: one bar per slice, coloured by bass / voice / treble (or one colour),
 * the beat lines, the cue and hot cues, the loop and the playhead. Tap or drag to jump.
 */
@Composable
fun DjWaveform(
    deck: DjDeckUi,
    accent: Color,
    colored: Boolean,
    showBeats: Boolean,
    onSeek: (Float) -> Unit,
    modifier: Modifier = Modifier,
) {
    val scheme = MaterialTheme.colorScheme
    val bands = BandColors(scheme.primary, scheme.secondary, scheme.tertiary)
    val ink = LocalDjLook.current?.playhead ?: scheme.onSurface
    val duration = deck.durationMs.takeIf { it > 0 } ?: deck.analysis?.durationMs?.toLong() ?: 0L
    val seek by rememberUpdatedState(onSeek)
    val minSpacing = with(LocalDensity.current) { BEAT_MIN_SPACING.toPx() }
    val playhead = with(LocalDensity.current) { PLAYHEAD_WIDTH.toPx() }
    val marker = with(LocalDensity.current) { MARKER_WIDTH.toPx() }
    val description = stringResource(R.string.dj_overview)
    val position = stringResource(R.string.dj_position, formatClock(deck.positionMs), formatClock(duration))
    Box(
        modifier
            .semantics {
                contentDescription = description
                stateDescription = position
            }
            .pointerInput(Unit) { detectTapGestures { seek((it.x / size.width).coerceIn(0f, 1f)) } }
            .pointerInput(Unit) { detectHorizontalDragGestures { change, _ -> seek((change.position.x / size.width).coerceIn(0f, 1f)) } },
    ) {
        Canvas(Modifier.fillMaxSize()) {
            val overview = deck.overview
            val played = if (duration > 0) (deck.positionMs.toFloat() / duration).coerceIn(0f, 1f) else 0f
            if (overview != null) drawBars(overview, played, accent, bands, colored)
            if (duration > 0) {
                if (showBeats) deck.analysis?.grid?.let { drawBeats(it.firstBeatMs, it.beatMs, duration, minSpacing, ink.copy(alpha = BEAT_ALPHA)) }
                deck.loopBeats?.let {
                    val from = deck.loopStartMs.toFloat() / duration * size.width
                    val to = deck.loopEndMs.toFloat() / duration * size.width
                    drawRect(accent.copy(alpha = LOOP_ALPHA), Offset(from, 0f), Size((to - from).coerceAtLeast(marker), size.height))
                }
                drawMarker(deck.cueMs, duration, marker, scheme.error)
                deck.hotCues.forEach { cue -> if (cue != null) drawMarker(cue, duration, marker, accent) }
                drawRect(ink, Offset(played * size.width - playhead / 2, 0f), Size(playhead, size.height))
            }
        }
    }
}

private class BandColors(val low: Color, val mid: Color, val high: Color)

private fun DrawScope.drawBars(overview: DjOverview, played: Float, accent: Color, bands: BandColors, colored: Boolean) {
    val bars = overview.bars
    val barWidth = size.width / bars
    val middle = size.height / 2
    for (i in 0 until bars) {
        val h = overview.peak[i].coerceAtLeast(MIN_BAR_HEIGHT) * size.height
        val base = if (colored) bandColor(overview, i, bands) else accent
        val color = if ((i + 0.5f) / bars <= played) base else base.copy(alpha = UPCOMING_ALPHA)
        drawRect(color, Offset(i * barWidth, middle - h / 2), Size(barWidth.coerceAtLeast(1f), h))
    }
}

/** The bar's colour: bass, voice and treble blended by how much energy each has. */
private fun bandColor(o: DjOverview, i: Int, bands: BandColors): Color {
    val total = o.low[i] + o.mid[i] + o.high[i]
    if (total <= 0f) return bands.mid
    val lowMid = lerp(bands.low, bands.mid, o.mid[i] / (o.low[i] + o.mid[i]).coerceAtLeast(Float.MIN_VALUE))
    return lerp(lowMid, bands.high, o.high[i] / total)
}

private fun DrawScope.drawBeats(firstMs: Double, beatMs: Double, durationMs: Long, minSpacing: Float, color: Color) {
    if (beatMs <= 0) return
    val pxPerBeat = (beatMs / durationMs * size.width).toFloat()
    if (pxPerBeat <= 0f) return
    // show every 2nd, 4th, 8th ... beat until the lines are far enough apart
    var every = 1
    while (pxPerBeat * every < minSpacing) every *= 2
    var beat = 0
    while (true) {
        val x = ((firstMs + beat * beatMs) / durationMs * size.width).toFloat()
        if (x > size.width) break
        drawLine(color, Offset(x, 0f), Offset(x, size.height))
        beat += every
    }
}

private fun DrawScope.drawMarker(ms: Long, durationMs: Long, width: Float, color: Color) {
    val x = ms.toFloat() / durationMs * size.width
    drawRect(color, Offset(x - width / 2, 0f), Size(width, size.height))
}
