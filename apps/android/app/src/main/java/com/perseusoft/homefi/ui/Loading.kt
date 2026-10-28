package com.perseusoft.homefi.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import com.perseusoft.homefi.ui.theme.LocalMotion
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update

/** Slow work in flight (search, Tidal pages, scans, starting a stream): drives the top loading bar. */
object Busy {
    private val _count = MutableStateFlow(0)
    val count: StateFlow<Int> = _count.asStateFlow()

    suspend fun <T> track(block: suspend () -> T): T {
        _count.update { it + 1 }
        try {
            return block()
        } finally {
            _count.update { it - 1 }
        }
    }

    /** For blocking callers (ExoPlayer's loading thread). */
    fun <T> trackBlocking(block: () -> T): T {
        _count.update { it + 1 }
        try {
            return block()
        } finally {
            _count.update { it - 1 }
        }
    }
}

/** Thin indeterminate bar shown under the top bar while something loads. */
@Composable
fun LoadingBar(visible: Boolean, modifier: Modifier = Modifier) {
    val motion = LocalMotion.current
    Box(modifier.fillMaxWidth().height(3.dp)) {
        // fades in and out rather than blinking on short loads
        AnimatedVisibility(visible, enter = fadeIn(motion.tween(200)), exit = fadeOut(motion.tween(300))) {
            LinearProgressIndicator(Modifier.fillMaxWidth().height(3.dp))
        }
    }
}

/** The sheen that sweeps across skeleton blocks (same light as the desktop's). */
fun Modifier.shimmer(shape: Shape = RoundedCornerShape(8.dp)): Modifier = composed {
    val base = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.07f)
    val shine = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.15f)
    // reduced motion: a still block, no looping sheen (as on the desktop)
    if (!LocalMotion.current.moves) return@composed clip(shape).background(base)
    val t by rememberInfiniteTransition(label = "shimmer").animateFloat(
        initialValue = -1f,
        targetValue = 2f,
        animationSpec = infiniteRepeatable(tween(1600, easing = LinearEasing), RepeatMode.Restart),
        label = "shimmer",
    )
    clip(shape).background(
        Brush.linearGradient(
            listOf(base, shine, base),
            start = Offset(t * 600f - 300f, 0f),
            end = Offset(t * 600f + 300f, 120f),
        ),
    )
}

@Composable
private fun SkeletonRow(index: Int, thumb: Dp, round: Boolean) {
    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(thumb).shimmer(if (round) CircleShape else RoundedCornerShape(8.dp)))
        Spacer(Modifier.width(16.dp))
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Box(Modifier.fillMaxWidth(0.45f + (index * 17 % 35) / 100f).height(14.dp).shimmer())
            Box(Modifier.fillMaxWidth(0.25f + (index * 11 % 25) / 100f).height(11.dp).shimmer())
        }
    }
}

/** Placeholder rows shaped like a track / result list. */
@Composable
fun SkeletonList(count: Int = 8, round: Boolean = false, modifier: Modifier = Modifier) {
    Column(modifier.semantics { contentDescription = "Loading" }) {
        repeat(count) { SkeletonRow(it, 52.dp, round) }
    }
}

/** Placeholder for an artist / album / playlist page: big cover, title, rows. */
@Composable
fun SkeletonDetail(round: Boolean = false, modifier: Modifier = Modifier) {
    Column(modifier.fillMaxWidth().semantics { contentDescription = "Loading" }, horizontalAlignment = Alignment.CenterHorizontally) {
        Spacer(Modifier.height(56.dp))
        Box(Modifier.size(150.dp).shimmer(if (round) CircleShape else RoundedCornerShape(16.dp)))
        Spacer(Modifier.height(16.dp))
        Box(Modifier.fillMaxWidth(0.6f).height(24.dp).shimmer())
        Spacer(Modifier.height(10.dp))
        Box(Modifier.fillMaxWidth(0.35f).height(14.dp).shimmer())
        Spacer(Modifier.height(20.dp))
        repeat(5) { SkeletonRow(it, 52.dp, false) }
    }
}
