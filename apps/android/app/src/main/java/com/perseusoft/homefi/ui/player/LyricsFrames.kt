package com.perseusoft.homefi.ui.player

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.perseusoft.homefi.R
import com.perseusoft.homefi.lyrics.Lyrics
import com.perseusoft.homefi.player.PlayerUiState
import com.perseusoft.homefi.ui.theme.LocalMotion
import com.perseusoft.homefi.ui.theme.LyricsPalette
import kotlin.math.abs
import kotlin.math.ceil
import kotlin.math.min
import kotlin.math.sin

// Timing and drawing of karaoke lines, shared by the phone's lyrics view, the picture-in-picture
// window and (on a TV) the tv/ screens: no touch handling lives here.

/** A break longer than this between two lines shows the notes and a countdown in karaoke. */
internal const val GAP_MS = 4_000L

/** A position report this far from the per-frame estimate moves the estimate (a seek, a stall). */
internal const val RESYNC_MS = 150L

/**
 * Where the song is for the lyrics, moved every frame while it plays: the player only
 * reports its position a few times a second. [offsetMs] > 0 shows the lyrics later.
 */
@Composable
internal fun rememberLyricsTime(state: PlayerUiState, offsetMs: Int): State<Long> {
    val time = remember { mutableLongStateOf(state.positionMs - offsetMs) }
    val sample = rememberUpdatedState(state.positionMs)
    val offset = rememberUpdatedState(offsetMs)
    LaunchedEffect(state.isPlaying, state.current?.mediaId) {
        if (!state.isPlaying) {
            snapshotFlow { sample.value - offset.value }.collect { time.longValue = it }
        } else {
            var anchorPos = sample.value
            var anchorNanos = -1L
            var last = sample.value
            while (true) {
                withFrameNanos { now ->
                    if (anchorNanos < 0) anchorNanos = now
                    var estimate = anchorPos + (now - anchorNanos) / 1_000_000
                    val s = sample.value
                    if (s != last) {
                        last = s
                        if (abs(s - estimate) > RESYNC_MS) {
                            anchorPos = s
                            anchorNanos = now
                            estimate = s
                        }
                    }
                    time.longValue = estimate - offset.value
                }
            }
        }
    }
    return time
}

/**
 * What karaoke shows at a moment: the line sung big ([line], -1 during a break), the one
 * before faint above, the one after small below, and when a break ends ([gapUntil]).
 */
@Immutable
internal data class KaraokeFrame(val line: Int, val previous: Int, val next: Int, val gapUntil: Long? = null)

internal fun karaokeFrame(l: Lyrics, t: Long): KaraokeFrame {
    val i = l.lineAt(t)
    if (i >= 0 && !l.lines[i].blank) {
        val next = l.nextSung(i)
        val end = l.endOf(i)
        // sung, and a long instrumental follows
        if (t > end && next >= 0 && l.timeOf(next) - end > GAP_MS) return KaraokeFrame(-1, i, next, l.timeOf(next))
        return KaraokeFrame(i, l.previousSung(i), next)
    }
    val next = l.nextSung(i)
    if (next < 0) return KaraokeFrame(-1, l.previousSung(l.lines.size), -1) // the song is over
    // before the first line, or a long break marked with an empty line
    if (i < 0 || l.timeOf(next) - l.timeOf(i) > GAP_MS) return KaraokeFrame(-1, l.previousSung(next), next, l.timeOf(next))
    // a short break: the next line waits, still unfilled
    return KaraokeFrame(next, l.previousSung(next), l.nextSung(next))
}

/** How much of line [index] has been sung at [t], 0..1. */
internal fun karaokeProgress(l: Lyrics, index: Int, t: Long): Float {
    val start = l.timeOf(index)
    if (t <= start) return 0f
    val end = l.endOf(index)
    return ((t - start).toFloat() / (end - start).coerceAtLeast(1)).coerceIn(0f, 1f)
}

/**
 * [text] in the dim colour, then again in the accent up to [progress] — across the wrapped
 * lines one after the other, with a soft leading edge. Drawn, not recomposed, every frame.
 */
