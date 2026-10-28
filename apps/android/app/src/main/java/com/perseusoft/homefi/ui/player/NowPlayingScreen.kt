package com.perseusoft.homefi.ui.player

import androidx.compose.material3.HorizontalDivider
import com.perseusoft.homefi.player.EqPanel
import com.perseusoft.homefi.ui.sound.HowSoundsButton
import com.perseusoft.homefi.ui.sound.SoundBadge
import androidx.compose.material.icons.rounded.Hearing

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.Crossfade
import com.perseusoft.homefi.ui.theme.LocalMotion
import com.perseusoft.homefi.ui.theme.ProvideMotionStyle
import com.perseusoft.homefi.ui.theme.itemMotion
import com.perseusoft.homefi.ui.theme.sharedCover
import coil3.request.ImageRequest
import androidx.compose.ui.draw.blur
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.material3.LocalContentColor
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.material.icons.rounded.Language
import androidx.compose.material.icons.rounded.Lyrics
import androidx.compose.runtime.saveable.rememberSaveable
import com.perseusoft.homefi.lyrics.LyricsState
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import androidx.compose.material.icons.rounded.ViewCarousel
import androidx.compose.material.icons.rounded.Brightness3
import androidx.compose.material.icons.rounded.Equalizer
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.basicMarquee
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.Album
import androidx.compose.material.icons.rounded.Cloud
import androidx.compose.material.icons.rounded.Dashboard
import androidx.compose.material.icons.rounded.Download
import androidx.compose.material.icons.rounded.DownloadDone
import androidx.compose.material.icons.rounded.EmojiEmotions
import androidx.compose.material.icons.rounded.GraphicEq
import androidx.compose.material.icons.rounded.Groups
import androidx.compose.material.icons.rounded.Image
import androidx.compose.material.icons.rounded.KeyboardArrowDown
import androidx.compose.material.icons.rounded.MusicNote
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PhoneAndroid
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Repeat
import androidx.compose.material.icons.rounded.RepeatOne
import androidx.compose.material.icons.rounded.Shuffle
import androidx.compose.material.icons.rounded.SkipNext
import androidx.compose.material.icons.rounded.SkipPrevious
import androidx.compose.material.icons.rounded.Speed
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.media3.common.Player
import coil3.compose.AsyncImage
import com.perseusoft.homefi.R
import androidx.compose.ui.graphics.luminance
import com.perseusoft.homefi.ui.theme.rememberCoverLightness
import com.perseusoft.homefi.data.ImmersiveShade
import com.perseusoft.homefi.data.PlayerLayout
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.NowPlaying
import com.perseusoft.homefi.player.PlayerUiState
import com.perseusoft.homefi.player.tidalId
import com.perseusoft.homefi.ui.QualityBadge
import com.perseusoft.homefi.ui.theme.LocalSkin
import com.perseusoft.homefi.ui.theme.currentTitleColor
import com.perseusoft.homefi.ui.theme.skinPanel
import com.perseusoft.homefi.ui.theme.skinRowColors
import com.perseusoft.homefi.ui.formatDuration
import com.perseusoft.homefi.ui.library.CoverArt
import com.perseusoft.homefi.ui.share.MemberDot
import com.perseusoft.homefi.ui.share.ShareSheet
import kotlinx.coroutines.launch

/** One spacing scale for every layout, so gaps line up. */
private object Space {
    val xs = 4.dp
    val s = 8.dp
    val m = 12.dp
    val l = 16.dp
    val xl = 24.dp
}

private fun PlayerLayout.label(): Int = when (this) {
    PlayerLayout.CLASSIC -> R.string.layout_classic
    PlayerLayout.IMMERSIVE -> R.string.layout_immersive
    PlayerLayout.VINYL -> R.string.layout_vinyl
    PlayerLayout.STUDIO -> R.string.layout_studio
    PlayerLayout.RETRO_AMP -> R.string.layout_retro_amp
    PlayerLayout.LUNA -> R.string.layout_luna
    PlayerLayout.ALUMINUM -> R.string.layout_aluminum
    PlayerLayout.WEB2000 -> R.string.layout_web2000
}

private fun PlayerLayout.icon(): ImageVector = when (this) {
    PlayerLayout.CLASSIC -> Icons.Rounded.Album
    PlayerLayout.IMMERSIVE -> Icons.Rounded.Image
    PlayerLayout.VINYL -> Icons.Rounded.MusicNote
    PlayerLayout.STUDIO -> Icons.Rounded.Tune
    PlayerLayout.RETRO_AMP -> Icons.Rounded.Equalizer
    PlayerLayout.LUNA -> Icons.Rounded.Brightness3
    PlayerLayout.ALUMINUM -> Icons.Rounded.ViewCarousel
    PlayerLayout.WEB2000 -> Icons.Rounded.Language
}

