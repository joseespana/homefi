package com.perseusoft.homefi.ui.player

import androidx.annotation.OptIn
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.media3.common.util.UnstableApi
import com.perseusoft.homefi.player.AudioLevels
import com.perseusoft.homefi.player.AudioTap
import com.perseusoft.homefi.ui.theme.Brand
import kotlin.math.cos
import kotlin.math.max
import kotlin.math.sin

/** Live levels while the view is on screen; the tap does no work otherwise. */
@OptIn(UnstableApi::class)
@Composable
internal fun rememberLevels(): AudioLevels {
    DisposableEffect(Unit) {
        AudioTap.watchers++
        onDispose { AudioTap.watchers-- }
    }
    val levels by AudioTap.levels.collectAsStateWithLifecycle()
    return levels
}

private val spectrum = listOf(Brand.Blue, Brand.Teal, Brand.Green)

/** Spectrum analyser: 32 log-spaced bands with falling peak caps. */
@Composable
fun SpectrumView(modifier: Modifier = Modifier) {
    val levels = rememberLevels()
    val peaks = remember { FloatArray(AudioTap.BANDS) }
    val grid = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.08f)
    val cap = MaterialTheme.colorScheme.onSurface
    Canvas(modifier.fillMaxSize().padding(8.dp)) {
        val n = levels.bands.size
        val gap = size.width * 0.25f / n
        val w = (size.width - gap * (n - 1)) / n
        for (i in 1..4) {
            val y = size.height * i / 5
            drawLine(grid, Offset(0f, y), Offset(size.width, y), strokeWidth = 1f)
        }
        val brush = Brush.verticalGradient(spectrum.reversed(), startY = 0f, endY = size.height)
        levels.bands.forEachIndexed { i, v ->
            peaks[i] = max(v, peaks[i] - 0.012f)
            val x = i * (w + gap)
            val h = max(2f, v * size.height)
            drawRoundRect(brush, Offset(x, size.height - h), Size(w, h), CornerRadius(w / 3))
            val py = size.height - peaks[i] * size.height - 4f
            drawRect(cap.copy(alpha = 0.8f), Offset(x, py), Size(w, 3f))
        }
    }
}

/** Two analogue-style VU meters (left / right). */
@Composable
fun VuMetersView(modifier: Modifier = Modifier) {
    val levels = rememberLevels()
    // needle ballistics: ~300 ms rise like a real VU (plain floats: no state writes while composing)
    val needles = remember { FloatArray(2) }
    needles[0] += (levels.left - needles[0]) * 0.35f
    needles[1] += (levels.right - needles[1]) * 0.35f
    val (left, right) = needles
    val face = MaterialTheme.colorScheme.surfaceContainerHigh
    val ink = MaterialTheme.colorScheme.onSurface
    val hot = MaterialTheme.colorScheme.error
    Box(modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Canvas(Modifier.fillMaxWidth().padding(8.dp).then(Modifier.size(320.dp))) {
            val meterH = size.height / 2 - 12f
            listOf(left, right).forEachIndexed { m, v ->
                val top = m * (meterH + 24f)
                drawRoundRect(face, Offset(0f, top), Size(size.width, meterH), CornerRadius(24f))
                val pivot = Offset(size.width / 2, top + meterH * 0.95f)
                val r = meterH * 0.78f
                // scale from -150° (left) to -30° (right); the last fifth is the red zone
                for (t in 0..20) {
                    val f = t / 20f
                    val a = Math.toRadians((-150 + 120 * f).toDouble())
                    val inner = r * if (t % 5 == 0) 0.84f else 0.9f
                    drawLine(
                        if (f > 0.8f) hot else ink.copy(alpha = 0.6f),
                        Offset(pivot.x + inner * cos(a).toFloat(), pivot.y + inner * sin(a).toFloat()),
                        Offset(pivot.x + r * cos(a).toFloat(), pivot.y + r * sin(a).toFloat()),
                        strokeWidth = if (t % 5 == 0) 4f else 2f,
                    )
                }
                drawArc(hot.copy(alpha = 0.25f), -54f, 24f, false, Offset(pivot.x - r, pivot.y - r), Size(2 * r, 2 * r), style = Stroke(10f))
                val a = Math.toRadians((-150 + 120 * v).toDouble())
                drawLine(
                    Brand.Teal,
                    pivot,
                    Offset(pivot.x + r * 1.02f * cos(a).toFloat(), pivot.y + r * 1.02f * sin(a).toFloat()),
                    strokeWidth = 5f,
                    cap = StrokeCap.Round,
                )
                drawCircle(ink, 10f, pivot)
            }
        }
    }
}
