package com.perseusoft.homefi.tv

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.Crossfade
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
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
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.GraphicEq
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Repeat
import androidx.compose.material.icons.rounded.RepeatOne
import androidx.compose.material.icons.rounded.Shuffle
import androidx.compose.material.icons.rounded.SkipNext
import androidx.compose.material.icons.rounded.SkipPrevious
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.material.icons.rounded.Favorite
import androidx.compose.material.icons.rounded.FavoriteBorder
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.animation.core.spring
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.media3.common.Player
import com.perseusoft.homefi.R
import com.perseusoft.homefi.data.ImmersiveShade
import com.perseusoft.homefi.data.PlayerPanel
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.lyrics.LyricsState
import com.perseusoft.homefi.player.PlayerUiState
import com.perseusoft.homefi.player.QueueEntry
import com.perseusoft.homefi.ui.library.CoverArt
import com.perseusoft.homefi.ui.player.Backdrop
import com.perseusoft.homefi.ui.player.KaraokeLines
import com.perseusoft.homefi.ui.player.LyricsLines
import com.perseusoft.homefi.ui.player.MascotView
import com.perseusoft.homefi.ui.player.SpectrumView
import com.perseusoft.homefi.ui.player.coverModel
import com.perseusoft.homefi.ui.player.karaokeFrame
import com.perseusoft.homefi.ui.player.rememberLyricsTime
import com.perseusoft.homefi.ui.theme.LocalMotion
import com.perseusoft.homefi.ui.theme.LyricsPalette
import kotlinx.coroutines.delay

private const val SECONDS_MS = 1_000L
private const val SEEK_BADGE_MS = 900L
private const val CONTROLS_FADE_MS = 250
private const val FOCUS_RETRIES = 6
private const val FOCUS_RETRY_MS = 50L

private val PANELS = PlayerPanel.entries
private val COVER_SIZE = 300.dp

/**
 * The full-screen player (docs/TV_UI.md 6.5): cover, title and a glass panel with the tabs
 * Lyrics (karaoke) / Homifito / Visualizer / Queue, the progress and the controls under it.
 * The controls stay on screen; with `tv.controlsHideSeconds` > 0 they hide after that many quiet seconds.
 * Keys: [playerKeyAction]. BACK hides the controls first, then closes.
 */