@Composable
internal fun SungLine(text: String, style: TextStyle, palette: LyricsPalette, progress: () -> Float) {
    var layout by remember(text) { mutableStateOf<TextLayoutResult?>(null) }
    val soft = with(LocalDensity.current) { 20.dp.toPx() }
    Text(
        text,
        style = style,
        color = palette.dim,
        textAlign = TextAlign.Center,
        onTextLayout = { layout = it },
        modifier = Modifier.fillMaxWidth().drawWithContent {
            drawContent()
            val l = layout ?: return@drawWithContent
            val p = progress()
            if (p <= 0f) return@drawWithContent
            var total = 0f
            for (i in 0 until l.lineCount) total += l.getLineRight(i) - l.getLineLeft(i)
            var left = total * p
            for (i in 0 until l.lineCount) {
                if (left <= 0f) break
                val x0 = l.getLineLeft(i)
                val x1 = l.getLineRight(i)
                val w = x1 - x0
                val full = left >= w
                val edge = x0 + min(w, left)
                left -= w
                val brush = if (full) {
                    Brush.horizontalGradient(listOf(palette.fill, palette.fillEnd), startX = x0, endX = x1.coerceAtLeast(x0 + 1f))
                } else {
                    val end = edge + soft
                    Brush.horizontalGradient(
                        0f to palette.fill,
                        ((edge - x0) / (end - x0)) to palette.fillEnd,
                        1f to palette.fillEnd.copy(alpha = 0f),
                        startX = x0,
                        endX = end,
                    )
                }
                clipRect(x0 - soft, l.getLineTop(i), if (full) x1 + soft else edge + soft, l.getLineBottom(i)) {
                    // alpha 1: otherwise it takes the dim colour's alpha from the layout
                    drawText(l, brush = brush, alpha = 1f)
                }
            }
        },
    )
}

/** "♪ ♪ ♪" during a break, three dots going out one per second before the singing comes back. */
@Composable
internal fun Countdown(until: Long?, time: State<Long>, style: TextStyle, palette: LyricsPalette) {
    val dots by remember(until) {
        derivedStateOf { if (until == null) 0 else ceil((until - time.value) / 1000.0).toInt().coerceIn(0, 3) }
    }
    Text(
        stringResource(R.string.lyrics_gap),
        style = style,
        color = palette.fill,
        textAlign = TextAlign.Center,
        // a gentle bob, moved in the draw phase only
        modifier = Modifier.graphicsLayer { translationY = sin(time.value / 260.0).toFloat() * 4.dp.toPx() },
    )
    if (until != null) {
        Spacer(Modifier.height(12.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            repeat(3) { k ->
                Box(
                    Modifier
                        .size(10.dp)
                        .clip(CircleShape)
                        .background(if (k < dots) palette.fill else palette.dim.copy(alpha = palette.dim.alpha * 0.4f)),
                )
            }
        }
    }
}

/**
 * The lines of one karaoke [frame]: the one before faint above, the one being sung big (filling as
 * it goes) or the break's countdown, the next one small below. The old frame leaves before the new
 * one comes in, so they never overlap. Draws state only: no touch handling.
 */
@Composable
internal fun KaraokeLines(
    lyrics: Lyrics,
    frame: KaraokeFrame,
    time: State<Long>,
    palette: LyricsPalette,
    bigStyle: TextStyle,
    smallStyle: TextStyle,
    modifier: Modifier = Modifier,
) {
    val motion = LocalMotion.current
    AnimatedContent(
        targetState = frame,
        transitionSpec = {
            if (motion.moves) {
                (fadeIn(motion.tween(200, delay = 110)) + slideInVertically(motion.tween(320)) { it / 6 }) togetherWith fadeOut(motion.tween(110))
            } else {
                fadeIn(motion.tween(200, delay = 110)) togetherWith fadeOut(motion.tween(110))
            }
        },
        contentKey = { it.line to it.gapUntil },
        modifier = modifier,
        label = "karaoke",
    ) { s ->
        Column(Modifier.fillMaxSize().padding(horizontal = 12.dp), verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally) {
            if (s.previous >= 0) {
                Text(
                    lyrics.lines[s.previous].text,
                    style = smallStyle,
                    color = palette.dim.copy(alpha = palette.dim.alpha * 0.6f),
                    textAlign = TextAlign.Center,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
                Spacer(Modifier.height(16.dp))
            }
            if (s.line >= 0) {
                SungLine(lyrics.lines[s.line].text, bigStyle, palette) { karaokeProgress(lyrics, s.line, time.value) }
            } else {
                Countdown(s.gapUntil, time, bigStyle, palette)
            }
            if (s.next >= 0 && s.next != s.line) {
                Spacer(Modifier.height(16.dp))
                Text(
                    lyrics.lines[s.next].text,
                    style = smallStyle,
                    color = palette.dim,
                    textAlign = TextAlign.Center,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}
