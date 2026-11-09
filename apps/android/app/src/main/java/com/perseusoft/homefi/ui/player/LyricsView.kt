package com.perseusoft.homefi.ui.player

import androidx.compose.animation.AnimatedContent
import com.perseusoft.homefi.ui.theme.LocalMotion
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.basicMarquee
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.DragInteraction
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Lyrics
import androidx.compose.material.icons.rounded.Mic
import androidx.compose.material.icons.rounded.MyLocation
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.SkipNext
import androidx.compose.material.icons.rounded.SkipPrevious
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.Immutable
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
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.data.LYRICS_OFFSET_RANGE
import com.perseusoft.homefi.data.LYRICS_OFFSET_STEP
import com.perseusoft.homefi.data.LyricsMode
import com.perseusoft.homefi.data.LyricsSize
import com.perseusoft.homefi.data.PlayerLayout
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.lyrics.Lyrics
import com.perseusoft.homefi.lyrics.LyricsState
import com.perseusoft.homefi.player.PlayerUiState
import com.perseusoft.homefi.ui.theme.Button
import com.perseusoft.homefi.ui.theme.LyricsPalette
import com.perseusoft.homefi.ui.theme.OutlinedButton
import com.perseusoft.homefi.ui.theme.lyricsPalette
import kotlinx.coroutines.delay
import java.util.Locale
import kotlin.math.abs
import kotlin.math.ceil
import kotlin.math.min
import kotlin.math.sin

/** After scrolling by hand the list stays put this long before following the song again. */
private const val FOLLOW_PAUSE_MS = 4_000L



fun LyricsSize.label(): Int = when (this) {
    LyricsSize.SMALL -> R.string.lyrics_size_small
    LyricsSize.MEDIUM -> R.string.lyrics_size_medium
    LyricsSize.LARGE -> R.string.lyrics_size_large
}

fun LyricsMode.label(): Int = when (this) {
    LyricsMode.NORMAL -> R.string.lyrics_mode_normal
    LyricsMode.KARAOKE -> R.string.lyrics_mode_karaoke
}

/** `+0.3`, `-1.2`, `0.0`: the lyrics timing in seconds, as the menu and settings show it. */
fun lyricsOffsetText(ms: Int): String =
    if (ms == 0) String.format(Locale.getDefault(), "%.1f", 0f) else String.format(Locale.getDefault(), "%+.1f", ms / 1000f)

private fun LyricsSize.normal(): TextUnit = when (this) {
    LyricsSize.SMALL -> 18.sp
    LyricsSize.MEDIUM -> 22.sp
    LyricsSize.LARGE -> 27.sp
}

private fun LyricsSize.karaoke(): TextUnit = when (this) {
    LyricsSize.SMALL -> 26.sp
    LyricsSize.MEDIUM -> 32.sp
    LyricsSize.LARGE -> 40.sp
}

private fun LyricsSize.companion(): TextUnit = when (this) {
    LyricsSize.SMALL -> 15.sp
    LyricsSize.MEDIUM -> 18.sp
    LyricsSize.LARGE -> 21.sp
}

/** `MUSIXMATCH` → `Musixmatch`. */
private fun providerName(provider: String): String =
    if (provider.equals("MUSIXMATCH", ignoreCase = true)) "Musixmatch"
    else provider.lowercase(Locale.ROOT).replaceFirstChar { it.titlecase(Locale.ROOT) }

