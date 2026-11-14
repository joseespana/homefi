package com.perseusoft.homefi.ui.exercise

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shadow
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.CalloutKind
import com.perseusoft.homefi.core.RideCallout
import com.perseusoft.homefi.core.SegmentKind
import com.perseusoft.homefi.ui.theme.LocalMotion
import java.util.Locale
import kotlin.math.abs
import kotlin.math.min

// Kinetic type, like a title in a video edit: the words slam in big, upper case, with a hard shadow, while outlined
// copies of them burst out above and below and fade. One spec with the desktop (`RideCallout.svelte`) and the TV
// (`ride.html`): keep the numbers in sync.

/** The whole entrance (the echoes spreading and fading). */
private const val KINETIC_MS = 900
/** The share of it the words take to land. */
private const val SLAM_SHARE = 0.4f
/** The words start this big and land at 1. */
private const val SLAM_FROM = 1.45f
/** The echoes: copies at these steps above (-) and below (+), each [ECHO_SPREAD] of the line apart at the end. */
private val ECHO_STEPS = intArrayOf(-2, -1, 1, 2)
private const val ECHO_SPREAD = 0.8f
private const val ECHO_ALPHA = 1f
private const val ECHO_STROKE_DP = 2f
/** The hard shadow, a fraction of the type size down and right. */
private const val SHADOW_SHARE = 0.07f
private const val SCRIM_ALPHA = 0.55f
private const val COUNT_SCRIM_ALPHA = 0.75f
private const val FADE_MS = 160
private const val COUNT_DIGIT_SP = 180f
private const val COUNT_LABEL_SP = 30f
private const val SHORT_CHARS = 8
private const val MEDIUM_CHARS = 18
private const val SHORT_SP = 76f
private const val MEDIUM_SP = 52f
private const val LONG_SP = 36f

/** The cheers the core rotates through, by `n` (core `CHEERS`). */
private val CHEERS = intArrayOf(R.string.ex_cheer_0, R.string.ex_cheer_1, R.string.ex_cheer_2, R.string.ex_cheer_3)

/**
 * The big message in the middle of the ride (`RideView.callout`): "Sprint in 3 · 2 · 1" over a dimmed screen on the
 * countdown's beeps, then "GO!", cheers and the last seconds, and the ride's last words. Draws only; taps go through.
 */
@Composable
fun RideCalloutLayer(callout: RideCallout?, zone: Color, modifier: Modifier = Modifier) {
    val moves = LocalMotion.current.moves
    // the last message stays drawn while the layer fades out
    var shown by remember { mutableStateOf(callout) }
    if (callout != null) shown = callout
    AnimatedVisibility(
        visible = callout != null,
        modifier = modifier.fillMaxSize(),
        enter = if (moves) fadeIn(tween(FADE_MS)) else EnterTransition.None,
        exit = if (moves) fadeOut(tween(FADE_MS)) else ExitTransition.None,
    ) {
        val c = shown ?: return@AnimatedVisibility
        if (c.kind == CalloutKind.COUNTDOWN) Countdown(c.n.toInt()) else Message(c, zone)
    }
}

@Composable
private fun Countdown(digit: Int) {
    val sprint = SegmentKind.SPRINT.zoneColor()
    val label = stringResource(R.string.ex_callout_sprint_in)
    Box(
        Modifier
            .fillMaxSize()
            .background(Color.Black.copy(alpha = COUNT_SCRIM_ALPHA))
            .semantics { liveRegion = LiveRegionMode.Polite; contentDescription = "$label $digit" },
        contentAlignment = Alignment.Center,
    ) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            KineticText(label, sprint, COUNT_LABEL_SP, echoes = false)
            KineticText(digit.toString(), sprint, COUNT_DIGIT_SP)
        }
    }
}

