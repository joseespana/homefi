package com.perseusoft.homefi.tv

import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.data.AmbientStyle
import com.perseusoft.homefi.data.ImmersiveShade
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.lyrics.LyricsState
import com.perseusoft.homefi.player.PlayerUiState
import com.perseusoft.homefi.ui.library.CoverArt
import com.perseusoft.homefi.ui.player.Backdrop
import com.perseusoft.homefi.ui.player.KaraokeLines
import com.perseusoft.homefi.ui.player.MascotView
import com.perseusoft.homefi.ui.player.coverModel
import com.perseusoft.homefi.ui.player.karaokeFrame
import com.perseusoft.homefi.ui.player.rememberLyricsTime
import com.perseusoft.homefi.ui.theme.LocalMotion
import com.perseusoft.homefi.ui.theme.LyricsPalette
import kotlinx.coroutines.delay

/** How long the group stays in one spot before it drifts to the next (burn-in protection). */
const val AMBIENT_DRIFT_SECONDS = 60L
private const val FADE_MS = 2_000
private const val MS_PER_SECOND = 1_000L
private val AMBIENT_COVER = 190.dp
private const val BREATH_MS = 6_000
private const val KEN_BURNS_MS = 40_000

/** The spots the group visits, as fractions of the free room (x, y): a slow walk around the screen. */
internal val DRIFT_SPOTS: List<Pair<Float, Float>> = listOf(0.5f to 0.5f, 0.2f to 0.3f, 0.8f to 0.65f, 0.3f to 0.85f, 0.75f to 0.2f)

/** The next spot's index after [index]. */
internal fun nextDriftSpot(index: Int): Int = (index + 1) % DRIFT_SPOTS.size

/**
 * The screensaver while music plays (docs/TV_UI.md 6.6): the cover's light dimmed to almost black, a big
 * cover with its title and a thin progress that drifts to a new spot every minute, cross-fading
 * through nothing so the TV panel never keeps one bright shape (burn-in). Any key wakes it
 * ([TvIdle] swallows that key). With motion OFF the group still moves, only without fading.
 */
@Composable
fun TvAmbient(state: PlayerUiState) {
    val app = LocalContext.current.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val motion = LocalMotion.current
    val look = LocalTvLook.current
    val style = settings.tv.ambientStyle
    var spot by remember { mutableIntStateOf(0) }
    var shown by remember { mutableIntStateOf(0) }
    val fade by animateFloatAsState(if (spot == shown) 1f else 0f, motion.tween(FADE_MS), label = "ambient-fade")
    // fade out, hop, fade in
    LaunchedEffect(Unit) {
        while (true) {
            delay(AMBIENT_DRIFT_SECONDS * MS_PER_SECOND)
            spot = nextDriftSpot(spot)
            if (motion.enabled) delay(FADE_MS.toLong())
            shown = spot
        }
    }
    val cover = state.current?.coverModel()

    Box(Modifier.fillMaxSize().background(Color(0xFF04050A))) {
        Box(Modifier.fillMaxSize().alpha(0.55f)) { Backdrop(cover, strong = false, shade = ImmersiveShade.OFF) }
        Box(Modifier.fillMaxSize().background(Color(0xCC04050A)))
        val safe = tvSafePadding()
        Box(Modifier.fillMaxSize().padding(safe)) {
            TvClock(
                Modifier.align(Alignment.TopEnd),
                style = TvType.display.copy(fontSize = 36.sp, fontWeight = FontWeight.Light),
            )
            Text(
                stringResource(R.string.tv_amb_wake),
                style = TvType.secondary,
                color = look.ink3,
                modifier = Modifier.align(Alignment.BottomCenter),
            )
            BoxWithConstraints(Modifier.fillMaxSize().padding(top = 60.dp, bottom = 30.dp)) {
                val groupWidth = if (style == AmbientStyle.CLOCK) 300.dp else 420.dp
                val groupHeight = if (style == AmbientStyle.CLOCK) 120.dp else 330.dp
                val (fx, fy) = DRIFT_SPOTS[shown]
                val x = (maxWidth - groupWidth).coerceAtLeast(0.dp) * fx
                val y = (maxHeight - groupHeight).coerceAtLeast(0.dp) * fy
                Box(Modifier.offset(x, y).width(groupWidth).alpha(if (motion.enabled) fade else 0.6f)) {
                    when (style) {
                        AmbientStyle.CLOCK -> TvClock(style = TvType.display.copy(fontSize = 72.sp, fontWeight = FontWeight.Light))
                        else -> AmbientGroup(state, cover, style)
                    }
                }
            }
        }
    }
}