/**
 * The lyrics in place of the player's layout: a Normal / Karaoke toggle and the options on
 * top, the lines in the middle, the credit, the seek bar and play / skip below.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
internal fun ColumnScope.LyricsLayout(state: PlayerUiState, layout: PlayerLayout, onClose: () -> Unit) {
    val app = LocalContext.current.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val source = playerSource()
    val caps = playerCapabilities()
    val palette = lyricsPalette(layout)
    val mediaId = state.current?.mediaId
    LaunchedEffect(mediaId) { mediaId?.let(app.lyrics::request) }
    val states by app.lyrics.states.collectAsStateWithLifecycle()
    val lyricsState = if (mediaId == null) null else states[mediaId] ?: LyricsState.Loading

    LyricsToolbar(settings.lyricsMode, palette, onMode = { m -> app.settings.update { it.copy(lyricsMode = m) } }, onClose = onClose)
    state.current?.let { now ->
        Text(
            listOf(now.title, now.artist).filter(String::isNotBlank).joinToString(" · "),
            style = MaterialTheme.typography.labelLarge,
            color = palette.label,
            maxLines = 1,
            textAlign = TextAlign.Center,
            modifier = Modifier.fillMaxWidth().basicMarquee(),
        )
    }
    val panel = palette.panel
    Box(
        Modifier
            .weight(1f)
            .fillMaxWidth()
            .padding(vertical = 4.dp)
            .then(if (palette.lcd) Modifier.lcdScreen() else Modifier)
            .then(if (panel != null) Modifier.clip(RoundedCornerShape(if (palette.lcd) 0.dp else 16.dp)).background(panel) else Modifier),
        contentAlignment = Alignment.Center,
    ) {
        when (lyricsState) {
            null -> LyricsMessage(stringResource(R.string.lyrics_nothing), palette)
            LyricsState.Loading -> Column(horizontalAlignment = Alignment.CenterHorizontally) {
                CircularProgressIndicator(color = palette.fill, strokeWidth = 3.dp)
                Spacer(Modifier.height(12.dp))
                Text(stringResource(R.string.lyrics_loading), color = palette.dim, style = MaterialTheme.typography.bodyMedium)
            }
            LyricsState.None -> LyricsMessage(stringResource(R.string.lyrics_none), palette)
            is LyricsState.Failed -> LyricsMessage(lyricsState.message, palette) {
                OutlinedButton(onClick = { mediaId?.let(app.lyrics::retry) }) { Text(stringResource(R.string.retry)) }
            }
            is LyricsState.Ready -> {
                val lyrics = lyricsState.lyrics
                val time = rememberLyricsTime(state, settings.lyricsOffsetMs)
                val seek: ((Long) -> Unit)? = if (lyrics.synced && caps.canSeek) {
                    { t -> source.seekTo((t + settings.lyricsOffsetMs).coerceAtLeast(0)) }
                } else {
                    null
                }
                val karaoke = settings.lyricsMode == LyricsMode.KARAOKE
                if (karaoke && lyrics.synced) {
                    KaraokeLyrics(lyrics, time, palette, settings.lyricsSize, mascot = settings.lyricsMascot, playing = state.isPlaying)
                } else {
                    val note = when {
                        karaoke -> R.string.lyrics_karaoke_needs_synced
                        !lyrics.synced -> R.string.lyrics_not_synced
                        else -> null
                    }
                    LyricsLines(lyrics, time, palette, settings.lyricsSize, note, seek)
                }
            }
        }
    }
    (lyricsState as? LyricsState.Ready)?.lyrics?.provider?.let { p ->
        Text(
            stringResource(R.string.lyrics_by, providerName(p)),
            style = MaterialTheme.typography.labelSmall,
            color = palette.label,
            textAlign = TextAlign.Center,
            modifier = Modifier.fillMaxWidth(),
        )
    }
    // the seek bar in the lyrics' colours too (the retro players' own bars aren't here)
    MaterialTheme(
        colorScheme = MaterialTheme.colorScheme.copy(
            primary = palette.fill,
            onSurface = palette.ink,
            onSurfaceVariant = palette.label,
        ),
    ) { StyledSeekBar(state) }
    LyricsTransport(state, palette)
    Spacer(Modifier.height(if (LocalPlayerCompact.current) 4.dp else 12.dp))
}

/** The amp's sunken LCD around the lines. */
private fun Modifier.lcdScreen(): Modifier = drawBehind {
    val w = 2.dp.toPx()
    val dark = Color.Black.copy(alpha = 0.7f)
    val light = Color.White.copy(alpha = 0.25f)
    drawRect(dark, Offset.Zero, Size(size.width, w))
    drawRect(dark, Offset.Zero, Size(w, size.height))
    drawRect(light, Offset(0f, size.height - w), Size(size.width, w))
    drawRect(light, Offset(size.width - w, 0f), Size(w, size.height))
}.padding(2.dp)

