package com.perseusoft.homefi.ui.player

import androidx.annotation.OptIn
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.media3.common.util.UnstableApi
import com.perseusoft.homefi.data.ProgressStyle
import com.perseusoft.homefi.data.ProgressThickness
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.PlayerUiState
import com.perseusoft.homefi.ui.formatDuration
import com.perseusoft.homefi.ui.theme.Brand
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.sin

private fun ProgressThickness.px(density: Float): Float = when (this) {
    ProgressThickness.THIN -> 3f
    ProgressThickness.MEDIUM -> 5f
    ProgressThickness.THICK -> 8f
} * density

/**
 * The player's seek bar in the style picked in Settings. Drawn by hand, so the
 * wavy and other new looks work on any Android version. Tap or drag to seek.
 */
@Composable
fun StyledSeekBar(state: PlayerUiState, modifier: Modifier = Modifier) {
    val app = LocalContext.current.homefi
    val player = playerSource()
    val canSeek = playerCapabilities().canSeek
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    var seeking by remember { mutableStateOf<Float?>(null) }
    val duration = state.durationMs.coerceAtLeast(1)
    val fraction = seeking ?: (state.positionMs.toFloat() / duration).coerceIn(0f, 1f)
    Column(modifier.fillMaxWidth()) {
        ProgressTrack(
            fraction = fraction,
            style = settings.progressStyle,
            thickness = settings.progressThickness,
            animate = settings.progressAnimated && state.isPlaying && seeking == null,
            seed = state.current?.mediaId.hashCode(),
            modifier = Modifier
                .fillMaxWidth()
                .height(32.dp)
                // a guest watches the hub's position; only the hub seeks
                .then(if (!canSeek) Modifier else Modifier
                    .pointerInput(duration) {
                        detectTapGestures { o -> player.seekTo((o.x / size.width).coerceIn(0f, 1f).times(duration).toLong()) }
                    }
                    .pointerInput(duration) {
                        detectHorizontalDragGestures(
                            onDragStart = { o -> seeking = (o.x / size.width).coerceIn(0f, 1f) },
                            onDragEnd = {
                                seeking?.let { player.seekTo((it * duration).toLong()) }
                                seeking = null
                            },
                            onDragCancel = { seeking = null },
                            onHorizontalDrag = { change, _ -> seeking = (change.position.x / size.width).coerceIn(0f, 1f) },
                        )
                    }),
        )
        Row(Modifier.fillMaxWidth()) {
            val shown = seeking?.let { (it * duration).toLong() } ?: state.positionMs
            Text(formatDuration(shown), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.weight(1f))
            Text(formatDuration(state.durationMs), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

/** The bar alone: also used for the previews in Settings. */
@OptIn(UnstableApi::class)
@Composable
fun ProgressTrack(
    fraction: Float,
    style: ProgressStyle,
    thickness: ProgressThickness,
    animate: Boolean,
    seed: Int,
    modifier: Modifier = Modifier,
) {
    val played = MaterialTheme.colorScheme.primary
    val rest = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.18f)
    // the wave flattens smoothly when playback pauses
    val amplitude by animateFloatAsState(if (animate) 1f else 0f, tween(400), label = "amp")
    val phase by rememberInfiniteTransition(label = "phase").animateFloat(
        0f,
        2 * PI.toFloat(),
        infiniteRepeatable(tween(1400, easing = LinearEasing), RepeatMode.Restart),
        label = "phase",
    )
    val levels = if (style == ProgressStyle.WAVEFORM && animate) rememberLevels() else null
    val heights = remember(seed) {
        val r = java.util.Random(seed.toLong())
        FloatArray(64) { 0.25f + r.nextFloat() * 0.75f }
    }
    Canvas(modifier) {
        val t = thickness.px(density)
        val y = size.height / 2
        val x = size.width * fraction
        when (style) {
            ProgressStyle.CLASSIC -> {
                line(rest, 0f, size.width, y, t)
                line(played, 0f, x, y, t)
                drawCircle(played, t * 1.6f + 3 * density, Offset(x, y))
            }
            ProgressStyle.WAVY -> {
                line(rest, x, size.width, y, t)
                val wave = Path()
                val amp = 4 * density * amplitude
                val length = 26 * density
                var px = 0f
                wave.moveTo(0f, y)
                while (px <= x) {
                    wave.lineTo(px, y + amp * sin(px / length * 2 * PI.toFloat() - phase))
                    px += 2f
                }
                drawPath(wave, played, style = Stroke(t, cap = StrokeCap.Round))
                drawRoundRect(played, Offset(x - t / 2, y - 9 * density), Size(t, 18 * density), CornerRadius(t))
            }
            ProgressStyle.WAVEFORM -> {
                val n = heights.size
                val w = size.width / n
                for (i in 0 until n) {
                    val cx = i * w + w / 2
                    // bars near the playhead breathe with the music
                    val near = 1f - (abs(cx - x) / (size.width * 0.15f)).coerceIn(0f, 1f)
                    val live = levels?.bands?.get(i % levels.bands.size) ?: 0f
                    val h = (heights[i] * (1f - near * 0.4f) + live * near * 0.8f).coerceIn(0.1f, 1f) * size.height * 0.9f
                    drawRoundRect(
                        if (cx <= x) played else rest,
                        Offset(cx - w * 0.3f, y - h / 2),
                        Size(w * 0.6f, h),
                        CornerRadius(w * 0.3f),
                    )
                }
            }
            ProgressStyle.NEON -> {
                line(rest, 0f, size.width, y, t)
                val gradient = Brush.horizontalGradient(listOf(Brand.Blue, Brand.Teal, Brand.Green), 0f, size.width)
                drawLine(gradient, Offset(0f, y), Offset(x, y), t * 3f, StrokeCap.Round, alpha = 0.25f) // the glow
                drawLine(gradient, Offset(0f, y), Offset(x, y), t, StrokeCap.Round)
                val pulse = if (animate) 0.7f + 0.3f * sin(phase * 2) else 1f
                drawCircle(Brand.Green.copy(alpha = 0.35f * pulse), t * 4f, Offset(x, y))
                drawCircle(Color.White, t * 1.4f, Offset(x, y))
            }
            ProgressStyle.LED -> {
                val segments = 40
                val gap = 2 * density
                val w = (size.width - gap * (segments - 1)) / segments
                val h = t * 2.2f
                for (i in 0 until segments) {
                    val lit = (i + 0.5f) / segments <= fraction
                    val c = when {
                        !lit -> rest
                        i > segments * 0.85f -> Color(0xFFE5484D)
                        i > segments * 0.65f -> Color(0xFFE5C04D)
                        else -> Color(0xFF3DD68C)
                    }
                    drawRect(c, Offset(i * (w + gap), y - h / 2), Size(w, h))
                }
            }
            ProgressStyle.MINIMAL -> {
                line(rest, 0f, size.width, y, t * 0.6f)
                line(played, 0f, x, y, t * 0.6f)
            }
        }
    }
}

private fun DrawScope.line(color: Color, from: Float, to: Float, y: Float, width: Float) {
    if (to > from) drawLine(color, Offset(from, y), Offset(to, y), width, StrokeCap.Round)
}