/** Full-screen player: four layouts over the same controls. */
@Composable
fun NowPlayingScreen(onClose: () -> Unit) {
    BackHandler(onBack = onClose)
    PlayerContent(playerSource(), onClose = onClose)
}

/**
 * Below this height (what's left for a layout once the system bars and the
 * header are gone, ~360x640 dp players like the SR35) everything tightens up
 * so the cover keeps most of the screen.
 */
private val CompactHeight = 560.dp

/** True when the layout has a short screen to fill (see [CompactHeight]). */
internal val LocalPlayerCompact = staticCompositionLocalOf { false }

/**
 * The player itself, backdrop and system bars included: this phone's player
 * full screen, or a hub's song on a guest (see GuestScreen). A [header] takes
 * the top bar's place, on the player's background, and gets the player's
 * action buttons (layout, outputs, ...) to put in its own row, so there is
 * one bar instead of two. [onQueue] replaces the queue sheet, [showQueue]
 * hides that button, and [onClose] / [showShare] hide theirs when null / false.
 */
@Composable
fun PlayerContent(
    source: PlayerSource,
    onClose: (() -> Unit)? = null,
    onQueue: (() -> Unit)? = null,
    showShare: Boolean = true,
    showQueue: Boolean = true,
    header: (@Composable (actions: @Composable RowScope.() -> Unit) -> Unit)? = null,
) {
    val app = LocalContext.current.homefi
    val state by source.state.collectAsStateWithLifecycle()
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    var queueOpen by remember { mutableStateOf(false) }
    var showOutputs by remember { mutableStateOf(false) }
    var shareOpen by remember { mutableStateOf(false) }
    val hub by app.share.hub.collectAsStateWithLifecycle()

    val current = state.current
    val cover = current?.coverModel()
    val layout = settings.playerLayout

    // lyrics in place of the layout; back closes them first
    var lyricsOpen by rememberSaveable { mutableStateOf(false) }
    var lyricsClosedFor by rememberSaveable { mutableStateOf<String?>(null) }
    val mediaId = current?.mediaId
    val closeLyrics = {
        lyricsOpen = false
        lyricsClosedFor = mediaId
    }
    BackHandler(enabled = lyricsOpen, onBack = closeLyrics)
    LaunchedEffect(mediaId, lyricsOpen, settings.lyricsAutoOpen) {
        if (mediaId != null && (lyricsOpen || settings.lyricsAutoOpen)) app.lyrics.request(mediaId)
    }
    val hasLyrics by remember(mediaId) {
        app.lyrics.states.map { mediaId != null && it[mediaId] is LyricsState.Ready }.distinctUntilChanged()
    }.collectAsStateWithLifecycle(false)
    LaunchedEffect(mediaId, hasLyrics, settings.lyricsAutoOpen) {
        if (settings.lyricsAutoOpen && hasLyrics && lyricsClosedFor != mediaId) lyricsOpen = true
    }

    val actions: @Composable RowScope.() -> Unit = {
        PlayerActions(
            layout = layout,
            onLayout = { l -> app.settings.update { it.copy(playerLayout = l) } },
            onOutputs = { showOutputs = true },
            onQueue = if (showQueue) (onQueue ?: { queueOpen = true }) else null,
            hasQueue = onQueue != null || state.queue.isNotEmpty(),
            onShare = if (showShare) ({ shareOpen = true }) else null,
            sharing = hub.running,
            lyricsOpen = lyricsOpen,
            onLyrics = { if (lyricsOpen) closeLyrics() else lyricsOpen = true },
        )
    }

    val retro = retroBackground(layout)
    // opaque even when the skin's surfaces are glass (the app is underneath)
    val base = LocalSkin.current.base
    CompositionLocalProvider(LocalPlayerSource provides source) {
    ProvideMotionStyle(layout) {
    Box(
        Modifier
            .fillMaxSize()
            .background(retro ?: SolidColor(base))
            .clickable(enabled = false) {}, // swallow touches meant for the screen below
    ) {
        if (retro == null) Backdrop(cover, strong = layout == PlayerLayout.IMMERSIVE, shade = settings.immersiveShade)
        // retro skins draw their own chrome; the top bar takes their ink
        CompositionLocalProvider(LocalContentColor provides (retroInk(layout) ?: LocalContentColor.current)) {
        Column(Modifier.fillMaxSize().systemBarsPadding()) {
        if (header != null) header(actions)
        BoxWithConstraints(Modifier.fillMaxSize()) {
        val compact = maxHeight - (if (header == null) 48.dp else 0.dp) < CompactHeight
        CompositionLocalProvider(LocalPlayerCompact provides compact) {
        Column(
            Modifier
                .fillMaxSize()
                .padding(horizontal = Space.l, vertical = Space.xs),
        ) {
            if (header == null) TopBar(onClose = onClose, actions = actions)
            // the lyrics rise over the layout (and sink back); another layout fades through
            val motion = LocalMotion.current
            AnimatedContent(
                targetState = if (lyricsOpen) null else layout,
                transitionSpec = {
                    when {
                        targetState == null -> motion.vertical(up = true)
                        initialState == null -> motion.vertical(up = false)
                        else -> motion.fadeThrough()
                    }
                },
                modifier = Modifier.weight(1f).fillMaxWidth(),
                label = "player-layout",
            ) { shown ->
                Column(Modifier.fillMaxSize()) {
                    when (shown) {
                        null -> LyricsLayout(state, layout, onClose = closeLyrics)
                        PlayerLayout.CLASSIC -> ClassicLayout(state, cover)
                        PlayerLayout.IMMERSIVE -> ImmersiveLayout(state)
                        PlayerLayout.VINYL -> VinylLayout(state, cover)
                        PlayerLayout.STUDIO -> StudioLayout(state, cover)
                        PlayerLayout.RETRO_AMP -> RetroAmpLayout(state)
                        PlayerLayout.LUNA -> LunaLayout(state, cover)
                        PlayerLayout.ALUMINUM -> AluminumLayout(state, cover)
                        PlayerLayout.WEB2000 -> Web2000Layout(state, cover)
                    }
                }
            }
        }
        }
        }
        }
        }
    }

    if (showOutputs) OutputPicker(onDismiss = { showOutputs = false })
    if (queueOpen) QueueSheet(state, onDismiss = { queueOpen = false })
    if (shareOpen) ShareSheet(onDismiss = { shareOpen = false })
    }
    }
}

