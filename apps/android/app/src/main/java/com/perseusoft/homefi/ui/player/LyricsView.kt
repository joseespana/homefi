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

/** A break longer than this between two lines shows the notes and a countdown in karaoke. */
private const val GAP_MS = 4_000L

/** A position report this far from the per-frame estimate moves the estimate (a seek, a stall). */
private const val RESYNC_MS = 150L

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
                    NormalLyrics(lyrics, time, palette, settings.lyricsSize, note, seek)
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

/**
 * Where the song is for the lyrics, moved every frame while it plays: the player only
 * reports its position a few times a second. [offsetMs] > 0 shows the lyrics later.
 */
@Composable
private fun rememberLyricsTime(state: PlayerUiState, offsetMs: Int): State<Long> {
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

// ------------------------------------------------------------------ normal

/**
 * The whole text, the line being sung bright and the rest dimmed, kept about a third from
 * the top. Scrolling by hand stops the following for a few seconds (with a chip to come
 * back at once); a tap on a line jumps there.
 */
@Composable
private fun NormalLyrics(lyrics: Lyrics, time: State<Long>, palette: LyricsPalette, size: LyricsSize, note: Int?, onSeek: ((Long) -> Unit)?) {
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
                            if (onSeek != null) {
                                Modifier.clickable(onClickLabel = stringResource(R.string.lyrics_seek_line)) {
                                    follow = true
                                    onSeek(lyrics.timeOf(i))
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
 * What karaoke shows at a moment: the line sung big ([line], -1 during a break), the one
 * before faint above, the one after small below, and when a break ends ([gapUntil]).
 */
@Immutable
private data class Slot(val line: Int, val previous: Int, val next: Int, val gapUntil: Long? = null)

private fun slotAt(l: Lyrics, t: Long): Slot {
    val i = l.lineAt(t)
    if (i >= 0 && !l.lines[i].blank) {
        val next = l.nextSung(i)
        val end = l.endOf(i)
        // sung, and a long instrumental follows
        if (t > end && next >= 0 && l.timeOf(next) - end > GAP_MS) return Slot(-1, i, next, l.timeOf(next))
        return Slot(i, l.previousSung(i), next)
    }
    val next = l.nextSung(i)
    if (next < 0) return Slot(-1, l.previousSung(l.lines.size), -1) // the song is over
    // before the first line, or a long break marked with an empty line
    if (i < 0 || l.timeOf(next) - l.timeOf(i) > GAP_MS) return Slot(-1, l.previousSung(next), next, l.timeOf(next))
    // a short break: the next line waits, still unfilled
    return Slot(next, l.previousSung(next), l.nextSung(next))
}

/** How much of line [index] has been sung at [t], 0..1. */
private fun progressOf(l: Lyrics, index: Int, t: Long): Float {
    val start = l.timeOf(index)
    if (t <= start) return 0f
    val end = l.endOf(index)
    return ((t - start).toFloat() / (end - start).coerceAtLeast(1)).coerceIn(0f, 1f)
}

/**
 * One big line filling with the accent as it's sung, with HomeFito holding the mic below
 * (or beside, on a wide screen). Only the sweep is redrawn each frame; lines change through
 * composition a few times a minute.
 */
@Composable
private fun KaraokeLyrics(lyrics: Lyrics, time: State<Long>, palette: LyricsPalette, size: LyricsSize, mascot: Boolean, playing: Boolean) {
    val motion = LocalMotion.current
    val slot by remember(lyrics) { derivedStateOf { slotAt(lyrics, time.value) } }
    val compact = LocalPlayerCompact.current
    val scale = if (compact) 0.85f else 1f
    val big = size.karaoke() * scale
    val small = size.companion() * scale
    val bigStyle = MaterialTheme.typography.headlineMedium.copy(fontSize = big, lineHeight = big * 1.2f, fontWeight = FontWeight.Bold)
    val smallStyle = MaterialTheme.typography.titleMedium.copy(fontSize = small, lineHeight = small * 1.25f, fontWeight = FontWeight.Medium)

    val lines: @Composable (Modifier) -> Unit = { m ->
        AnimatedContent(
            targetState = slot,
            // the old line is gone before the new one comes in, so they never overlap
            transitionSpec = {
                if (motion.moves) {
                    (fadeIn(motion.tween(200, delay = 110)) + slideInVertically(motion.tween(320)) { it / 6 }) togetherWith fadeOut(motion.tween(110))
                } else {
                    fadeIn(motion.tween(200, delay = 110)) togetherWith fadeOut(motion.tween(110))
                }
            },
            contentKey = { it.line to it.gapUntil },
            modifier = m,
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
                    SungLine(lyrics.lines[s.line].text, bigStyle, palette) { progressOf(lyrics, s.line, time.value) }
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

    BoxWithConstraints(Modifier.fillMaxSize()) {
        val w = maxWidth
        val h = maxHeight
        val singing = playing && slot.line >= 0
        when {
            !mascot -> lines(Modifier.fillMaxSize())
            w > h && w >= 480.dp -> Row(Modifier.fillMaxSize(), verticalAlignment = Alignment.CenterVertically) {
                lines(Modifier.weight(1f).fillMaxHeight())
                MascotView(Modifier.width(min(w.value * 0.34f, h.value).dp).fillMaxHeight(), singing = singing)
            }
            else -> Column(Modifier.fillMaxSize()) {
                lines(Modifier.weight(1f).fillMaxWidth())
                MascotView(Modifier.fillMaxWidth().height((h * 0.36f).coerceIn(90.dp, 190.dp)), singing = singing)
            }
        }
    }
}

/**
 * [text] in the dim colour, then again in the accent up to [progress] — across the wrapped
 * lines one after the other, with a soft leading edge. Drawn, not recomposed, every frame.
 */
@Composable
private fun SungLine(text: String, style: TextStyle, palette: LyricsPalette, progress: () -> Float) {
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
private fun Countdown(until: Long?, time: State<Long>, style: TextStyle, palette: LyricsPalette) {
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