@Composable
fun TvNowPlaying(onClose: () -> Unit, onSound: () -> Unit = {}) {
    val context = LocalContext.current
    val app = context.homefi
    val player = app.player
    val state by player.state.collectAsStateWithLifecycle()
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val tv = settings.tv
    val look = LocalTvLook.current

    var panel by remember { mutableStateOf(tv.playerPanel) }
    var controlsVisible by remember { mutableStateOf(true) }
    var keyTick by remember { mutableIntStateOf(0) }
    var seekBadge by remember { mutableStateOf<String?>(null) }
    var badgeTick by remember { mutableIntStateOf(0) }
    var heldSeek by remember { mutableStateOf(false) }
    val stateNow by rememberUpdatedState(state)
    val seekSeconds = tv.dpadSeekSeconds

    val root = remember { FocusRequester() }
    val playButton = remember { FocusRequester() }

    // shared element: the card the music started from hands over its place (taken once, never reused)
    val backdropCover = LocalTvBackdropCover.current
    val motionNow = LocalMotion.current
    val growFrom = remember { backdropCover.origin.takeIf { motionNow.sharedCover }.also { backdropCover.origin = null } }
    val grow = remember { androidx.compose.animation.core.Animatable(if (growFrom != null) 0f else 1f) }
    var coverBounds by remember { mutableStateOf<androidx.compose.ui.geometry.Rect?>(null) }
    LaunchedEffect(Unit) {
        if (growFrom != null) grow.animateTo(1f, spring(dampingRatio = 0.82f, stiffness = 260f))
    }

    // nothing left to show: back to where the user was
    LaunchedEffect(state.current == null) { if (state.current == null) onClose() }
    // the controls fade away after a few quiet seconds (any key starts the count again); 0 = never
    LaunchedEffect(controlsVisible, keyTick, tv.controlsHideSeconds) {
        if (controlsVisible && tv.controlsHideSeconds > 0) {
            delay(tv.controlsHideSeconds * SECONDS_MS)
            controlsVisible = false
        }
    }
    LaunchedEffect(badgeTick) {
        if (seekBadge != null) {
            delay(SEEK_BADGE_MS)
            seekBadge = null
        }
    }
    // focus: on the play button while the controls show, on the screen itself while they are hidden
    LaunchedEffect(controlsVisible) {
        val target = if (controlsVisible) playButton else root
        repeat(FOCUS_RETRIES) {
            if (runCatching { target.requestFocus() }.getOrDefault(false)) return@LaunchedEffect
            delay(FOCUS_RETRY_MS)
        }
    }
    BackHandler { if (controlsVisible) controlsVisible = false else onClose() }

    fun seek(seconds: Int) {
        val s = stateNow
        player.seekTo(seekTarget(s.positionMs, s.durationMs, seconds))
        seekBadge = if (seconds > 0) context.getString(R.string.tv_np_seek_forward, seconds) else context.getString(R.string.tv_np_seek_back, -seconds)
        badgeTick++
    }

    Box(
        Modifier
            .fillMaxSize()
            .focusRequester(root)
            .onPreviewKeyEvent { e ->
                val isDown = e.type == KeyEventType.KeyDown
                if (isDown) keyTick++
                val code = e.nativeKeyEvent.keyCode
                val repeats = e.nativeKeyEvent.repeatCount
                when (playerKeyAction(code, controlsVisible)) {
                    PlayerKeyAction.NONE -> false
                    PlayerKeyAction.PLAY_PAUSE -> {
                        if (isDown && repeats == 0) player.togglePlayPause()
                        true
                    }
                    PlayerKeyAction.LEFT, PlayerKeyAction.RIGHT -> {
                        val forward = code == android.view.KeyEvent.KEYCODE_DPAD_RIGHT
                        when {
                            isDown && repeats == 0 -> heldSeek = false
                            isDown && holdSeekStep(repeats) -> {
                                heldSeek = true
                                seek(if (forward) seekSeconds else -seekSeconds)
                            }
                            !isDown && e.type == KeyEventType.KeyUp -> {
                                if (!heldSeek) if (forward) player.next() else player.previous()
                                heldSeek = false
                            }
                        }
                        true
                    }
                    PlayerKeyAction.CYCLE_PANEL -> {
                        if (isDown && repeats == 0) panel = PANELS[(panel.ordinal + 1) % PANELS.size]
                        true
                    }
                    PlayerKeyAction.SHOW_CONTROLS -> {
                        if (isDown) controlsVisible = true
                        true
                    }
                    PlayerKeyAction.SEEK_FORWARD -> {
                        if (isDown) seek(seekSeconds)
                        true
                    }
                    PlayerKeyAction.SEEK_BACK -> {
                        if (isDown) seek(-seekSeconds)
                        true
                    }
                }
            }
            .focusable(),
    ) {
        val current = state.current
        val cover = current?.coverModel()
        Box(Modifier.fillMaxSize().background(look.bg))
        Backdrop(cover, strong = false, shade = ImmersiveShade.OFF)
        Box(Modifier.fillMaxSize().background(Brush.radialGradient(listOf(Color.Transparent, Color(0xB3020309)), Offset(960f, 500f), 1500f)))

        val safe = tvSafePadding()
        Column(Modifier.fillMaxSize().padding(safe)) {
            TopBar(state)
            Spacer(Modifier.height(14.dp))
            Row(Modifier.weight(1f).fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(36.dp)) {
                Column(Modifier.width(COVER_SIZE)) {
                    Box(
                        Modifier
                            .size(COVER_SIZE)
                            .onGloballyPositioned { coverBounds = it.boundsInRoot() }
                            .graphicsLayer {
                                // the cover grows out of the card it was started from (once), as the spring settles
                                val from = growFrom
                                val to = coverBounds
                                if (from != null && to != null && grow.value < 1f) {
                                    val t = grow.value
                                    val s0 = from.width / to.width
                                    scaleX = s0 + (1f - s0) * t
                                    scaleY = scaleX
                                    translationX = (from.center.x - to.center.x) * (1f - t)
                                    translationY = (from.center.y - to.center.y) * (1f - t)
                                }
                            }
                            .drawBehind {
                                drawCircle(
                                    Brush.radialGradient(listOf(look.accent.copy(alpha = 0.35f), Color.Transparent), center, size.maxDimension * 0.85f),
                                    radius = size.maxDimension * 0.85f,
                                )
                            },
                    ) {
                        // a new song's cover dissolves into the old one
                        Crossfade(cover, animationSpec = LocalMotion.current.tween(450), label = "tv-cover") { shown ->
                            CoverArt(shown, COVER_SIZE, radius = 18.dp)
                        }
                    }
                    Spacer(Modifier.height(14.dp))
                    Text(current?.title.orEmpty(), style = TvType.song, color = look.ink, maxLines = 2, overflow = TextOverflow.Ellipsis)
                    Text(
                        listOfNotNull(current?.artist, current?.album).filter { !it.isNullOrBlank() }.joinToString(" · "),
                        style = TvType.body.copy(fontWeight = FontWeight.Normal, fontSize = 17.sp),
                        color = look.ink2,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                Column(Modifier.weight(1f).fillMaxHeight()) {
                    PanelCard(
                        state = state,
                        panel = panel,
                        onPanel = { panel = it },
                        controlsVisible = controlsVisible,
                        modifier = Modifier.weight(1f),
                    )
                    Spacer(Modifier.height(14.dp))
                    Controls(state, controlsVisible, playButton, onSound)
                }
            }
            Spacer(Modifier.height(10.dp))
            TvKeyHints(
                listOf(
                    "OK" to stringResource(R.string.tv_np_hint_ok),
                    "◀ ▶" to stringResource(R.string.tv_np_hint_lr, seekSeconds),
                    "▲" to stringResource(R.string.tv_np_hint_up),
                    "▼" to stringResource(R.string.tv_np_hint_down),
                    "Back" to stringResource(R.string.tv_np_hint_back),
                ),
            )
        }
        seekBadge?.let { badge ->
            Text(
                badge,
                style = TvType.display.copy(fontSize = 30.sp),
                color = Color.White,
                modifier = Modifier
                    .align(Alignment.Center)
                    .background(Color.Black.copy(alpha = 0.55f), RoundedCornerShape(50))
                    .padding(horizontal = 28.dp, vertical = 12.dp),
            )
        }
    }
}

@Composable
private fun TopBar(state: PlayerUiState) {
    val look = LocalTvLook.current
    val output = outputLabel()
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        state.current?.quality?.let { TvTag(it, tone = look.ok) }
        output?.let { TvTag(it) }
        if (state.isBuffering) {
            CircularProgressIndicator(Modifier.size(16.dp), color = look.accent2, strokeWidth = 2.dp)
            Text(stringResource(R.string.tv_np_buffering), style = TvType.secondary, color = look.ink2)
        }
        Spacer(Modifier.weight(1f))
        TvClock(style = TvType.body.copy(fontWeight = FontWeight.Bold, fontSize = 15.sp))
    }
}

// ------------------------------------------------------------------ panel

@Composable
private fun PanelCard(
    state: PlayerUiState,
    panel: PlayerPanel,
    onPanel: (PlayerPanel) -> Unit,
    controlsVisible: Boolean,
    modifier: Modifier = Modifier,
) {
    val player = LocalContext.current.homefi.player
    val look = LocalTvLook.current
    val motion = LocalMotion.current
    TvGlass(modifier.fillMaxWidth()) {
        Column(Modifier.fillMaxSize().padding(horizontal = 20.dp, vertical = 16.dp)) {
            Row(
                Modifier.focusProperties { canFocus = controlsVisible },
                horizontalArrangement = Arrangement.spacedBy(7.dp),
            ) {
                PANELS.forEach { p ->
                    val label = panelLabel(p) + if (p == PlayerPanel.QUEUE && state.queue.isNotEmpty()) " · ${state.queue.size}" else ""
                    TvPill(label, { onPanel(p) }, selected = p == panel, height = 30.dp, labelStyle = TvType.body.copy(fontSize = 13.sp))
                }
            }
            Spacer(Modifier.height(10.dp))
            if (state.error != null) {
                Row(Modifier.fillMaxWidth().padding(bottom = 6.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    Text(stringResource(R.string.tv_np_error), style = TvType.body.copy(fontSize = 14.sp), color = look.error, modifier = Modifier.weight(1f), maxLines = 1)
                    TvPill(stringResource(R.string.player_next), player::next, height = 28.dp, labelStyle = TvType.secondary)
                }
            }
            Box(Modifier.weight(1f).fillMaxWidth()) {
                // the tabs slide the panel the way they are ordered
                AnimatedContent(
                    targetState = panel,
                    transitionSpec = {
                        val m = motion
                        val dir = if (targetState.ordinal >= initialState.ordinal) 1 else -1
                        val slide = if (m.moves) slideInHorizontally(m.tween(300)) { dir * it / 8 } else androidx.compose.animation.EnterTransition.None
                        (fadeIn(m.tween(220, delay = 80)) + slide) togetherWith fadeOut(m.tween(80))
                    },
                    label = "tv-panel",
                ) { shownPanel ->
                  when (shownPanel) {
                    PlayerPanel.LYRICS -> LyricsPanel(state)
                    PlayerPanel.MASCOT -> MascotView(Modifier.fillMaxSize(), singing = false, danceButton = false)
                    PlayerPanel.VISUALIZER -> SpectrumView(Modifier.fillMaxSize())
                    PlayerPanel.QUEUE -> QueuePanel(state, controlsVisible)
                  }
                }
            }
        }
    }
}

@Composable
private fun panelLabel(p: PlayerPanel): String = stringResource(
    when (p) {
        PlayerPanel.LYRICS -> R.string.tv_np_tab_lyrics
        PlayerPanel.MASCOT -> R.string.tv_np_tab_mascot
        PlayerPanel.VISUALIZER -> R.string.tv_np_tab_visualizer
        PlayerPanel.QUEUE -> R.string.tv_np_tab_queue
    },
)

@Composable
private fun LyricsPanel(state: PlayerUiState) {
    val app = LocalContext.current.homefi
    val look = LocalTvLook.current
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val mediaId = state.current?.mediaId
    LaunchedEffect(mediaId) { mediaId?.let(app.lyrics::request) }
    val all by app.lyrics.states.collectAsStateWithLifecycle()
    // the sung part sweeps in teal over white; the neighbours are the dim ink
    val palette = remember(look) { LyricsPalette(ink = look.ink, dim = look.ink2.copy(alpha = 0.85f), fill = look.accent2, fillEnd = Color.White) }
    when (val lyrics = mediaId?.let(all::get)) {
        is LyricsState.Ready -> {
            val time = rememberLyricsTime(state, settings.lyricsOffsetMs)
            if (lyrics.lyrics.synced) {
                val frame by remember(lyrics.lyrics) { derivedStateOf { karaokeFrame(lyrics.lyrics, time.value) } }
                val big = TvType.display.copy(fontSize = 30.sp, lineHeight = 36.sp, fontWeight = FontWeight.ExtraBold)
                val small = TvType.body.copy(fontSize = 17.sp, lineHeight = 22.sp)
                KaraokeLines(lyrics.lyrics, frame, time, palette, big, small, Modifier.fillMaxSize())
            } else {
                LyricsLines(lyrics.lyrics, time, palette, settings.lyricsSize, note = null, onLine = null)
            }
        }
        is LyricsState.Failed -> PanelMessage(lyrics.message)
        LyricsState.None -> PanelMessage(stringResource(R.string.lyrics_none))
        else -> LyricsSkeleton()
    }
}

@Composable
private fun PanelMessage(text: String) {
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Text(text, style = TvType.section, color = LocalTvLook.current.ink2, textAlign = TextAlign.Center)
    }
}

/** Calm placeholder lines while the lyrics are looked up. */
@Composable
private fun LyricsSkeleton() {
    Column(Modifier.fillMaxSize(), verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally) {
        listOf(0.55f, 0.8f, 0.45f).forEach { w ->
            Box(Modifier.fillMaxWidth(w).height(14.dp).background(Color.White.copy(alpha = 0.08f), RoundedCornerShape(50)))
            Spacer(Modifier.height(14.dp))
        }
    }
}

@Composable
private fun QueuePanel(state: PlayerUiState, controlsVisible: Boolean) {
    val player = LocalContext.current.homefi.player
    val list = rememberLazyListState(initialFirstVisibleItemIndex = (state.index - 1).coerceAtLeast(0))
    LaunchedEffect(state.index) { if (state.index >= 0) list.animateScrollToItem((state.index - 1).coerceAtLeast(0)) }
    LazyColumn(
        state = list,
        modifier = Modifier.fillMaxSize().focusProperties { canFocus = controlsVisible },
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        itemsIndexed(state.queue, key = { i, e -> "$i-${e.mediaId}" }) { i, entry ->
            QueueRow(entry, current = i == state.index, playing = state.isPlaying && i == state.index) { player.skipTo(i) }
        }
    }
}

@Composable
private fun QueueRow(entry: QueueEntry, current: Boolean, playing: Boolean, onClick: () -> Unit) {
    val look = LocalTvLook.current
    TvFocusable(
        onClick = onClick,
        modifier = Modifier.fillMaxWidth().height(44.dp),
        shape = RoundedCornerShape(12.dp),
        focusedScale = 1.02f,
        glow = false,
    ) { focused ->
        Row(
            Modifier.fillMaxSize().then(if (focused) Modifier.background(Color.White, RoundedCornerShape(12.dp)) else Modifier).padding(horizontal = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Box(contentAlignment = Alignment.Center) {
                CoverArt(entry.coverModel(), 32.dp, radius = 6.dp)
                if (current) {
                    Box(Modifier.size(32.dp).background(Color.Black.copy(alpha = 0.5f), RoundedCornerShape(6.dp)))
                    Icon(Icons.Rounded.GraphicEq, null, tint = look.accent2, modifier = Modifier.size(18.dp))
                }
            }
            Column(Modifier.weight(1f)) {
                val title = when {
                    focused -> look.onFocus
                    current -> look.accent2
                    else -> look.ink
                }
                Text(entry.title, style = TvType.cardTitle.copy(fontSize = 14.sp), color = title, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(entry.artist, style = TvType.secondary, color = if (focused) Color(0xFF3A4660) else look.ink2, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}

// ------------------------------------------------------------------ controls

@Composable
private fun Controls(state: PlayerUiState, visible: Boolean, playButton: FocusRequester, onSound: () -> Unit) {
    val app = LocalContext.current.homefi
    val player = app.player
    val look = LocalTvLook.current
    val motion = LocalMotion.current
    val alpha by animateFloatAsState(if (visible) 1f else 0f, motion.tween(CONTROLS_FADE_MS), label = "tv-controls")
    val fraction = if (state.durationMs > 0) state.positionMs.toFloat() / state.durationMs else 0f
    Box(Modifier.fillMaxWidth()) {
        // hidden: only a thin line of progress stays
        TvProgress(fraction, Modifier.alpha(1f - alpha).padding(top = 20.dp), height = 3.dp)
        Column(Modifier.alpha(alpha).focusProperties { canFocus = visible }) {
            TvProgress(fraction, thumb = true)
            Row(Modifier.fillMaxWidth().padding(top = 2.dp), horizontalArrangement = Arrangement.SpaceBetween) {
                Text(formatClock(state.positionMs), style = TvType.secondary.copy(fontSize = 13.sp), color = look.ink2)
                Text("-" + formatClock((state.durationMs - state.positionMs).coerceAtLeast(0)), style = TvType.secondary.copy(fontSize = 13.sp), color = look.ink2)
            }
            Spacer(Modifier.height(6.dp))
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(14.dp, Alignment.CenterHorizontally), verticalAlignment = Alignment.CenterVertically) {
                TvPill("", player::previous, icon = Icons.Rounded.SkipPrevious, height = 40.dp, modifier = Modifier.width(52.dp), description = stringResource(R.string.player_previous))
                TvPill(
                    "",
                    player::togglePlayPause,
                    Modifier.focusRequester(playButton).width(72.dp),
                    icon = if (state.isPlaying) Icons.Rounded.Pause else Icons.Rounded.PlayArrow,
                    primary = true,
                    height = 48.dp,
                    description = stringResource(if (state.isPlaying) R.string.player_pause else R.string.player_play),
                )
                TvPill("", player::next, icon = Icons.Rounded.SkipNext, height = 40.dp, modifier = Modifier.width(52.dp), description = stringResource(R.string.player_next))
                TvPill("", player::toggleShuffle, icon = Icons.Rounded.Shuffle, selected = state.shuffle, height = 40.dp, modifier = Modifier.width(52.dp), description = stringResource(R.string.player_shuffle))
                TvPill(
                    "",
                    player::cycleRepeat,
                    icon = if (state.repeatMode == Player.REPEAT_MODE_ONE) Icons.Rounded.RepeatOne else Icons.Rounded.Repeat,
                    selected = state.repeatMode != Player.REPEAT_MODE_OFF,
                    height = 40.dp,
                    modifier = Modifier.width(52.dp),
                    description = stringResource(R.string.player_repeat),
                )
                val liked by app.liked.ids.collectAsStateWithLifecycle()
                val id = state.current?.mediaId
                val isLiked = id != null && id in liked
                TvPill(
                    "",
                    { id?.let(app.liked::toggle) },
                    icon = if (isLiked) Icons.Rounded.Favorite else Icons.Rounded.FavoriteBorder,
                    selected = isLiked,
                    height = 40.dp,
                    modifier = Modifier.width(52.dp),
                    description = stringResource(R.string.tv_np_heart),
                )
                TvPill("", onSound, icon = Icons.Rounded.Tune, height = 40.dp, modifier = Modifier.width(52.dp), description = stringResource(R.string.sound_menu_item))
            }
        }
    }
}