/**
 * The album behind everything: faint in most layouts, the whole backdrop in
 * the immersive one, always fading into the surface so text stays readable.
 */
@Composable
private fun Backdrop(cover: Any?, strong: Boolean, shade: ImmersiveShade) {
    val surface = LocalSkin.current.base
    val context = LocalContext.current
    // a new album dissolves into the old one
    Crossfade(cover, animationSpec = LocalMotion.current.tween(500), label = "backdrop") { shown ->
        if (shown != null) {
            // the desktop's blurred album: Modifier.blur needs Android 12, so the cover is
            // also decoded tiny and stretched, which blurs it on any version
            AsyncImage(
                model = if (strong) shown else ImageRequest.Builder(context).data(shown).size(24, 24).build(),
                contentDescription = null,
                contentScale = ContentScale.Crop,
                filterQuality = FilterQuality.High,
                alpha = if (strong) 1f else 0.75f,
                modifier = Modifier.fillMaxSize().then(if (strong) Modifier else Modifier.blur(24.dp)),
            )
        }
    }
    // immersive: how much the fade under the title hides the album (0 none, 1 strong)
    val lower = rememberCoverLightness(if (strong && shade == ImmersiveShade.AUTO) cover else null)
    val amount = when (shade) {
        ImmersiveShade.STRONG -> 1f
        ImmersiveShade.SOFT -> 0.5f
        ImmersiveShade.OFF -> 0f
        // the text is the surface's opposite: a cover close to the text's tone needs more shade
        ImmersiveShade.AUTO -> lower?.let { l -> if (surface.luminance() < 0.5f) l else 1f - l }
            ?.let { 0.35f + 0.65f * it }
            ?: 0.8f
    }
    val stops = if (strong) {
        listOf(surface.copy(alpha = 0.35f * amount), Color.Transparent, surface.copy(alpha = 0.92f * amount), surface.copy(alpha = amount))
    } else {
        listOf(surface.copy(alpha = 0.25f), surface.copy(alpha = 0.55f), surface.copy(alpha = 0.92f))
    }
    Box(Modifier.fillMaxSize().background(Brush.verticalGradient(stops)))
}

@Composable
private fun TopBar(onClose: (() -> Unit)?, actions: @Composable RowScope.() -> Unit) {
    Row(Modifier.fillMaxWidth().height(48.dp), verticalAlignment = Alignment.CenterVertically) {
        if (onClose != null) IconButton(onClick = onClose) { Icon(Icons.Rounded.KeyboardArrowDown, stringResource(R.string.player_close)) }
        Spacer(Modifier.weight(1f))
        actions()
    }
}

