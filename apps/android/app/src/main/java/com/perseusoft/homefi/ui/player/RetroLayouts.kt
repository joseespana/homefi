package com.perseusoft.homefi.ui.player

import com.perseusoft.homefi.ui.QueueMenu
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.material3.IconButton
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.animation.core.tween
import androidx.activity.compose.BackHandler
import android.os.SystemClock
import kotlin.math.sign
import kotlin.math.roundToInt
import kotlin.math.abs
import kotlinx.coroutines.launch
import com.perseusoft.homefi.library.albums
import com.perseusoft.homefi.library.albumKey
import com.perseusoft.homefi.core.LibraryTrack
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.input.pointer.util.VelocityTracker
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.Animatable
import androidx.annotation.OptIn
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.basicMarquee
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Repeat
import androidx.compose.material.icons.rounded.RepeatOne
import androidx.compose.material.icons.rounded.Shuffle
import androidx.compose.material.icons.rounded.SkipNext
import androidx.compose.material.icons.rounded.SkipPrevious
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shadow
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import com.perseusoft.homefi.R
import com.perseusoft.homefi.data.PlayerLayout
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.PlayerUiState
import com.perseusoft.homefi.player.QueueEntry
import com.perseusoft.homefi.ui.formatDuration
import com.perseusoft.homefi.ui.library.CoverArt
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.sin

// Skins in the spirit of the players of the early 2000s. Only the look is
// borrowed (colors, bevels, LCDs); no names or logos.

/** Background and top-bar ink of each retro skin. */
internal fun retroBackground(layout: PlayerLayout): Brush? = when (layout) {
    PlayerLayout.RETRO_AMP -> Brush.verticalGradient(listOf(Color(0xFF34344A), Color(0xFF1E1E2C)))
    PlayerLayout.LUNA -> Brush.verticalGradient(listOf(Color(0xFF7FA6EE), Color(0xFF2E5BD0), Color(0xFF1B3A93)))
    PlayerLayout.ALUMINUM -> Brush.verticalGradient(listOf(Color(0xFFE6E6E6), Color(0xFFBDBDBD)))
    PlayerLayout.WEB2000 -> Brush.verticalGradient(listOf(Color(0xFF3C3C3F), Color(0xFF3C3C3F)))
    else -> null
}

internal fun retroInk(layout: PlayerLayout): Color? = when (layout) {
    PlayerLayout.RETRO_AMP -> Color(0xFFCFCFE0)
    PlayerLayout.LUNA -> Color.White
    PlayerLayout.ALUMINUM -> Color(0xFF333333)
    PlayerLayout.WEB2000 -> Color(0xFFEDEDED)
    else -> null
}

/** Raised (or sunken) 3-D edge, like a 1999 window. */
private fun Modifier.bevel(face: Color, light: Color, dark: Color, sunken: Boolean = false, width: Float = 3f) = this
    .background(face)
    .drawBehind {
        val (top, bottom) = if (sunken) dark to light else light to dark
        drawRect(top, Offset.Zero, Size(size.width, width))
        drawRect(top, Offset.Zero, Size(width, size.height))
        drawRect(bottom, Offset(0f, size.height - width), Size(size.width, width))
        drawRect(bottom, Offset(size.width - width, 0f), Size(width, size.height))
    }

private fun qualityParts(quality: String?): Pair<String, String> {
    val q = quality.orEmpty()
    val khz = Regex("""([\d.]+)\s*kHz""").find(q)?.groupValues?.get(1) ?: "44.1"
    val bits = Regex("""(\d+)-bit""").find(q)?.groupValues?.get(1) ?: "16"
    return bits to khz
}

// ===================================================================== Retro Amp

private val AmpFace = Color(0xFF2E2E40)
private val AmpLight = Color(0xFF5C5C78)
private val AmpDark = Color(0xFF12121A)
private val Lcd = Color(0xFF000000)
private val LcdGreen = Color(0xFF00E000)
private val AmpFont = FontFamily.Monospace

@Composable
private fun AmpTitleBar(text: String) {
    Box(
        Modifier
            .fillMaxWidth()
            .height(16.dp)
            .background(Brush.horizontalGradient(listOf(Color(0xFF3A3A55), Color(0xFF8A7A3A), Color(0xFF3A3A55)))),
        contentAlignment = Alignment.Center,
    ) {
        Text(text, color = Color(0xFFE8E0B0), fontSize = 9.sp, fontFamily = AmpFont, fontWeight = FontWeight.Bold, letterSpacing = 2.sp)
    }
}

@Composable
private fun AmpPanel(title: String, modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    Column(modifier.fillMaxWidth().bevel(AmpFace, AmpLight, AmpDark).padding(3.dp)) {
        AmpTitleBar(title)
        Column(Modifier.padding(6.dp), content = content)
    }
}