@Composable
private fun LyricsMessage(text: String, palette: LyricsPalette, action: (@Composable () -> Unit)? = null) {
    Column(Modifier.padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        Icon(Icons.Rounded.Lyrics, null, tint = palette.dim, modifier = Modifier.size(40.dp))
        Spacer(Modifier.height(12.dp))
        Text(text, color = palette.ink, style = MaterialTheme.typography.titleMedium, textAlign = TextAlign.Center)
        if (action != null) {
            Spacer(Modifier.height(16.dp))
            action()
        }
    }
}

/** Normal / Karaoke, the options menu and close. */
@Composable
private fun LyricsToolbar(mode: LyricsMode, palette: LyricsPalette, onMode: (LyricsMode) -> Unit, onClose: () -> Unit) {
    Row(Modifier.fillMaxWidth().height(48.dp), verticalAlignment = Alignment.CenterVertically) {
        Row(Modifier.selectableGroup(), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            LyricsMode.entries.forEach { m ->
                val selected = m == mode
                Box(
                    Modifier
                        .fillMaxHeight()
                        .selectable(selected = selected, role = Role.Tab, onClick = { onMode(m) })
                        .padding(vertical = 6.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    Row(
                        Modifier
                            .fillMaxHeight()
                            .clip(RoundedCornerShape(50))
                            .background(if (selected) palette.fill else palette.ink.copy(alpha = 0.08f))
                            .then(if (selected) Modifier else Modifier.border(1.dp, palette.dim.copy(alpha = 0.4f), RoundedCornerShape(50)))
                            .padding(horizontal = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        val ink = if (selected) palette.onFill else palette.ink
                        Icon(if (m == LyricsMode.NORMAL) Icons.Rounded.Lyrics else Icons.Rounded.Mic, null, tint = ink, modifier = Modifier.size(16.dp))
                        Spacer(Modifier.width(6.dp))
                        Text(stringResource(m.label()), color = ink, style = MaterialTheme.typography.labelLarge, maxLines = 1)
                    }
                }
            }
        }
        Spacer(Modifier.weight(1f))
        LyricsOptions(palette)
        IconButton(onClick = onClose) { Icon(Icons.Rounded.Close, stringResource(R.string.lyrics_close), tint = palette.ink) }
    }
}

/** Size, timing, HomeFito and auto-open, without leaving the lyrics (all in Settings too). */
@Composable
private fun LyricsOptions(palette: LyricsPalette) {
    val app = LocalContext.current.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    var open by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { open = true }) { Icon(Icons.Rounded.Tune, stringResource(R.string.lyrics_options), tint = palette.ink) }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            MenuLabel(R.string.lyrics_size)
            LyricsSize.entries.forEach { s ->
                DropdownMenuItem(
                    text = { Text(stringResource(s.label())) },
                    trailingIcon = { RadioButton(selected = s == settings.lyricsSize, onClick = null) },
                    onClick = { app.settings.update { it.copy(lyricsSize = s) } },
                )
            }
            HorizontalDivider()
            MenuLabel(R.string.lyrics_timing, lyricsOffsetText(settings.lyricsOffsetMs))
            fun shift(by: Int) = app.settings.update {
                it.copy(lyricsOffsetMs = (it.lyricsOffsetMs + by).coerceIn(LYRICS_OFFSET_RANGE))
            }
            DropdownMenuItem(
                text = { Text(stringResource(R.string.lyrics_timing_earlier)) },
                enabled = settings.lyricsOffsetMs > LYRICS_OFFSET_RANGE.first,
                onClick = { shift(-LYRICS_OFFSET_STEP) },
            )
            DropdownMenuItem(
                text = { Text(stringResource(R.string.lyrics_timing_later)) },
                enabled = settings.lyricsOffsetMs < LYRICS_OFFSET_RANGE.last,
                onClick = { shift(LYRICS_OFFSET_STEP) },
            )
            if (settings.lyricsOffsetMs != 0) {
                DropdownMenuItem(
                    text = { Text(stringResource(R.string.lyrics_timing_reset)) },
                    onClick = { app.settings.update { it.copy(lyricsOffsetMs = 0) } },
                )
            }
            HorizontalDivider()
            DropdownMenuItem(
                text = { Text(stringResource(R.string.lyrics_mascot)) },
                trailingIcon = { Switch(checked = settings.lyricsMascot, onCheckedChange = null) },
                onClick = { app.settings.update { it.copy(lyricsMascot = !it.lyricsMascot) } },
            )
            DropdownMenuItem(
                text = { Text(stringResource(R.string.lyrics_auto_open)) },
                trailingIcon = { Switch(checked = settings.lyricsAutoOpen, onCheckedChange = null) },
                onClick = { app.settings.update { it.copy(lyricsAutoOpen = !it.lyricsAutoOpen) } },
            )
        }
    }
}