/** Layout picker, sharing, outputs and queue: the top bar's buttons, or a header's. */
@Composable
private fun PlayerActions(
    layout: PlayerLayout,
    onLayout: (PlayerLayout) -> Unit,
    onOutputs: () -> Unit,
    onQueue: (() -> Unit)?,
    hasQueue: Boolean,
    onShare: (() -> Unit)?,
    sharing: Boolean,
    lyricsOpen: Boolean,
    onLyrics: () -> Unit,
) {
    var menu by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { menu = true }) { Icon(Icons.Rounded.Dashboard, stringResource(R.string.layout_title)) }
        DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
            val app = LocalContext.current.homefi
            val eqActive by app.equalizer.active.collectAsStateWithLifecycle()
            DropdownMenuItem(
                text = { Text(stringResource(R.string.eq_title)) },
                leadingIcon = {
                    Icon(Icons.Rounded.GraphicEq, null, tint = if (eqActive) MaterialTheme.colorScheme.primary else LocalContentColor.current)
                },
                onClick = {
                    menu = false
                    app.equalizer.open(EqPanel.EQUALIZER)
                },
            )
            // "How does it sound?" is always here, whatever the button setting says
            if (playerCapabilities().remoteName == null) {
                DropdownMenuItem(
                    text = { Text(stringResource(R.string.sound_how)) },
                    leadingIcon = { Icon(Icons.Rounded.Hearing, null) },
                    onClick = {
                        menu = false
                        app.sound.openHow(true)
                    },
                )
            }
            HorizontalDivider()
            PlayerLayout.entries.forEach { l ->
                DropdownMenuItem(
                    text = { Text(stringResource(l.label())) },
                    leadingIcon = { Icon(l.icon(), null) },
                    trailingIcon = { RadioButton(selected = l == layout, onClick = null) },
                    onClick = {
                        menu = false
                        onLayout(l)
                    },
                )
            }
        }
    }
    // shared control / party: lit while this phone is the hub
    if (onShare != null) {
        IconButton(onClick = onShare) {
            Icon(
                Icons.Rounded.Groups,
                stringResource(R.string.sharing_title),
                tint = if (sharing) MaterialTheme.colorScheme.primary else LocalContentColor.current,
            )
        }
    }
    IconButton(onClick = onOutputs) { Icon(currentOutputKind().icon(), stringResource(R.string.output_title)) }
    // lyrics, lit while they're showing
    IconButton(onClick = onLyrics) {
        Icon(
            Icons.Rounded.Lyrics,
            stringResource(if (lyricsOpen) R.string.lyrics_close else R.string.lyrics_show),
            tint = if (lyricsOpen) com.perseusoft.homefi.ui.theme.lyricsPalette(layout).fill else LocalContentColor.current,
        )
    }
    if (onQueue != null) {
        IconButton(onClick = onQueue, enabled = hasQueue) {
            Icon(Icons.AutoMirrored.Rounded.QueueMusic, stringResource(R.string.player_queue))
        }
    }
}

// ------------------------------------------------------------------ layouts

/** Cover (or an effect) on top, everything centered. */
@Composable
private fun ColumnScope.ClassicLayout(state: PlayerUiState, cover: Any?) {
    val compact = LocalPlayerCompact.current
    Stage(state.slot(), Modifier.weight(1f).fillMaxWidth().padding(vertical = if (compact) Space.xs else Space.s))
    Info(state.slot(), centered = true)
    Spacer(Modifier.height(if (compact) Space.s else Space.m))
    StyledSeekBar(state)
    if (!compact) Spacer(Modifier.height(Space.s))
    Transport(state)
    Spacer(Modifier.height(if (compact) Space.s else Space.l))
}

/** The album fills the screen behind; text and controls sit on the fade. */
@Composable
private fun ColumnScope.ImmersiveLayout(state: PlayerUiState) {
    val compact = LocalPlayerCompact.current
    Spacer(Modifier.weight(1f))
    Info(state.slot(), centered = false, large = true)
    Spacer(Modifier.height(if (compact) Space.s else Space.l))
    StyledSeekBar(state)
    if (!compact) Spacer(Modifier.height(Space.s))
    Transport(state)
    Spacer(Modifier.height(if (compact) Space.m else Space.xl))
}