@Composable
private fun AmbientGroup(state: PlayerUiState, cover: Any?, style: AmbientStyle) {
    val look = LocalTvLook.current
    val motion = LocalMotion.current
    val current = state.current
    // slow "breathing" of the glow and a gentle Ken Burns on the cover (only when motion is free)
    val breathe = rememberInfiniteTransition(label = "ambient")
    val glow by breathe.animateFloat(0.18f, 0.38f, infiniteRepeatable(tween(BREATH_MS, easing = FastOutSlowInEasing), RepeatMode.Reverse), label = "glow")
    val zoom by breathe.animateFloat(1f, 1.06f, infiniteRepeatable(tween(KEN_BURNS_MS, easing = LinearEasing), RepeatMode.Reverse), label = "zoom")
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(18.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(
                Modifier.drawBehind {
                    drawCircle(Brush.radialGradient(listOf(look.accent.copy(alpha = if (motion.moves) glow else 0.30f), Color.Transparent), center, size.maxDimension * 0.9f), radius = size.maxDimension * 0.9f)
                },
            ) {
                Box(Modifier.graphicsLayer { if (motion.moves) { scaleX = zoom; scaleY = zoom } }) { CoverArt(cover, AMBIENT_COVER, radius = 14.dp) }
            }
            Column(Modifier.weight(1f)) {
                Text(stringResource(R.string.tv_amb_now).uppercase(), style = TvType.caps, color = look.accent2, maxLines = 1)
                Text(current?.title.orEmpty(), style = TvType.display.copy(fontSize = 30.sp), color = look.ink, maxLines = 3, overflow = TextOverflow.Ellipsis)
                Text(current?.artist.orEmpty(), style = TvType.section.copy(fontWeight = FontWeight.Medium), color = look.ink2, maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
        }
        val fraction = if (state.durationMs > 0) state.positionMs.toFloat() / state.durationMs else 0f
        TvProgress(fraction, Modifier.alpha(0.7f).padding(top = 8.dp), height = 3.dp)
        when (style) {
            AmbientStyle.KARAOKE -> AmbientKaraoke(state)
            AmbientStyle.MASCOT -> Box(Modifier.fillMaxWidth().height(90.dp)) { MascotView(Modifier.fillMaxSize(), singing = false, danceButton = false) }
            else -> Unit
        }
    }
}

@Composable
private fun AmbientKaraoke(state: PlayerUiState) {
    val app = LocalContext.current.homefi
    val look = LocalTvLook.current
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val mediaId = state.current?.mediaId ?: return
    LaunchedEffect(mediaId) { app.lyrics.request(mediaId) }
    val all by app.lyrics.states.collectAsStateWithLifecycle()
    val ready = (all[mediaId] as? LyricsState.Ready)?.lyrics?.takeIf { it.synced } ?: return
    val time = rememberLyricsTime(state, settings.lyricsOffsetMs)
    val frame by remember(ready) { derivedStateOf { karaokeFrame(ready, time.value) } }
    val palette = remember(look) { LyricsPalette(ink = look.ink, dim = look.ink2.copy(alpha = 0.8f), fill = look.accent2, fillEnd = Color.White) }
    Box(Modifier.fillMaxWidth().height(110.dp)) {
        KaraokeLines(ready, frame, time, palette, TvType.display.copy(fontSize = 22.sp, lineHeight = 26.sp), TvType.body, Modifier.fillMaxSize())
    }
}