@Composable
private fun MenuLabel(text: Int, vararg args: Any) {
    Text(
        stringResource(text, *args),
        style = MaterialTheme.typography.labelMedium,
        color = MaterialTheme.colorScheme.primary,
        modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
    )
}

/** Previous · play / pause · next, in the lyrics' colours so they read on every skin. */
@Composable
private fun LyricsTransport(state: PlayerUiState, palette: LyricsPalette) {
    val player = playerSource()
    val caps = playerCapabilities()
    val compact = LocalPlayerCompact.current
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(16.dp, Alignment.CenterHorizontally), verticalAlignment = Alignment.CenterVertically) {
        IconButton(onClick = player::previous, enabled = caps.canPrevious) {
            Icon(Icons.Rounded.SkipPrevious, stringResource(R.string.player_previous), tint = palette.ink.copy(alpha = if (caps.canPrevious) 1f else DISABLED_ALPHA), modifier = Modifier.size(30.dp))
        }
        val play: Dp = if (compact) 52.dp else 60.dp
        Box(
            Modifier
                .size(play)
                .clip(CircleShape)
                .background(palette.fill.copy(alpha = if (caps.canPlayPause) 1f else DISABLED_ALPHA))
                .clickable(enabled = caps.canPlayPause, role = Role.Button, onClick = player::togglePlayPause),
            contentAlignment = Alignment.Center,
        ) {
            if (state.isBuffering) CircularProgressIndicator(Modifier.size(play), color = palette.onFill, strokeWidth = 2.dp)
            Icon(
                if (state.isPlaying) Icons.Rounded.Pause else Icons.Rounded.PlayArrow,
                stringResource(if (state.isPlaying) R.string.player_pause else R.string.player_play),
                tint = palette.onFill,
                modifier = Modifier.size(if (compact) 30.dp else 34.dp),
            )
        }
        IconButton(onClick = player::next, enabled = caps.canNext) {
            Icon(Icons.Rounded.SkipNext, stringResource(R.string.player_next), tint = palette.ink.copy(alpha = if (caps.canNext) 1f else DISABLED_ALPHA), modifier = Modifier.size(30.dp))
        }
    }
}


// ------------------------------------------------------------------ normal

/**
 * The whole text, the line being sung bright and the rest dimmed, kept about a third from
 * the top. Scrolling by hand stops the following for a few seconds (with a chip to come
 * back at once); a tap on a line jumps there ([onLine] gets that line's time; null = lines are not tappable).
 * Also what the TV screens draw.
 */