/** A record turning on the platter while the music plays. */
@Composable
private fun ColumnScope.VinylLayout(state: PlayerUiState, cover: Any?) {
    val compact = LocalPlayerCompact.current
    Box(Modifier.weight(1f).fillMaxWidth().padding(vertical = if (compact) Space.xs else Space.m), contentAlignment = Alignment.Center) {
        Vinyl(cover, state.isPlaying, Modifier.fillMaxHeight().aspectRatio(1f, matchHeightConstraintsFirst = true))
    }
    Info(state.slot(), centered = true)
    Spacer(Modifier.height(if (compact) Space.s else Space.m))
    StyledSeekBar(state)
    if (!compact) Spacer(Modifier.height(Space.s))
    Transport(state)
    Spacer(Modifier.height(if (compact) Space.s else Space.l))
}

/** Hi-fi deck: track strip, spectrum and VU meters. */
@Composable
private fun ColumnScope.StudioLayout(state: PlayerUiState, cover: Any?) {
    val compact = LocalPlayerCompact.current
    Row(Modifier.fillMaxWidth().padding(vertical = if (compact) Space.xs else Space.s), verticalAlignment = Alignment.CenterVertically) {
        CoverArt(cover, if (compact) 52.dp else 64.dp, radius = 10.dp)
        Spacer(Modifier.width(Space.m))
        Info(state.slot(), centered = false, modifier = Modifier.weight(1f))
    }
    // the spectrum takes what's left; the meters shrink with the screen, never past 150 dp
    BoxWithConstraints(Modifier.weight(1f).fillMaxWidth()) {
        val vu = (maxHeight * 0.42f).coerceIn(72.dp, 150.dp)
        Column(Modifier.fillMaxSize()) {
            Panel(Modifier.weight(1f)) { SpectrumView() }
            Spacer(Modifier.height(Space.s))
            Box(Modifier.fillMaxWidth().height(vu)) { VuMetersView() }
        }
    }
    Spacer(Modifier.height(if (compact) Space.s else Space.m))
    StyledSeekBar(state)
    if (!compact) Spacer(Modifier.height(Space.s))
    Transport(state)
    Spacer(Modifier.height(if (compact) Space.s else Space.l))
}

@Composable
private fun Panel(modifier: Modifier, content: @Composable BoxScope.() -> Unit) {
    Box(
        modifier
            .fillMaxWidth()
            .skinPanel(LocalSkin.current, MaterialTheme.colorScheme.surfaceContainerHigh.copy(alpha = 0.7f), MaterialTheme.shapes.large)
            .padding(Space.s),
        contentAlignment = Alignment.Center,
        content = content,
    )
}

// ------------------------------------------------------------------ pieces

/** What fills the classic layout's stage. */
private enum class StageView(val label: Int, val icon: ImageVector) {
    COVER(R.string.stage_cover, Icons.Rounded.Album),
    SPECTRUM(R.string.stage_spectrum, Icons.Rounded.GraphicEq),
    VU(R.string.stage_vu, Icons.Rounded.Speed),
    MASCOT(R.string.stage_mascot, Icons.Rounded.EmojiEmotions),
}

/** Width of the stage's picker (one 40 dp button wide). */
private val StagePicker = 44.dp

/**
 * Album or an effect: swipe, tap, or pick with the dots. The dots go below,
 * or beside the square when the stage is wide enough to spare a column on
 * each side (short screens), so the album gets the whole height.
 */
@Composable
private fun Stage(slot: TrackSlot, modifier: Modifier) {
    val app = LocalContext.current.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    val views = StageView.entries
    val pager = rememberPagerState(initialPage = settings.stage.coerceIn(0, views.lastIndex)) { views.size }
    LaunchedEffect(pager.settledPage) { app.settings.update { it.copy(stage = pager.settledPage) } }
    val page: @Composable (Modifier) -> Unit = { m ->
        HorizontalPager(state = pager, modifier = m) { page ->
            Box(
                Modifier
                    .fillMaxSize()
                    .clickable(interactionSource = null, indication = null) {
                        scope.launch { pager.animateScrollToPage((page + 1) % views.size) }
                    },
                contentAlignment = Alignment.Center,
            ) {
                when (views[page]) {
                    // the next song's cover slides in the way the queue moved
                    StageView.COVER -> TrackChange(slot) { s ->
                        CoverArt(
                            s.track?.coverModel(),
                            size = null,
                            radius = 16.dp,
                            modifier = Modifier
                                .fillMaxHeight()
                                .aspectRatio(1f, matchHeightConstraintsFirst = true)
                                .sharedCover(s.track?.mediaId),
                        )
                    }
                    StageView.SPECTRUM -> SpectrumView()
                    StageView.VU -> VuMetersView()
                    StageView.MASCOT -> MascotView()
                }
            }
        }
    }
    val dots: @Composable () -> Unit = {
        views.forEachIndexed { i, view ->
            val selected = pager.currentPage == i
            IconButton(onClick = { scope.launch { pager.animateScrollToPage(i) } }, modifier = Modifier.size(40.dp)) {
                Icon(
                    view.icon,
                    stringResource(view.label),
                    Modifier.size(20.dp),
                    tint = if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.55f),
                )
            }
        }
    }
    BoxWithConstraints(modifier) {
        if (maxWidth - maxHeight >= StagePicker * 2) {
            Row(Modifier.fillMaxSize(), verticalAlignment = Alignment.CenterVertically) {
                Spacer(Modifier.width(StagePicker))
                page(Modifier.weight(1f).fillMaxHeight())
                Column(Modifier.width(StagePicker), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(Space.xs)) { dots() }
            }
        } else {
            Column(Modifier.fillMaxSize(), horizontalAlignment = Alignment.CenterHorizontally) {
                page(Modifier.weight(1f).fillMaxWidth())
                Row(Modifier.padding(top = Space.xs), horizontalArrangement = Arrangement.spacedBy(Space.xs)) { dots() }
            }
        }
    }
}