@OptIn(UnstableApi::class)
@Composable
private fun AmpSpectrum(modifier: Modifier) {
    val levels = rememberLevels()
    Canvas(modifier) {
        val n = 19
        val w = size.width / n
        for (i in 0 until n) {
            val v = (levels.bands[i * levels.bands.size / n] + levels.bands[(i * levels.bands.size / n + 1).coerceAtMost(levels.bands.size - 1)]) / 2
            val segs = 8
            val lit = (v * segs).toInt()
            for (s in 0 until lit) {
                val c = when {
                    s >= 6 -> Color(0xFFE03020)
                    s >= 4 -> Color(0xFFE0D020)
                    else -> Color(0xFF20C020)
                }
                val h = size.height / segs
                drawRect(c, Offset(i * w + 1, size.height - (s + 1) * h + 1), Size(w - 2, h - 2))
            }
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
internal fun ColumnScope.RetroAmpLayout(state: PlayerUiState) {
    val player = playerSource()
    val caps = playerCapabilities()
    val current = state.current
    val (bits, khz) = qualityParts(current?.quality)
    val lcdTime = TextStyle(
        color = LcdGreen,
        fontFamily = AmpFont,
        fontWeight = FontWeight.Bold,
        fontSize = 34.sp,
        shadow = Shadow(LcdGreen.copy(alpha = 0.6f), blurRadius = 8f),
    )
    AmpPanel("H O M E F I") {
        Row {
            // the LCD: play state, time, spectrum
            Column(Modifier.width(132.dp).bevel(Lcd, AmpDark, AmpLight, sunken = true, width = 2f).padding(6.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Icon(if (state.isPlaying) Icons.Rounded.PlayArrow else Icons.Rounded.Pause, null, tint = LcdGreen, modifier = Modifier.size(14.dp))
                    Spacer(Modifier.width(4.dp))
                    Text(formatDuration(state.positionMs), style = lcdTime)
                }
                AmpSpectrum(Modifier.fillMaxWidth().height(30.dp))
            }
            Spacer(Modifier.width(6.dp))
            Column(Modifier.weight(1f)) {
                Box(Modifier.fillMaxWidth().bevel(Lcd, AmpDark, AmpLight, sunken = true, width = 2f).padding(horizontal = 6.dp, vertical = 4.dp)) {
                    Text(
                        "${state.index + 1}. ${current?.artist.orEmpty()} - ${current?.title.orEmpty()} (${formatDuration(state.durationMs)})",
                        color = LcdGreen,
                        fontFamily = AmpFont,
                        fontSize = 12.sp,
                        maxLines = 1,
                        modifier = Modifier.basicMarquee(iterations = Int.MAX_VALUE),
                    )
                }
                Spacer(Modifier.height(6.dp))
                Row(verticalAlignment = Alignment.CenterVertically) {
                    LcdBox(bits)
                    Text(" bit", color = Color(0xFFB0B0C8), fontFamily = AmpFont, fontSize = 10.sp)
                    Spacer(Modifier.width(6.dp))
                    LcdBox(khz)
                    Text(" kHz", color = Color(0xFFB0B0C8), fontFamily = AmpFont, fontSize = 10.sp)
                }
                Spacer(Modifier.height(4.dp))
                Text("stereo", color = LcdGreen, fontFamily = AmpFont, fontSize = 10.sp, modifier = Modifier.align(Alignment.End))
            }
        }
        Spacer(Modifier.height(8.dp))
        AmpSeek(state, caps.canSeek) { player.seekTo(it) }
        Spacer(Modifier.height(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(3.dp), verticalAlignment = Alignment.CenterVertically) {
            AmpButton(Icons.Rounded.SkipPrevious, R.string.player_previous, caps.canPrevious, player::previous)
            AmpButton(Icons.Rounded.PlayArrow, R.string.player_play, enabled = caps.canPlayPause) { if (!state.isPlaying) player.togglePlayPause() }
            AmpButton(Icons.Rounded.Pause, R.string.player_pause, enabled = caps.canPlayPause) { if (state.isPlaying) player.togglePlayPause() }
            AmpButton(Icons.Rounded.SkipNext, R.string.player_next, caps.canNext, player::next)
            Spacer(Modifier.weight(1f))
            AmpToggle("SHUF", state.shuffle, player::toggleShuffle, caps.canShuffleRepeat)
            AmpToggle(if (state.repeatMode == Player.REPEAT_MODE_ONE) "REP1" else "REP", state.repeatMode != Player.REPEAT_MODE_OFF, player::cycleRepeat, caps.canShuffleRepeat)
        }
    }
    Spacer(Modifier.height(4.dp))
    AmpPanel("S P E C T R U M") { AmpEq() }
    Spacer(Modifier.height(4.dp))
    AmpPanel("P L A Y L I S T", Modifier.weight(1f)) {
        AmpPlaylist(state, Modifier.fillMaxSize())
    }
    Spacer(Modifier.height(8.dp))
}

@Composable
private fun LcdBox(text: String) {
    Box(Modifier.bevel(Lcd, AmpDark, AmpLight, sunken = true, width = 2f).padding(horizontal = 4.dp, vertical = 1.dp)) {
        Text(text, color = LcdGreen, fontFamily = AmpFont, fontSize = 11.sp, fontWeight = FontWeight.Bold)
    }
}

@Composable
private fun AmpSeek(state: PlayerUiState, enabled: Boolean, onSeek: (Long) -> Unit) {
    val f = if (state.durationMs > 0) state.positionMs.toFloat() / state.durationMs else 0f
    BoxWithConstraints(
        Modifier
            .fillMaxWidth()
            .height(14.dp)
            .bevel(Color(0xFF1A1A26), AmpDark, AmpLight, sunken = true, width = 2f)
            .clickable {},
    ) {
        val w = maxWidth
        Box(
            Modifier
                .offset(x = (w - 28.dp) * f)
                .width(28.dp)
                .fillMaxHeight()
                .bevel(Color(0xFFC8A848), Color(0xFFF0E0A0), Color(0xFF6A5418), width = 2f),
        )
        // tap anywhere on the groove to jump there
        Row(Modifier.fillMaxSize()) {
            repeat(20) { i ->
                Box(Modifier.weight(1f).fillMaxHeight().clickable(enabled = enabled) { onSeek((state.durationMs * (i + 0.5f) / 20).toLong()) })
            }
        }
    }
}

@Composable
private fun RowScope.AmpButton(icon: ImageVector, label: Int, enabled: Boolean = true, onClick: () -> Unit) {
    Box(
        Modifier.size(40.dp, 30.dp).bevel(Color(0xFFB8B8C8), Color(0xFFF0F0FA), Color(0xFF55556A), width = 2f).clickable(enabled = enabled, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, stringResource(label), tint = Color(0xFF22222E).copy(alpha = if (enabled) 1f else DISABLED_ALPHA), modifier = Modifier.size(20.dp))
    }
}

@Composable
private fun AmpToggle(text: String, on: Boolean, onClick: () -> Unit, enabled: Boolean = true) {
    Row(
        Modifier
            .bevel(Color(0xFFB8B8C8), Color(0xFFF0F0FA), Color(0xFF55556A), width = 2f)
            .clickable(enabled = enabled, onClick = onClick)
            .alpha(if (enabled) 1f else DISABLED_ALPHA)
            .padding(horizontal = 6.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(6.dp).background(if (on) Color(0xFF30F030) else Color(0xFF2A4A2A)))
        Spacer(Modifier.width(4.dp))
        Text(text, color = Color(0xFF22222E), fontFamily = AmpFont, fontSize = 10.sp, fontWeight = FontWeight.Bold)
    }
}

/** Ten faders riding the live spectrum (a visual, not a real equaliser). */
@OptIn(UnstableApi::class)
@Composable
private fun AmpEq() {
    val levels = rememberLevels()
    val labels = listOf("60", "170", "310", "600", "1K", "3K", "6K", "12K", "14K", "16K")
    Row(Modifier.fillMaxWidth().height(78.dp), horizontalArrangement = Arrangement.SpaceBetween) {
        labels.forEachIndexed { i, label ->
            val v = levels.bands[(i * levels.bands.size / labels.size).coerceAtMost(levels.bands.size - 1)]
            Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.weight(1f)) {
                BoxWithConstraints(Modifier.weight(1f).width(14.dp)) {
                    Box(Modifier.align(Alignment.Center).width(4.dp).fillMaxHeight().background(AmpDark))
                    Box(
                        Modifier
                            .offset(y = (maxHeight - 10.dp) * (1f - v))
                            .size(14.dp, 10.dp)
                            .bevel(Color(0xFF6A9A3A), Color(0xFFB0E080), Color(0xFF2A4A10), width = 2f),
                    )
                }
                Text(label, color = Color(0xFFB0B0C8), fontFamily = AmpFont, fontSize = 8.sp)
            }
        }
    }
}

@Composable
private fun AmpPlaylist(state: PlayerUiState, modifier: Modifier) {
    val player = playerSource()
    val caps = playerCapabilities()
    val list = rememberLazyListState()
    LaunchedEffect(state.index) { if (state.index >= 0) list.animateScrollToItem((state.index - 2).coerceAtLeast(0)) }
    LazyColumn(modifier.background(Lcd).padding(4.dp), state = list) {
        items(state.queue.size) { i ->
            val e = state.queue[i]
            Text(
                "${i + 1}. ${listOf(e.artist, e.title).filter(String::isNotBlank).joinToString(" - ")}",
                color = if (i == state.index) Color.White else LcdGreen,
                fontFamily = AmpFont,
                fontSize = 13.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier
                    .fillMaxWidth()
                    .background(if (i == state.index) Color(0xFF0000A0) else Color.Transparent)
                    .clickable(enabled = caps.canSkipTo) { player.skipTo(i) }
                    .padding(vertical = 2.dp),
            )
        }
    }
}

// ===================================================================== Luna

private val LunaGlass = Brush.verticalGradient(listOf(Color(0xFFF4F8FF), Color(0xFFBFD2F2), Color(0xFF9DB8E6)))

@OptIn(UnstableApi::class)
@Composable
internal fun ColumnScope.LunaLayout(state: PlayerUiState, cover: Any?) {
    val player = playerSource()
    val caps = playerCapabilities()
    val levels = rememberLevels()
    val current = state.current
    Column(
        Modifier
            .weight(1f)
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .border(2.dp, Color(0xFF0B2A80), RoundedCornerShape(12.dp))
            .background(Color(0xFF0A1450)),
    ) {
        // "ambience": rays around the album
        Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
            Canvas(Modifier.fillMaxSize()) {
                drawRect(Brush.radialGradient(listOf(Color(0xFF1C3AA8), Color(0xFF000033)), center, size.maxDimension / 1.4f))
                val r0 = size.minDimension * 0.27f
                val n = 64
                for (i in 0 until n) {
                    val a = 2 * PI * i / n
                    val v = levels.bands[(i % (n / 2)) * levels.bands.size / (n / 2)]
                    val r1 = r0 + 8 + v * size.minDimension * 0.2f
                    drawLine(
                        Color(0xFF7FD4FF).copy(alpha = 0.35f + v * 0.6f),
                        Offset(center.x + r0 * cos(a).toFloat(), center.y + r0 * sin(a).toFloat()),
                        Offset(center.x + r1 * cos(a).toFloat(), center.y + r1 * sin(a).toFloat()),
                        strokeWidth = 4f,
                    )
                }
            }
            CoverArt(cover, size = null, radius = 999.dp, modifier = Modifier.fillMaxSize(0.48f).aspectRatio(1f, matchHeightConstraintsFirst = true).clip(CircleShape))
        }
        Row(
            Modifier.fillMaxWidth().background(Color(0xFF1D2D5C)).padding(horizontal = 10.dp, vertical = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                listOf(current?.artist.orEmpty(), current?.title.orEmpty()).filter(String::isNotBlank).joinToString(" : "),
                color = Color.White,
                fontSize = 13.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
            Text(formatDuration(state.positionMs), color = Color(0xFF9EC5FF), fontSize = 12.sp)
        }
    }
    Spacer(Modifier.height(10.dp))
    // seek: a groove with the green pill
    LunaSeek(state, caps.canSeek) { player.seekTo(it) }
    Spacer(Modifier.height(10.dp))
    Row(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(40.dp))
            .background(Brush.verticalGradient(listOf(Color(0xFFE8EEF8), Color(0xFFA9BDE0))))
            .border(1.dp, Color(0xFF5C7CC0), RoundedCornerShape(40.dp))
            .padding(horizontal = 12.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        LunaOrb(if (state.isPlaying) Icons.Rounded.Pause else Icons.Rounded.PlayArrow, if (state.isPlaying) R.string.player_pause else R.string.player_play, 64.dp, player::togglePlayPause, enabled = caps.canPlayPause)
        LunaOrb(Icons.Rounded.SkipPrevious, R.string.player_previous, 40.dp, player::previous, enabled = caps.canPrevious)
        LunaOrb(Icons.Rounded.SkipNext, R.string.player_next, 40.dp, player::next, enabled = caps.canNext)
        Spacer(Modifier.weight(1f))
        LunaOrb(Icons.Rounded.Shuffle, R.string.player_shuffle, 34.dp, player::toggleShuffle, on = state.shuffle, enabled = caps.canShuffleRepeat)
        LunaOrb(
            if (state.repeatMode == Player.REPEAT_MODE_ONE) Icons.Rounded.RepeatOne else Icons.Rounded.Repeat,
            R.string.player_repeat,
            34.dp,
            player::cycleRepeat,
            on = state.repeatMode != Player.REPEAT_MODE_OFF,
            enabled = caps.canShuffleRepeat,
        )
    }
    Spacer(Modifier.height(12.dp))
}

@Composable
private fun LunaOrb(icon: ImageVector, label: Int, size: Dp, onClick: () -> Unit, on: Boolean = false, enabled: Boolean = true) {
    Box(
        Modifier
            .size(size)
            .alpha(if (enabled) 1f else DISABLED_ALPHA)
            .clip(CircleShape)
            .background(
                if (on) {
                    Brush.radialGradient(listOf(Color(0xFFD6FFD0), Color(0xFF3CB043)))
                } else {
                    Brush.radialGradient(listOf(Color.White, Color(0xFF9DB8E6), Color(0xFF3D6CC8)))
                },
            )
            .border(1.dp, Color(0xFF2E4E9A), CircleShape)
            .clickable(enabled = enabled, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        // the glossy highlight on the upper half
        Box(Modifier.fillMaxSize().padding(size / 10).clip(CircleShape).background(Brush.verticalGradient(0f to Color.White.copy(alpha = 0.55f), 0.5f to Color.Transparent)))
        Icon(icon, stringResource(label), tint = Color(0xFF14326E), modifier = Modifier.size(size * 0.5f))
    }
}

@Composable
private fun LunaSeek(state: PlayerUiState, enabled: Boolean, onSeek: (Long) -> Unit) {
    val f = if (state.durationMs > 0) state.positionMs.toFloat() / state.durationMs else 0f
    BoxWithConstraints(Modifier.fillMaxWidth().height(16.dp)) {
        Box(Modifier.align(Alignment.CenterStart).fillMaxWidth().height(6.dp).clip(RoundedCornerShape(3.dp)).background(Color(0xFF0E2366)))
        Box(Modifier.align(Alignment.CenterStart).fillMaxWidth(f).height(6.dp).clip(RoundedCornerShape(3.dp)).background(Color(0xFF7FD4FF)))
        Box(
            Modifier
                .align(Alignment.CenterStart)
                .offset(x = (maxWidth - 26.dp) * f)
                .size(26.dp, 14.dp)
                .clip(RoundedCornerShape(7.dp))
                .background(Brush.verticalGradient(listOf(Color(0xFFB8F0A8), Color(0xFF2E9A36))))
                .border(1.dp, Color(0xFF1C5A20), RoundedCornerShape(7.dp)),
        )
        Row(Modifier.fillMaxSize()) {
            repeat(20) { i -> Box(Modifier.weight(1f).fillMaxHeight().clickable(enabled = enabled) { onSeek((state.durationMs * (i + 0.5f) / 20).toLong()) }) }
        }
    }
}

// ===================================================================== Aluminum

@Composable
internal fun ColumnScope.AluminumLayout(state: PlayerUiState, cover: Any?) {
    val player = playerSource()
    val caps = playerCapabilities()
    val current = state.current
    // transport on the left of the bar, like the jukebox software of 2008
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        MetalButton(Icons.Rounded.SkipPrevious, R.string.player_previous, 40.dp, player::previous, enabled = caps.canPrevious)
        MetalButton(if (state.isPlaying) Icons.Rounded.Pause else Icons.Rounded.PlayArrow, if (state.isPlaying) R.string.player_pause else R.string.player_play, 52.dp, player::togglePlayPause, enabled = caps.canPlayPause)
        MetalButton(Icons.Rounded.SkipNext, R.string.player_next, 40.dp, player::next, enabled = caps.canNext)
        Spacer(Modifier.weight(1f))
        MetalButton(Icons.Rounded.Shuffle, R.string.player_shuffle, 34.dp, player::toggleShuffle, on = state.shuffle, enabled = caps.canShuffleRepeat)
        MetalButton(
            if (state.repeatMode == Player.REPEAT_MODE_ONE) Icons.Rounded.RepeatOne else Icons.Rounded.Repeat,
            R.string.player_repeat,
            34.dp,
            player::cycleRepeat,
            on = state.repeatMode != Player.REPEAT_MODE_OFF,
            enabled = caps.canShuffleRepeat,
        )
    }
    Spacer(Modifier.height(10.dp))
    // the pale LCD
    Column(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(8.dp))
            .background(Brush.verticalGradient(listOf(Color(0xFFF2F4E6), Color(0xFFD8DCC4))))
            .border(1.dp, Color(0xFF8C8C8C), RoundedCornerShape(8.dp))
            .padding(horizontal = 12.dp, vertical = 8.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(current?.title.orEmpty(), color = Color(0xFF1E1E1E), fontWeight = FontWeight.SemiBold, fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
        Text(
            listOf(current?.artist.orEmpty(), current?.album.orEmpty()).filter(String::isNotBlank).joinToString(" — "),
            color = Color(0xFF4A4A4A),
            fontSize = 12.sp,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Spacer(Modifier.height(6.dp))
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(formatDuration(state.positionMs), color = Color(0xFF3A3A3A), fontSize = 10.sp)
            Spacer(Modifier.width(6.dp))
            val f = if (state.durationMs > 0) state.positionMs.toFloat() / state.durationMs else 0f
            BoxWithConstraints(Modifier.weight(1f).height(10.dp)) {
                Box(Modifier.align(Alignment.Center).fillMaxWidth().height(7.dp).border(1.dp, Color(0xFF555555)))
                Box(Modifier.align(Alignment.CenterStart).fillMaxWidth(f).height(7.dp).background(Color(0xFF555555)))
                Box(Modifier.align(Alignment.CenterStart).offset(x = (maxWidth - 8.dp) * f).size(8.dp).graphicsLayer { rotationZ = 45f }.background(Color(0xFF222222)))
                Row(Modifier.fillMaxSize()) {
                    repeat(20) { i -> Box(Modifier.weight(1f).fillMaxHeight().clickable(enabled = caps.canSeek) { player.seekTo((state.durationMs * (i + 0.5f) / 20).toLong()) }) }
                }
            }
            Spacer(Modifier.width(6.dp))
            Text("-" + formatDuration((state.durationMs - state.positionMs).coerceAtLeast(0)), color = Color(0xFF3A3A3A), fontSize = 10.sp)
        }
    }
    Spacer(Modifier.height(10.dp))
    CoverFlow(state, cover, Modifier.weight(1f).fillMaxWidth())
    Spacer(Modifier.height(8.dp))
}

@Composable
private fun MetalButton(icon: ImageVector, label: Int, size: Dp, onClick: () -> Unit, on: Boolean = false, enabled: Boolean = true) {
    Box(
        Modifier
            .size(size)
            .alpha(if (enabled) 1f else DISABLED_ALPHA)
            .clip(CircleShape)
            .background(Brush.verticalGradient(listOf(Color(0xFFFDFDFD), Color(0xFFC9C9C9), Color(0xFFA0A0A0))))
            .border(1.dp, Color(0xFF6E6E6E), CircleShape)
            .clickable(enabled = enabled, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, stringResource(label), tint = if (on) Color(0xFF2E6FD8) else Color(0xFF2E2E2E), modifier = Modifier.size(size * 0.55f))
    }
}

/** One album in the cover flow: from the library, or the song playing when it is not in it. */
private data class FlowAlbum(val key: String, val title: String, val artist: String, val cover: Any?, val tracks: List<LibraryTrack>)

/**
 * The iPod's Cover Flow: every album of the library in a row, the one in front face-on with
 * its reflection, the rest turned away. Swiping only browses (the music never changes);
 * tapping a side album brings it forward; tapping the one in front turns it over to its
 * songs, to play one or queue it. Sized from the space it gets, centred, on any screen.
 */
@Composable
private fun CoverFlow(state: PlayerUiState, cover: Any?, modifier: Modifier) {
    val app = LocalContext.current.homefi
    val library by app.library.state.collectAsStateWithLifecycle()
    val current = state.current
    // the album playing: by file, else by name (a file opened from elsewhere, a copy on the SD card...)
    val playingTrack = current?.let { c ->
        library.tracks.firstOrNull { it.path == c.mediaId }
            ?: library.tracks.firstOrNull { c.album.isNotBlank() && it.album.equals(c.album, true) && it.artist.equals(c.artist, true) }
    }
    val playingKey = playingTrack?.let(::albumKey) ?: current?.let { "now\u0000${it.artist}\u0000${it.album}" }

    val albums = remember(library.tracks, playingKey) {
        val list = library.tracks.albums()
            .sortedWith(compareBy({ it.artist.lowercase() }, { it.year ?: 0u }, { it.title.lowercase() }))
            .map { FlowAlbum(it.key, it.title, it.artist, it.cover, it.tracks) }
            .toMutableList()
        // a Tidal stream (or a file outside the library) gets its place in front of the others
        if (current != null && list.none { it.key == playingKey }) {
            list.add(0, FlowAlbum(playingKey!!, current.album.ifBlank { current.title }, current.artist, null, emptyList()))
        }
        list
    }
    if (albums.isEmpty()) return
    val playingIndex = albums.indexOfFirst { it.key == playingKey }.coerceAtLeast(0)

    val position = remember { Animatable(playingIndex.toFloat()) }
    val flip = remember { Animatable(0f) } // 0 cover, 180 its songs
    var opened by rememberSaveable { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()
    var dragging by remember { mutableStateOf(false) }
    var touchedAt by remember { mutableLongStateOf(0L) }
    val front = position.value.roundToInt().coerceIn(0, albums.lastIndex)

    // a new album started: glide to it, unless you are browsing
    LaunchedEffect(playingIndex, albums.size) {
        if (!dragging && opened == null && SystemClock.uptimeMillis() - touchedAt > FOLLOW_AFTER_MS) {
            position.animateTo(playingIndex.toFloat(), spring(stiffness = Spring.StiffnessLow))
        }
    }
    LaunchedEffect(opened) { flip.animateTo(if (opened != null) 180f else 0f, tween(450)) }
    BackHandler(enabled = opened != null) { opened = null }

    BoxWithConstraints(
        modifier
            .clip(RoundedCornerShape(8.dp))
            .background(Brush.verticalGradient(listOf(Color(0xFF2A2A2A), Color.Black))),
    ) {
        val captionHeight = 52.dp
        // the cover in front with its reflection, and the caption, fit both ways
        val side = minOf(maxWidth * 0.52f, (maxHeight - captionHeight) / (1f + REFLECTION + 0.12f))
        val spacing = side * 0.66f // centre to the first neighbour
        val step = side * 0.16f // between the ones further away, stacked like iTunes
        val pxPerItem = with(LocalDensity.current) { spacing.toPx() }
        val groupHeight = side * (1f + REFLECTION) + captionHeight
        val top = (maxHeight - groupHeight) / 2
        val visible = ((maxWidth / 2 - spacing) / step).toInt() + 3
        val captionWidth = maxWidth - 32.dp
        // the back of the album: wider than the cover, as tall as cover and reflection
        val backWidth = minOf(maxWidth - 24.dp, side * 1.5f)
        val backHeight = minOf(maxHeight - 24.dp, side * (1f + REFLECTION) + captionHeight)

        Box(
            Modifier
                .fillMaxSize()
                .pointerInput(albums.size) {
                    val velocity = VelocityTracker()
                    detectHorizontalDragGestures(
                        onDragStart = {
                            dragging = true
                            opened = null
                            velocity.resetTracking()
                        },
                        onHorizontalDrag = { change, amount ->
                            change.consume()
                            velocity.addPosition(change.uptimeMillis, change.position)
                            scope.launch {
                                position.snapTo((position.value - amount / pxPerItem).coerceIn(-0.4f, albums.lastIndex + 0.4f))
                            }
                        },
                        onDragEnd = {
                            val v = -velocity.calculateVelocity().x / pxPerItem
                            touchedAt = SystemClock.uptimeMillis()
                            scope.launch {
                                // fling, then settle on the nearest album
                                val target = (position.value + v * 0.35f).roundToInt().coerceIn(0, albums.lastIndex)
                                position.animateTo(target.toFloat(), spring(dampingRatio = 0.85f, stiffness = Spring.StiffnessLow), initialVelocity = v)
                                dragging = false
                            }
                        },
                        onDragCancel = { dragging = false },
                    )
                },
        ) {
            val p = position.value
            val turned = flip.value
            // far ones first, so the front one is drawn on top
            albums.indices
                .filter { abs(it - p) < visible && !(it == front && turned > 90f) }
                .sortedByDescending { abs(it - p) }
                .forEach { i ->
                    val album = albums[i]
                    val d = i - p
                    val near = d.coerceIn(-1f, 1f)
                    val x = if (abs(d) <= 1f) spacing * d else (spacing + step * (abs(d) - 1f)) * sign(d)
                    val scale = 1f - 0.18f * abs(near)
                    val art = if (album.key == playingKey && album.tracks.isEmpty()) cover else album.cover
                    val dim = if (i == front) 1f else 1f - 0.6f * (turned / 180f)
                    Flowed(
                        art,
                        side,
                        Modifier
                            .align(Alignment.TopCenter)
                            .offset(x = x, y = top)
                            .graphicsLayer {
                                rotationY = -62f * near + if (i == front) turned else 0f
                                scaleX = scale
                                scaleY = scale
                                cameraDistance = 10f * density
                                alpha = (visible - abs(d)).coerceIn(0f, 1f) * dim
                            },
                    ) {
                        touchedAt = SystemClock.uptimeMillis()
                        if (i == front) {
                            if (album.tracks.isNotEmpty()) opened = album.key
                        } else {
                            opened = null
                            scope.launch { position.animateTo(i.toFloat(), spring(stiffness = Spring.StiffnessLow)) }
                        }
                    }
                }

            val shown = albums[front]
            if (turned > 90f && opened == shown.key) {
                AlbumBack(
                    shown,
                    playingMediaId = current?.mediaId,
                    onClose = { opened = null },
                    modifier = Modifier
                        .align(Alignment.Center)
                        .size(backWidth, backHeight)
                        .graphicsLayer {
                            rotationY = turned - 180f
                            cameraDistance = 10f * density
                        },
                )
            } else {
                // caption of the album in front
                Column(
                    Modifier
                        .align(Alignment.TopCenter)
                        .offset(y = top + side * (1f + REFLECTION))
                        .width(captionWidth)
                        .height(captionHeight),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.Center,
                ) {
                    Text(shown.title, color = Color.White, fontWeight = FontWeight.Bold, fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    val hint = if (shown.tracks.isEmpty()) shown.artist else stringResource(R.string.coverflow_tap_for_songs, shown.artist)
                    Text(hint, color = Color(0xFFB0B0B0), fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
        }
    }
}

/** The back of an album in the cover flow: its songs; tap one to play the album from it. */
@Composable
private fun AlbumBack(album: FlowAlbum, playingMediaId: String?, onClose: () -> Unit, modifier: Modifier) {
    val player = playerSource()
    val caps = playerCapabilities()
    Column(
        modifier
            .clip(RoundedCornerShape(6.dp))
            .background(Brush.verticalGradient(listOf(Color(0xFFF7F7F7), Color(0xFFDCDCDC))))
            .border(1.dp, Color(0xFF8C8C8C), RoundedCornerShape(6.dp)),
    ) {
        // header: tap it to turn the album back over
        Row(
            Modifier
                .fillMaxWidth()
                .background(Brush.verticalGradient(listOf(Color(0xFF5A5A5A), Color(0xFF2E2E2E))))
                .clickable(onClickLabel = stringResource(R.string.coverflow_close), onClick = onClose)
                .padding(start = 12.dp, end = 4.dp, top = 6.dp, bottom = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f)) {
                Text(album.title, color = Color.White, fontWeight = FontWeight.Bold, fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(album.artist, color = Color(0xFFCFCFCF), fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            // a guest browses its own albums; the hub's queue takes songs from the hub's search
            val canPlay = caps.canPlayLibrary
            val headerTint = Color.White.copy(alpha = if (canPlay) 1f else DISABLED_ALPHA)
            var menu by remember { mutableStateOf(false) }
            Box {
                IconButton(onClick = { menu = true }, enabled = canPlay) {
                    Icon(Icons.AutoMirrored.Rounded.QueueMusic, stringResource(R.string.queue_add), tint = headerTint)
                }
                QueueMenu(menu, { menu = false }) { next -> player.queueTracks(album.tracks, next) }
            }
            IconButton(onClick = { player.play(album.tracks) }, enabled = canPlay) {
                Icon(Icons.Rounded.PlayArrow, stringResource(R.string.player_play), tint = headerTint)
            }
        }
        LazyColumn(Modifier.weight(1f)) {
            items(album.tracks.size, key = { album.tracks[it].path }) { i ->
                val track = album.tracks[i]
                val playing = track.path == playingMediaId
                var menu by remember { mutableStateOf(false) }
                Row(
                    Modifier
                        .fillMaxWidth()
                        .background(if (playing) Color(0xFFBFD4F2) else if (i % 2 == 1) Color(0x0F000000) else Color.Transparent)
                        .clickable(enabled = caps.canPlayLibrary) { player.play(album.tracks, i) }
                        .padding(start = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        if (track.trackNumber > 0u) track.trackNumber.toString() else "${i + 1}",
                        color = Color(0xFF777777),
                        fontSize = 12.sp,
                        modifier = Modifier.width(26.dp),
                    )
                    Text(
                        track.title,
                        color = Color(0xFF1E1E1E),
                        fontSize = 14.sp,
                        fontWeight = if (playing) FontWeight.Bold else null,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f),
                    )
                    if (track.durationMs > 0u) Text(formatDuration(track.durationMs.toLong()), color = Color(0xFF666666), fontSize = 12.sp)
                    Box {
                        IconButton(onClick = { menu = true }, enabled = caps.canPlayLibrary) {
                            Icon(Icons.Rounded.MoreVert, stringResource(R.string.queue_add), tint = Color(0xFF555555).copy(alpha = if (caps.canPlayLibrary) 1f else DISABLED_ALPHA))
                        }
                        QueueMenu(menu, { menu = false }) { next -> player.queueTracks(listOf(track), next) }
                    }
                }
            }
        }
    }
}

private const val REFLECTION = 0.35f

/** Browsing pauses following the album playing for this long. */
private const val FOLLOW_AFTER_MS = 8_000L

@Composable
private fun Flowed(cover: Any?, side: Dp, modifier: Modifier, onClick: () -> Unit) {
    Column(modifier.clickable(onClick = onClick)) {
        CoverArt(cover, side, radius = 0.dp)
        // reflection on the glossy floor
        Box(Modifier.size(side, side * REFLECTION).graphicsLayer { clip = true }) {
            CoverArt(cover, side, radius = 0.dp, modifier = Modifier.graphicsLayer { scaleY = -1f; alpha = 0.35f })
            Box(Modifier.fillMaxSize().background(Brush.verticalGradient(listOf(Color.Black.copy(alpha = 0.3f), Color.Black))))
        }
    }
}

// ===================================================================== Web 2000

private val WebGreen = Color(0xFF8CC63F)
private val WebPanel = Color(0xFFF2F2F2)

@Composable
internal fun ColumnScope.Web2000Layout(state: PlayerUiState, cover: Any?) {
    val player = playerSource()
    val caps = playerCapabilities()
    var tab by remember { mutableStateOf(0) }
    val current = state.current
    // tabs on the charcoal header
    Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        listOf(R.string.web_playing, R.string.web_up_next).forEachIndexed { i, label ->
            val selected = tab == i
            Box(
                Modifier
                    .clip(RoundedCornerShape(topStart = 10.dp, topEnd = 10.dp))
                    .background(if (selected) Brush.verticalGradient(listOf(Color(0xFFF4F4F4), Color(0xFFD9D9D9))) else Brush.verticalGradient(listOf(Color(0xFF6A6A6E), Color(0xFF525256))))
                    .clickable { tab = i }
                    .padding(horizontal = 18.dp, vertical = 8.dp),
            ) {
                Text(stringResource(label), color = if (selected) Color(0xFF3A3A3A) else Color(0xFFDADADA), fontWeight = FontWeight.Bold, fontSize = 14.sp)
            }
        }
    }
    Column(
        Modifier
            .weight(1f)
            .fillMaxWidth()
            .background(Color(0xFFD9D9D9))
            .padding(8.dp),
    ) {
        if (tab == 0) {
            WebBox(stringResource(R.string.web_playing), Modifier.weight(1f)) {
                Column(Modifier.fillMaxSize().padding(10.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    CoverArt(cover, size = null, radius = 4.dp, modifier = Modifier.weight(1f).aspectSquare())
                    Spacer(Modifier.height(8.dp))
                    Text(current?.title.orEmpty(), color = Color(0xFF222222), fontWeight = FontWeight.Bold, fontSize = 17.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(current?.artist.orEmpty(), color = Color(0xFF555555), fontSize = 13.sp, maxLines = 1)
                    current?.quality?.let { Text(it, color = Color(0xFF5E8F1E), fontSize = 12.sp, fontWeight = FontWeight.Bold) }
                }
            }
        } else {
            WebBox(stringResource(R.string.web_up_next), Modifier.weight(1f)) {
                val list = rememberLazyListState()
                LazyColumn(state = list, contentPadding = PaddingValues(vertical = 4.dp)) {
                    items(state.queue.size) { i ->
                        val e = state.queue[i]
                        Row(
                            Modifier
                                .fillMaxWidth()
                                .background(if (i == state.index) Color(0xFFE4F1D2) else Color.Transparent)
                                .clickable(enabled = caps.canSkipTo) { player.skipTo(i) }
                                .padding(horizontal = 10.dp, vertical = 6.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text("${i + 1}", color = Color(0xFF777777), fontSize = 13.sp, fontWeight = FontWeight.Bold, modifier = Modifier.width(26.dp))
                            Column(Modifier.weight(1f)) {
                                Text(e.title, color = Color(0xFF222222), fontWeight = FontWeight.Bold, fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                Text(e.artist, color = Color(0xFF666666), fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            }
                            Box(Modifier.size(26.dp).clip(CircleShape).border(1.5.dp, Color(0xFF8A8A8A), CircleShape), contentAlignment = Alignment.Center) {
                                Icon(Icons.Rounded.PlayArrow, null, tint = Color(0xFF6A6A6A), modifier = Modifier.size(16.dp))
                            }
                        }
                    }
                }
            }
        }
        Spacer(Modifier.height(8.dp))
        // progress and controls
        val f = if (state.durationMs > 0) state.positionMs.toFloat() / state.durationMs else 0f
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(formatDuration(state.positionMs), color = Color(0xFF444444), fontSize = 11.sp)
            Spacer(Modifier.width(6.dp))
            Box(Modifier.weight(1f).height(8.dp).clip(RoundedCornerShape(4.dp)).background(Color(0xFFB5B5B5))) {
                Box(Modifier.fillMaxWidth(f).fillMaxHeight().background(WebGreen))
                Row(Modifier.fillMaxSize()) {
                    repeat(20) { i -> Box(Modifier.weight(1f).fillMaxHeight().clickable(enabled = caps.canSeek) { player.seekTo((state.durationMs * (i + 0.5f) / 20).toLong()) }) }
                }
            }
            Spacer(Modifier.width(6.dp))
            Text(formatDuration(state.durationMs), color = Color(0xFF444444), fontSize = 11.sp)
        }
        Spacer(Modifier.height(8.dp))
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally), verticalAlignment = Alignment.CenterVertically) {
            WebButton(Icons.Rounded.Shuffle, R.string.player_shuffle, 36.dp, player::toggleShuffle, on = state.shuffle, enabled = caps.canShuffleRepeat)
            WebButton(Icons.Rounded.SkipPrevious, R.string.player_previous, 44.dp, player::previous, enabled = caps.canPrevious)
            WebButton(if (state.isPlaying) Icons.Rounded.Pause else Icons.Rounded.PlayArrow, if (state.isPlaying) R.string.player_pause else R.string.player_play, 60.dp, player::togglePlayPause, on = true, enabled = caps.canPlayPause)
            WebButton(Icons.Rounded.SkipNext, R.string.player_next, 44.dp, player::next, enabled = caps.canNext)
            WebButton(
                if (state.repeatMode == Player.REPEAT_MODE_ONE) Icons.Rounded.RepeatOne else Icons.Rounded.Repeat,
                R.string.player_repeat,
                36.dp,
                player::cycleRepeat,
                on = state.repeatMode != Player.REPEAT_MODE_OFF,
                enabled = caps.canShuffleRepeat,
            )
        }
    }
    Spacer(Modifier.height(8.dp))
}

private fun Modifier.aspectSquare() = aspectRatio(1f, matchHeightConstraintsFirst = true)

@Composable
private fun WebBox(title: String, modifier: Modifier, content: @Composable () -> Unit) {
    Column(modifier.fillMaxWidth().clip(RoundedCornerShape(6.dp)).background(WebPanel)) {
        Box(
            Modifier
                .fillMaxWidth()
                .background(Brush.verticalGradient(listOf(Color(0xFFA8A8A8), Color(0xFF8E8E8E))))
                .padding(horizontal = 10.dp, vertical = 5.dp),
        ) {
            Text(title, color = Color.White, fontWeight = FontWeight.Bold, fontSize = 13.sp)
        }
        Box(Modifier.weight(1f)) { content() }
    }
}

@Composable
private fun WebButton(icon: ImageVector, label: Int, size: Dp, onClick: () -> Unit, on: Boolean = false, enabled: Boolean = true) {
    Box(
        Modifier
            .size(size)
            .alpha(if (enabled) 1f else DISABLED_ALPHA)
            .clip(CircleShape)
            .background(Brush.verticalGradient(listOf(Color(0xFFFAFAFA), Color(0xFFCFCFCF))))
            .border(1.5.dp, if (on) WebGreen else Color(0xFF8A8A8A), CircleShape)
            .clickable(enabled = enabled, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, stringResource(label), tint = if (on) Color(0xFF5E8F1E) else Color(0xFF555555), modifier = Modifier.size(size * 0.55f))
    }
}