@Composable
internal fun LyricsLines(lyrics: Lyrics, time: State<Long>, palette: LyricsPalette, size: LyricsSize, note: Int?, onLine: ((Long) -> Unit)?) {
    val motion = LocalMotion.current
    val synced = lyrics.synced
    val current by remember(lyrics) { derivedStateOf { lyrics.lineAt(time.value) } }
    val list = rememberLazyListState()
    var follow by remember { mutableStateOf(true) }
    var dragging by remember { mutableStateOf(false) }
    LaunchedEffect(list) {
        list.interactionSource.interactions.collect {
            when (it) {
                is DragInteraction.Start -> {
                    dragging = true
                    follow = false
                }
                is DragInteraction.Stop, is DragInteraction.Cancel -> dragging = false
            }
        }
    }
    LaunchedEffect(follow, dragging) {
        if (!follow && !dragging) {
            delay(FOLLOW_PAUSE_MS)
            follow = true
        }
    }
    val header = if (note != null) 1 else 0
    var first by remember(lyrics) { mutableStateOf(true) }
    LaunchedEffect(lyrics, current, follow) {
        if (!follow || !synced) return@LaunchedEffect
        val target = current.coerceAtLeast(0) + header
        if (first || !motion.enabled) list.scrollToItem(target) else list.animateScrollToItem(target)
        first = false
    }
    val fontSize = size.normal() * if (LocalPlayerCompact.current) 0.9f else 1f
    val style = MaterialTheme.typography.titleLarge.copy(fontSize = fontSize, lineHeight = fontSize * 1.3f, fontWeight = FontWeight.SemiBold)
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val padding = if (synced) PaddingValues(top = maxHeight / 3, bottom = maxHeight * 2 / 3) else PaddingValues(vertical = 16.dp)
        LazyColumn(state = list, modifier = Modifier.fillMaxSize(), contentPadding = padding) {
            if (note != null) {
                item(key = "note") {
                    Text(
                        stringResource(note),
                        style = MaterialTheme.typography.labelMedium,
                        color = palette.dim,
                        textAlign = TextAlign.Center,
                        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
                    )
                }
            }
            items(lyrics.lines.size, key = { it }) { i ->
                val line = lyrics.lines[i]
                val now = synced && i == current
                val alpha by animateFloatAsState(if (!synced || now) 1f else if (i < current) 0.5f else 0.75f, motion.tween(), label = "alpha")
                // SUBTLE: the sung line only brightens, it doesn't grow
                val scale by animateFloatAsState(if (!synced || now || !motion.moves) 1f else 0.92f, motion.move(), label = "scale")
                Text(
                    // a break: notes while following the song, a plain gap between verses otherwise
                    if (!line.blank) line.text else if (synced) stringResource(R.string.lyrics_gap) else "",
                    style = style,
                    color = if (!synced || now) palette.ink else palette.dim,
                    textAlign = TextAlign.Center,
                    modifier = Modifier
                        .fillMaxWidth()
                        .graphicsLayer {
                            this.alpha = alpha
                            scaleX = scale
                            scaleY = scale
                        }
                        .then(
                            if (onLine != null) {
                                Modifier.clickable(onClickLabel = stringResource(R.string.lyrics_seek_line)) {
                                    follow = true
                                    onLine(lyrics.timeOf(i))
                                }
                            } else {
                                Modifier
                            },
                        )
                        .padding(horizontal = 12.dp, vertical = 6.dp),
                )
            }
        }
        AnimatedVisibility(
            visible = synced && !follow,
            enter = if (motion.moves) fadeIn(motion.tween(200)) + slideInVertically(motion.move()) { it } else fadeIn(motion.tween(150)),
            exit = if (motion.moves) fadeOut(motion.tween(150)) + slideOutVertically(motion.move()) { it } else fadeOut(motion.tween(120)),
            modifier = Modifier.align(Alignment.BottomCenter).padding(bottom = 12.dp),
        ) {
            Button(onClick = { follow = true }) {
                Icon(Icons.Rounded.MyLocation, null, Modifier.size(18.dp))
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.lyrics_back_to_current))
            }
        }
    }
}

