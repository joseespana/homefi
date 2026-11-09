package com.perseusoft.homefi.tv

import android.text.format.DateFormat
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.MusicNote
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.OutputKind
import com.perseusoft.homefi.ui.library.CoverArt
import com.perseusoft.homefi.ui.theme.LocalMotion
import kotlinx.coroutines.delay
import java.util.Date

private const val MS_PER_MINUTE = 60_000L

/** The time of day in the user's format, updated at each minute. */
@Composable
fun rememberClock(): State<String> {
    val context = LocalContext.current
    val format = remember(context) { DateFormat.getTimeFormat(context) }
    val text = remember { mutableStateOf(format.format(Date())) }
    LaunchedEffect(format) {
        while (true) {
            val now = System.currentTimeMillis()
            text.value = format.format(Date(now))
            delay(MS_PER_MINUTE - now % MS_PER_MINUTE)
        }
    }
    return text
}

/** The clock as a text. */
@Composable
fun TvClock(modifier: Modifier = Modifier, style: androidx.compose.ui.text.TextStyle = TvType.section) {
    val time by rememberClock()
    Text(time, style = style, color = LocalTvLook.current.ink2, modifier = modifier)
}

/** What the audio goes out through, in words: HDMI is a system mix (never bit-perfect), the rest by name. */
@Composable
fun outputLabel(): String? {
    val app = LocalContext.current.homefi
    val output by app.outputs.current.collectAsStateWithLifecycle()
    val out = output ?: return null
    return if (out.kind == OutputKind.HDMI) stringResource(R.string.tv_out_hdmi) else out.name.takeIf { it.isNotBlank() }
}

/** A cover with its title and subtitle under it; focused, the cover grows with its ring and the label drops. */
@Composable
fun TvCoverCard(
    cover: Any?,
    title: String,
    subtitle: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    icon: ImageVector = Icons.Rounded.MusicNote,
    size: Dp = TvDimens.coverCard,
    /** Artists are circles. */
    round: Boolean = false,
    /** A row of the library that is not on the device now (its drive is out). */
    dimmed: Boolean = false,
    onLongClick: (() -> Unit)? = null,
    onFocused: (Boolean) -> Unit = {},
) {
    val motion = LocalMotion.current
    var focused by remember { mutableStateOf(false) }
    val drop by animateDpAsState(if (focused && motion.enabled) 7.dp else 0.dp, motion.tween(LABEL_DROP_MS), label = "tv-card-drop")
    val backdrop = LocalTvBackdropCover.current
    val bounds = remember { arrayOfNulls<androidx.compose.ui.geometry.Rect>(1) }
    val shape = if (round) CircleShape else RoundedCornerShape(TvDimens.cardRadius)
    Column(modifier.width(size).alpha(if (dimmed) 0.45f else 1f)) {
        TvFocusable(
            onClick = {
                backdrop.origin = bounds[0]
                onClick()
            },
            onLongClick = onLongClick,
            shape = shape,
            focusedScale = 1.08f,
            onFocused = {
                focused = it
                // the room takes the colour of the cover under the cursor
                if (it && cover != null) backdrop.cover = cover
                onFocused(it)
            },
        ) {
            CoverArt(
                cover, size, radius = if (round) size / 2 else TvDimens.cardRadius, icon = icon,
                modifier = Modifier.onGloballyPositioned { bounds[0] = it.boundsInRoot() },
            )
        }
        Spacer(Modifier.height(8.dp))
        Column(Modifier.offset(y = drop).fillMaxWidth()) {
            val align = if (round) androidx.compose.ui.text.style.TextAlign.Center else androidx.compose.ui.text.style.TextAlign.Start
            Text(title, style = TvType.cardTitle, color = if (focused) Color.White else LocalTvLook.current.ink, maxLines = 1, overflow = TextOverflow.Ellipsis, textAlign = align, modifier = Modifier.fillMaxWidth())
            if (subtitle.isNotBlank()) {
                Text(subtitle, style = TvType.secondary, color = LocalTvLook.current.ink2, maxLines = 1, overflow = TextOverflow.Ellipsis, textAlign = align, modifier = Modifier.fillMaxWidth())
            }
        }
    }
}

private const val LABEL_DROP_MS = 200
private const val PROGRESS_GLIDE_MS = 450

/** A thin progress bar in the accent gradient; [thumb] adds the glowing knob (the player's). */
@Composable
fun TvProgress(fraction: Float, modifier: Modifier = Modifier, thumb: Boolean = false, height: Dp = 6.dp) {
    val look = LocalTvLook.current
    val shape = RoundedCornerShape(height / 2)
    // the player reports its position a few times a second: glide between the reports
    val shownFraction by animateFloatAsState(fraction.coerceIn(0f, 1f), LocalMotion.current.tween(PROGRESS_GLIDE_MS), label = "tv-progress")
    Box(modifier.fillMaxWidth().height(height + if (thumb) 10.dp else 0.dp)) {
        Box(Modifier.fillMaxWidth().height(height).align(androidx.compose.ui.Alignment.CenterStart).background14(shape))
        Box(
            Modifier
                .fillMaxWidth(shownFraction)
                .height(height)
                .align(androidx.compose.ui.Alignment.CenterStart)
                .background(look.gradient, shape),
        )
        if (thumb) {
            Box(
                Modifier
                    .fillMaxWidth(shownFraction)
                    .align(androidx.compose.ui.Alignment.CenterStart),
            ) {
                Box(
                    Modifier
                        .align(androidx.compose.ui.Alignment.CenterEnd)
                        .offset(x = 8.dp)
                        .width(16.dp)
                        .height(16.dp)
                        .background(Color.White, androidx.compose.foundation.shape.CircleShape)
                        .border(3.dp, look.accent.copy(alpha = 0.6f), androidx.compose.foundation.shape.CircleShape),
                )
            }
        }
    }
}

private fun Modifier.background14(shape: androidx.compose.ui.graphics.Shape): Modifier =
    this.background(Color.White.copy(alpha = 0.14f), shape)

/** `m:ss` for a position in milliseconds. */
fun formatClock(ms: Long): String {
    val total = (ms / 1000).coerceAtLeast(0)
    val h = total / 3600
    val m = total % 3600 / 60
    val s = total % 60
    return if (h > 0) "%d:%02d:%02d".format(h, m, s) else "%d:%02d".format(m, s)
}