@Composable
private fun Message(callout: RideCallout, zone: Color) {
    val n = callout.n.toInt()
    val sprinting = callout.kind == CalloutKind.GO || callout.kind == CalloutKind.CHEER || callout.kind == CalloutKind.SECONDS_LEFT
    val color = if (sprinting) SegmentKind.SPRINT.zoneColor() else zone
    val text = when (callout.kind) {
        CalloutKind.GO -> stringResource(R.string.ex_callout_go)
        CalloutKind.CHEER -> stringResource(CHEERS[n.mod(CHEERS.size)])
        CalloutKind.SECONDS_LEFT -> pluralStringResource(R.plurals.ex_callout_seconds_left, n, n)
        CalloutKind.GOOD_JOB -> stringResource(R.string.ex_callout_good_job)
        CalloutKind.ALMOST_DONE -> stringResource(R.string.ex_callout_almost)
        CalloutKind.STRETCH -> stringResource(R.string.ex_callout_stretch)
        CalloutKind.SEE_YOU -> stringResource(R.string.ex_callout_see_you)
        CalloutKind.COUNTDOWN -> n.toString()
    }
    val size = when {
        text.length <= SHORT_CHARS -> SHORT_SP
        text.length <= MEDIUM_CHARS -> MEDIUM_SP
        else -> LONG_SP
    }
    Box(
        Modifier
            .fillMaxSize()
            // a soft dark pool under the words: readable over the ring, the mountain or the lyrics
            .background(Brush.radialGradient(listOf(Color.Black.copy(alpha = SCRIM_ALPHA), Color.Transparent)))
            .padding(horizontal = 20.dp)
            .semantics { liveRegion = LiveRegionMode.Polite; contentDescription = text },
        contentAlignment = Alignment.Center,
    ) {
        KineticText(text, color, size)
    }
}

/**
 * [text] in upper case, [sizeSp] big, landing from [SLAM_FROM] with a hard shadow; with [echoes], outlined copies
 * burst out above and below and fade. A new [text] plays it again. Motion off: the words, still.
 */
@Composable
private fun KineticText(text: String, color: Color, sizeSp: Float, echoes: Boolean = true) {
    val moves = LocalMotion.current.moves
    val progress = remember(text) { Animatable(if (moves) 0f else 1f) }
    LaunchedEffect(text, moves) {
        if (moves) progress.animateTo(1f, tween(KINETIC_MS, easing = LinearEasing)) else progress.snapTo(1f)
    }
    val upper = text.uppercase(Locale.getDefault())
    val density = LocalDensity.current
    val sizePx = with(density) { sizeSp.sp.toPx() }
    val base = TextStyle(
        fontSize = sizeSp.sp, lineHeight = (sizeSp * 1.02f).sp, fontWeight = FontWeight.Black,
        textAlign = TextAlign.Center, letterSpacing = (-0.01f * sizeSp).sp,
    )
    val p = progress.value
    val slam = FastOutSlowInEasing.transform(min(1f, p / SLAM_SHARE))
    val spread = FastOutSlowInEasing.transform(p)
    Box(contentAlignment = Alignment.Center) {
        if (echoes && moves && p < 1f) {
            val outline = base.copy(drawStyle = Stroke(width = with(density) { ECHO_STROKE_DP.dp.toPx() }))
            for (step in ECHO_STEPS) {
                Text(
                    upper, style = outline, color = color,
                    modifier = Modifier.graphicsLayer {
                        translationY = step * ECHO_SPREAD * sizePx * spread
                        alpha = ECHO_ALPHA * (1f - p) / abs(step)
                    },
                )
            }
        }
        Text(
            upper,
            style = base.copy(shadow = Shadow(Color.Black, Offset(sizePx * SHADOW_SHARE, sizePx * SHADOW_SHARE), 0f)),
            color = color,
            modifier = Modifier.graphicsLayer {
                val scale = SLAM_FROM + (1f - SLAM_FROM) * slam
                scaleX = scale
                scaleY = scale
                alpha = min(1f, slam * 2f)
            },
        )
    }
}