// ------------------------------------------------------------------ karaoke




/**
 * One big line filling with the accent as it's sung, with HomeFito holding the mic below
 * (or beside, on a wide screen). Only the sweep is redrawn each frame; lines change through
 * composition a few times a minute.
 */
@Composable
private fun KaraokeLyrics(lyrics: Lyrics, time: State<Long>, palette: LyricsPalette, size: LyricsSize, mascot: Boolean, playing: Boolean) {
    val motion = LocalMotion.current
    val slot by remember(lyrics) { derivedStateOf { karaokeFrame(lyrics, time.value) } }
    val compact = LocalPlayerCompact.current
    val scale = if (compact) 0.85f else 1f
    val big = size.karaoke() * scale
    val small = size.companion() * scale
    val bigStyle = MaterialTheme.typography.headlineMedium.copy(fontSize = big, lineHeight = big * 1.2f, fontWeight = FontWeight.Bold)
    val smallStyle = MaterialTheme.typography.titleMedium.copy(fontSize = small, lineHeight = small * 1.25f, fontWeight = FontWeight.Medium)

    val lines: @Composable (Modifier) -> Unit = { m ->
        KaraokeLines(lyrics, slot, time, palette, bigStyle, smallStyle, m)
    }

    BoxWithConstraints(Modifier.fillMaxSize()) {
        val w = maxWidth
        val h = maxHeight
        val singing = playing && slot.line >= 0
        when {
            !mascot -> lines(Modifier.fillMaxSize())
            w > h && w >= 480.dp -> Row(Modifier.fillMaxSize(), verticalAlignment = Alignment.CenterVertically) {
                lines(Modifier.weight(1f).fillMaxHeight())
                MascotView(Modifier.width(min(w.value * 0.34f, h.value).dp).fillMaxHeight(), singing = singing, danceButton = false)
            }
            else -> Column(Modifier.fillMaxSize()) {
                lines(Modifier.weight(1f).fillMaxWidth())
                MascotView(Modifier.fillMaxWidth().height((h * 0.36f).coerceIn(90.dp, 190.dp)), singing = singing, danceButton = false)
            }
        }
    }
}

/**
 * The karaoke lines for the picture-in-picture window: the line being sung (filling with the
 * accent as it goes) and the next one, no controls, no mascot. Same timing and sweep as
 * [KaraokeLyrics] (both draw frames from [karaokeFrame]).
 */
@Composable
internal fun PipLyrics(lyrics: Lyrics, state: PlayerUiState, palette: LyricsPalette, offsetMs: Int) {
    val time = rememberLyricsTime(state, offsetMs)
    val slot by remember(lyrics) { derivedStateOf { karaokeFrame(lyrics, time.value) } }
    val big = MaterialTheme.typography.titleMedium.copy(fontSize = 17.sp, lineHeight = 21.sp, fontWeight = FontWeight.Bold)
    val small = MaterialTheme.typography.bodySmall.copy(fontSize = 12.sp, lineHeight = 15.sp)
    Column(
        Modifier.fillMaxSize().padding(horizontal = 10.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        if (slot.line >= 0) {
            SungLine(lyrics.lines[slot.line].text, big, palette) { karaokeProgress(lyrics, slot.line, time.value) }
        } else {
            Text(stringResource(R.string.lyrics_gap), style = big, color = palette.fill, textAlign = TextAlign.Center)
        }
        if (slot.next >= 0 && slot.next != slot.line) {
            Spacer(Modifier.height(6.dp))
            Text(
                lyrics.lines[slot.next].text,
                style = small,
                color = palette.dim,
                textAlign = TextAlign.Center,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}