/** Title, artist · album, then source, quality and a download button on one line. */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun Info(slot: TrackSlot, centered: Boolean, modifier: Modifier = Modifier, large: Boolean = false) {
    val current = slot.track
    val app = LocalContext.current.homefi
    val caps = playerCapabilities()
    val align = if (centered) Alignment.CenterHorizontally else Alignment.Start
    var queued by remember(current?.mediaId) { mutableStateOf(false) }
    val compact = LocalPlayerCompact.current
    val chain = signalChainShown(remote = caps.remoteName != null) && current != null
    Column(modifier.fillMaxWidth(), horizontalAlignment = align) {
        // a new song's title slides in the way the queue moved
        TrackChange(slot, Modifier.fillMaxWidth(), contentAlignment = if (centered) Alignment.TopCenter else Alignment.TopStart) { s ->
            val shown = s.track
            Column(Modifier.fillMaxWidth(), horizontalAlignment = align) {
                Text(
                    shown?.title.orEmpty(),
                    style = when {
                        large && !compact -> MaterialTheme.typography.headlineSmall
                        compact && !large -> MaterialTheme.typography.titleMedium
                        else -> MaterialTheme.typography.titleLarge
                    },
                    fontWeight = FontWeight.Bold,
                    maxLines = 1,
                    textAlign = if (centered) TextAlign.Center else TextAlign.Start,
                    modifier = Modifier.basicMarquee(), // long titles scroll instead of being cut
                )
                Text(
                    listOf(shown?.artist.orEmpty(), shown?.album.orEmpty()).filter(String::isNotBlank).joinToString(" · "),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        Spacer(Modifier.height(if (compact) Space.xs else Space.s))
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Space.s)) {
            val id = current?.mediaId?.let(::tidalId)
            // the chip gives way (ellipsis) before the quality badge does
            SourceChip(local = id == null, remote = caps.remoteName, Modifier.weight(1f, fill = false))
            // the signal chain below says the same (and more); without it, today's badges
            if (!chain) {
                current?.quality?.let { QualityBadge(it) }
                EqBadge(remote = caps.remoteName != null)
            }
            HowSoundsButton(remote = caps.remoteName != null || current == null)
            if (id != null && caps.canDownload) {
                // streaming from Tidal: one tap keeps it
                IconButton(
                    enabled = !queued,
                    onClick = {
                        app.downloads.enqueue(current.title, listOf("track/$id"))
                        queued = true
                    },
                    modifier = Modifier.size(32.dp),
                ) {
                    Icon(
                        if (queued) Icons.Rounded.DownloadDone else Icons.Rounded.Download,
                        stringResource(if (queued) R.string.tidal_queued else R.string.download),
                        Modifier.size(20.dp),
                        tint = MaterialTheme.colorScheme.primary,
                    )
                }
            }
        }
        if (chain) SignalChain(current, Modifier.padding(top = Space.xs), centered = centered)
        CrossfadeIndicator(Modifier.padding(top = Space.xs))
    }
}

/**
 * The signal badge ("EQ · HD 650 + Electronic · −3 dB") while the equalizer, a taste layer
 * or loudness works on what this phone plays (not for a hub's song heard on the hub); a tap
 * opens the equalizer.
 */
@Composable
internal fun EqBadge(remote: Boolean) {
    val app = LocalContext.current.homefi
    val guest by app.share.guest.collectAsStateWithLifecycle()
    SoundBadge(heardHere = !remote || (guest.playAlong && guest.state?.audio != null))
}

@Composable
private fun SourceChip(local: Boolean, remote: String?, modifier: Modifier = Modifier) {
    Row(
        modifier
            .clip(RoundedCornerShape(50))
            .background(MaterialTheme.colorScheme.surfaceContainerHigh)
            .padding(horizontal = 10.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            if (remote != null) Icons.Rounded.Groups else if (local) Icons.Rounded.PhoneAndroid else Icons.Rounded.Cloud,
            null,
            Modifier.size(14.dp),
            tint = if (local) MaterialTheme.colorScheme.secondary else MaterialTheme.colorScheme.primary,
        )
        Spacer(Modifier.width(6.dp))
        Text(
            // a guest: the hub plays it, wherever the hub got it from
            if (remote != null) {
                stringResource(R.string.player_source_hub, remote)
            } else {
                stringResource(if (local) R.string.player_source_local else R.string.player_source_tidal)
            },
            style = MaterialTheme.typography.labelMedium,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

/** Shuffle · previous · play · next · repeat, one even gap between all of them. */
@Composable
private fun Transport(state: PlayerUiState) {
    val player = playerSource()
    val caps = playerCapabilities()
    val active = MaterialTheme.colorScheme.primary
    val idle = MaterialTheme.colorScheme.onSurfaceVariant
    // short screens: a smaller play button, everything still 48 dp to touch
    val compact = LocalPlayerCompact.current
    val play = if (compact) 64.dp else 76.dp
    val skip = if (compact) 52.dp else 56.dp
    Row(
        Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(if (compact) Space.s else Space.xs, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        ControlButton(Icons.Rounded.Shuffle, R.string.player_shuffle, 48.dp, 24.dp, if (state.shuffle) active else idle, player::toggleShuffle, caps.canShuffleRepeat)
        ControlButton(Icons.Rounded.SkipPrevious, R.string.player_previous, skip, if (compact) 32.dp else 36.dp, MaterialTheme.colorScheme.onSurface, player::previous, caps.canPrevious)
        // a ring around play while the track loads (a Tidal stream being fetched)
        Box(contentAlignment = Alignment.Center) {
            if (state.isBuffering) CircularProgressIndicator(Modifier.size(play + 8.dp), strokeWidth = 3.dp)
            FilledIconButton(
                onClick = player::togglePlayPause,
                enabled = caps.canPlayPause,
                modifier = Modifier.size(play),
                colors = IconButtonDefaults.filledIconButtonColors(),
            ) {
                PlayPauseIcon(state.isPlaying, Modifier.size(if (compact) 38.dp else 44.dp))
            }
        }
        ControlButton(Icons.Rounded.SkipNext, R.string.player_next, skip, if (compact) 32.dp else 36.dp, MaterialTheme.colorScheme.onSurface, player::next, caps.canNext)
        ControlButton(
            if (state.repeatMode == Player.REPEAT_MODE_ONE) Icons.Rounded.RepeatOne else Icons.Rounded.Repeat,
            R.string.player_repeat,
            48.dp,
            24.dp,
            if (state.repeatMode == Player.REPEAT_MODE_OFF) idle else active,
            player::cycleRepeat,
            caps.canShuffleRepeat,
        )
    }
}

@Composable
private fun ControlButton(icon: ImageVector, label: Int, size: Dp, iconSize: Dp, tint: Color, onClick: () -> Unit, enabled: Boolean = true) {
    IconButton(onClick = onClick, enabled = enabled, modifier = Modifier.size(size)) {
        Icon(icon, stringResource(label), Modifier.size(iconSize), tint = if (enabled) tint else tint.copy(alpha = DISABLED_ALPHA))
    }
}

/** A black record with grooves and the cover as its label, turning at 33⅓ rpm. */
@Composable
private fun Vinyl(cover: Any?, spinning: Boolean, modifier: Modifier) {
    var angle by remember { mutableFloatStateOf(0f) }
    LaunchedEffect(spinning) {
        var last = 0L
        while (spinning) {
            withFrameNanos { now ->
                if (last != 0L) angle = (angle + (now - last) / 1e9f * 200f) % 360f
                last = now
            }
        }
    }
    val groove = Color.White.copy(alpha = 0.06f)
    Box(modifier.rotate(angle), contentAlignment = Alignment.Center) {
        Canvas(Modifier.fillMaxSize()) {
            val r = size.minDimension / 2
            drawCircle(Brush.radialGradient(listOf(Color(0xFF2A2A2E), Color(0xFF0B0B0D)), center, r), r)
            var gr = r * 0.95f
            while (gr > r * 0.4f) {
                drawCircle(groove, gr, center, style = Stroke(1.2f))
                gr -= r * 0.035f
            }
            // light catching the grooves
            drawArc(
                Brush.sweepGradient(listOf(Color.Transparent, Color.White.copy(alpha = 0.10f), Color.Transparent), center),
                -60f, 50f, true, Offset(center.x - r, center.y - r), androidx.compose.ui.geometry.Size(2 * r, 2 * r),
            )
        }
        CoverArt(cover, size = null, radius = 999.dp, modifier = Modifier.fillMaxSize(0.38f))
        Box(Modifier.size(10.dp).clip(CircleShape).background(MaterialTheme.colorScheme.surface))
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun QueueSheet(state: PlayerUiState, onDismiss: () -> Unit) {
    val app = LocalContext.current.homefi
    val player = playerSource()
    val caps = playerCapabilities()
    // while sharing: who added each song
    val hub by app.share.hub.collectAsStateWithLifecycle()
    val addedBy = remember(hub.state) {
        hub.state?.let { s -> (s.queue + listOfNotNull(s.nowPlaying)).associate { it.queueId.toLong() to it.addedBy } }.orEmpty()
    }
    ModalBottomSheet(onDismissRequest = onDismiss) {
        Text(
            stringResource(R.string.player_queue),
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.Bold,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
        )
        val listState = rememberLazyListState(initialFirstVisibleItemIndex = state.index.coerceAtLeast(0))
        LazyColumn(state = listState) {
            items(state.queue.size, key = { i -> "$i-${state.queue[i].mediaId}" }) { i ->
                val entry = state.queue[i]
                val isCurrent = i == state.index
                val skin = LocalSkin.current
                ListItem(
                    modifier = itemMotion().clickable(enabled = caps.canSkipTo) { player.skipTo(i) },
                    colors = if (isCurrent && skin.selection != null) {
                        skinRowColors(i, current = true)
                    } else {
                        ListItemDefaults.colors(containerColor = if (skin.stripe != null && i % 2 == 1) skin.stripe else MaterialTheme.colorScheme.surfaceContainerLow)
                    },
                    leadingContent = {
                        Box(contentAlignment = Alignment.Center) {
                            CoverArt(entry.coverModel(), 44.dp, radius = 6.dp)
                            if (isCurrent) {
                                Box(Modifier.size(44.dp).clip(RoundedCornerShape(6.dp)).background(Color.Black.copy(alpha = 0.45f)))
                                Icon(Icons.Rounded.GraphicEq, null, tint = Color.White)
                            }
                        }
                    },
                    headlineContent = {
                        Text(
                            entry.title,
                            color = currentTitleColor(isCurrent),
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    },
                    supportingContent = {
                        val by = entry.queueId?.let(addedBy::get)
                        if (by == null) {
                            Text(entry.artist, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        } else {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                MemberDot(by.color, 10.dp)
                                Spacer(Modifier.width(6.dp))
                                Text(stringResource(R.string.sharing_by, entry.artist, by.name), maxLines = 1, overflow = TextOverflow.Ellipsis)
                            }
                        }
                    },
                )
            }
        }
    }
}

// ------------------------------------------------------------------ track changes

/** A song and where it sits in the queue, so a change of song slides the way the queue moved. */
internal data class TrackSlot(val track: NowPlaying?, val index: Int, val last: Int)

internal fun PlayerUiState.slot() = TrackSlot(current, index, queue.lastIndex)

/** Next (or wrapping from the last song to the first) goes forward; anything else back. */
private fun forward(from: TrackSlot, to: TrackSlot): Boolean = when {
    from.index == from.last && to.index == 0 && from.last > 0 -> true
    from.index == 0 && to.index == from.last && from.last > 0 -> false
    else -> to.index >= from.index
}

/** [content] for the song in [slot]; a new song replaces the old one with the track transition. */
@Composable
internal fun TrackChange(
    slot: TrackSlot,
    modifier: Modifier = Modifier,
    contentAlignment: Alignment = Alignment.Center,
    content: @Composable (TrackSlot) -> Unit,
) {
    val motion = LocalMotion.current
    AnimatedContent(
        targetState = slot,
        contentKey = { it.track?.mediaId },
        transitionSpec = { motion.track(forward(initialState, targetState)) },
        contentAlignment = contentAlignment,
        modifier = modifier,
        label = "track",
    ) { content(it) }
}
