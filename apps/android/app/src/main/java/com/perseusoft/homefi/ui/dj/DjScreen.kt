package com.perseusoft.homefi.ui.dj

import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import com.perseusoft.homefi.ui.DialogAction
import com.perseusoft.homefi.ui.HomefiDialog
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.rounded.Adjust
import androidx.compose.material.icons.automirrored.rounded.TrendingUp
import androidx.compose.material.icons.rounded.Album
import androidx.compose.material.icons.rounded.VerticalAlignBottom
import androidx.compose.material.icons.rounded.FastRewind
import androidx.compose.material.icons.rounded.Bolt
import androidx.compose.material.icons.rounded.NotificationsActive
import androidx.compose.material.icons.rounded.Campaign
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.Info
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.DjSectionKind
import com.perseusoft.homefi.core.LoadTarget
import com.perseusoft.homefi.data.djLayout
import com.perseusoft.homefi.data.visibleSections
import androidx.compose.material.icons.rounded.Bookmarks
import androidx.compose.material.icons.rounded.LibraryMusic
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.ViewAgenda
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.runtime.key
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.material3.IconToggleButton
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Fill
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.drawscope.withTransform
import com.perseusoft.homefi.core.HomifitoMode
import com.perseusoft.homefi.core.HomifitoPlace
import com.perseusoft.homefi.data.toggleHomifito
import com.perseusoft.homefi.ui.player.Art
import com.perseusoft.homefi.core.SfxKind
import com.perseusoft.homefi.data.DJ_PADS
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.dj.DJ_DECKS
import com.perseusoft.homefi.ui.QualityBadge
import com.perseusoft.homefi.ui.haptics
import com.perseusoft.homefi.ui.theme.Card

/** Below this height the Mix row and the crossfader stay at the bottom of the screen. */
private const val SHORT_SCREEN_DP = 700

/** The "not bit-perfect" badge next to the title needs this much width; the info button says the same. */
private const val BADGE_MIN_WIDTH_DP = 420

/** From this height Homifito stays in view above or below the scrolling board; below it he scrolls with it. */
private const val PIN_STRIP_MIN_DP = 560

/** From this width the board gets wider margins and stops growing at [MAX_CONTENT_DP]. */
private const val ROOMY_SCREEN_DP = 840
private const val MAX_CONTENT_DP = 1200

/** Side by side decks from this width. */
private const val WIDE_SCREEN_DP = 600

private val PAD_HEIGHT = 56.dp

/** The mixer column is a bit narrower than a deck when they share a row. */
private const val MIXER_WEIGHT = 0.8f

/** The pad's name, from the core's sound. */
/** The pad's picture: the same outline icon set as the rest of the app. */
fun SfxKind.icon(): androidx.compose.ui.graphics.vector.ImageVector = when (this) {
    SfxKind.AIR_HORN -> Icons.Rounded.Campaign
    SfxKind.SIREN -> Icons.Rounded.NotificationsActive
    SfxKind.LASER -> Icons.Rounded.Bolt
    SfxKind.REWIND -> Icons.Rounded.FastRewind
    SfxKind.DROP -> Icons.Rounded.VerticalAlignBottom
    SfxKind.SCRATCH -> Icons.Rounded.Album
    SfxKind.RISER -> Icons.AutoMirrored.Rounded.TrendingUp
    SfxKind.HIT -> Icons.Rounded.Adjust
}

fun SfxKind.label(): Int = when (this) {
    SfxKind.AIR_HORN -> R.string.dj_pad_airhorn
    SfxKind.SIREN -> R.string.dj_pad_siren
    SfxKind.LASER -> R.string.dj_pad_laser
    SfxKind.REWIND -> R.string.dj_pad_rewind
    SfxKind.DROP -> R.string.dj_pad_drop
    SfxKind.SCRATCH -> R.string.dj_pad_scratch
    SfxKind.RISER -> R.string.dj_pad_riser
    SfxKind.HIT -> R.string.dj_pad_hit
}

/**
 * The simple DJ: two deck cards, Mix, the crossfader, the EQ sheet, four pads and the crate.
 * All the logic is in [com.perseusoft.homefi.player.dj.DjSession]; this only draws its state.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DjScreen(onClose: () -> Unit) {
    val app = LocalContext.current.homefi
    val session = app.dj
    val decks by session.decks.collectAsStateWithLifecycle()
    val mixer by session.mixer.collectAsStateWithLifecycle()
    val dj by app.settings.dj.collectAsStateWithLifecycle()
    val prefs by app.settings.settings.collectAsStateWithLifecycle()
    val notice by session.notice.collectAsStateWithLifecycle()
    val haptics = haptics()
    val snackbar = remember { SnackbarHostState() }
    var crateFor by rememberSaveable { mutableStateOf<Int?>(null) }
    var crateOpen by rememberSaveable { mutableStateOf(false) }
    var eqOpen by rememberSaveable { mutableStateOf(false) }
    var info by rememberSaveable { mutableStateOf(false) }
    var layoutOpen by rememberSaveable { mutableStateOf(false) }
    var setupsOpen by rememberSaveable { mutableStateOf(false) }

    BackHandler(onBack = onClose)

    val current = notice
    if (current != null) {
        val text = stringResource(current.res, *current.args.toTypedArray())
        LaunchedEffect(current.id) {
            snackbar.showSnackbar(text)
            session.dismissNotice(current.id)
        }
    }

    val actions = List(DJ_DECKS) { i ->
        DeckActions(
            choose = { crateFor = i; crateOpen = true },
            togglePlay = { session.togglePlay(i) },
            cueDown = { session.cueDown(i) },
            cueUp = { session.cueUp(i) },
            sync = { session.toggleSync(i) },
            seek = { session.seekTo(i, it) },
            tempo = { session.setTempoPct(i, it) },
            resetTempo = { session.resetTempo(i) },
            hotCue = { session.hotCue(i, it) },
            clearHotCue = { session.clearHotCue(i, it) },
            loop = { session.toggleLoop(i, it) },
            keyLock = { v -> app.settings.update { it.copy(djKeyLock = v) } },
        )
    }
    val mixerActions = MixerActions(
        mixTo = session::mixTo,
        stopMix = session::cancelMix,
        setBeats = { beats -> app.settings.setDj(dj.copy(autoMixBeats = beats.toUInt())) },
        crossfader = session::setCrossfader,
        openEq = { eqOpen = true },
    )
    val configuration = LocalConfiguration.current
    val short = configuration.screenHeightDp < SHORT_SCREEN_DP
    val wide = configuration.screenWidthDp >= WIDE_SCREEN_DP
    val pad = if (configuration.screenWidthDp >= ROOMY_SCREEN_DP) 24.dp else 12.dp

    val deckCard: @Composable (Int, Modifier) -> Unit = { i, modifier ->
        DjDeckCard(
            index = i,
            deck = decks[i],
            actions = actions[i],
            simple = prefs.djSimpleView,
            tempoRangePct = dj.tempoRangePct,
            keyLock = prefs.djKeyLock,
            waveformColors = dj.waveformColors,
            showBeats = dj.showBeatGrid,
            modifier = modifier,
        )
    }
    val layout = remember(prefs.djSections, prefs.djPads) { prefs.djLayout() }
    val stripHeight = if (dj.homifitoMode == HomifitoMode.OFF) null else homifitoStripHeightDp(configuration.screenHeightDp, dj.homifitoMode == HomifitoMode.BIG)
    val stripPinned = configuration.screenHeightDp >= PIN_STRIP_MIN_DP
    val strip: @Composable () -> Unit = {
        if (stripHeight != null) HomifitoStrip(decks, mixer, session.padEvents, stripHeight.dp)
    }
    val sections = remember(layout) { layout.visibleSections().map { it.kind } }
    val pinMixer = short && DjSectionKind.MIXER in sections
    val mixBlock: @Composable (Modifier) -> Unit = { modifier ->
        MixBlock(mixer, dj.autoMixBeats.toInt(), mixerActions, modifier, compact = short, horizontal = short && wide, column = wide && !short)
    }

    val look = if (dj.homifitoLook) homifitoLook() else null
    CompositionLocalProvider(LocalDjLook provides look) {
    Surface(Modifier.fillMaxSize().systemBarsPadding(), color = MaterialTheme.colorScheme.background) {
        Scaffold(
            topBar = {
                TopAppBar(
                    title = {
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            Text(stringResource(R.string.dj_title))
                            if (configuration.screenWidthDp >= BADGE_MIN_WIDTH_DP) QualityBadge(stringResource(R.string.dj_badge))
                        }
                    },
                    navigationIcon = {
                        IconButton(onClick = onClose) { Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.dj_back)) }
                    },
                    actions = {
                        HomifitoToggle(on = dj.homifitoMode != HomifitoMode.OFF, onClick = { haptics.tick(); app.settings.toggleHomifito() })
                        IconButton(onClick = { info = true }) { Icon(Icons.Rounded.Info, stringResource(R.string.dj_badge_info)) }
                        DjMenu(onLayout = { layoutOpen = true }, onSetups = { setupsOpen = true })
                    },
                )
            },
            snackbarHost = { SnackbarHost(snackbar) },
            bottomBar = {
                if (pinMixer) {
                    Surface(tonalElevation = 3.dp) { mixBlock(Modifier) }
                }
            },
            containerColor = MaterialTheme.colorScheme.background,
        ) { padding ->
            Column(Modifier.padding(padding).fillMaxSize(), horizontalAlignment = Alignment.CenterHorizontally) {
            val top = dj.homifitoPlace == HomifitoPlace.TOP
            if (stripPinned && top) Box(Modifier.widthIn(max = MAX_CONTENT_DP.dp).padding(horizontal = pad, vertical = 6.dp)) { strip() }
            Column(
                Modifier.weight(1f).widthIn(max = MAX_CONTENT_DP.dp).verticalScroll(rememberScrollState()).padding(horizontal = pad, vertical = 8.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                if (!stripPinned && top) strip()
                if (prefs.djShowTips) {
                    FirstMixCard { app.settings.update { it.copy(djShowTips = false) } }
                }
                DjSections(
                    sections = sections,
                    wide = wide,
                    mixerPinned = pinMixer,
                    deck = deckCard,
                    mixer = mixBlock,
                    pads = { Pads(layout.pads, onPad = { kind -> haptics.tick(); session.pad(kind) }) },
                    crate = { CrateButton { crateFor = null; crateOpen = true } },
                )
                TextButton(
                    onClick = { app.settings.update { it.copy(djSimpleView = !it.djSimpleView) } },
                    modifier = Modifier.align(Alignment.CenterHorizontally),
                ) { Text(stringResource(if (prefs.djSimpleView) R.string.dj_show_all else R.string.dj_show_simple)) }
                if (!stripPinned && !top) strip()
                Box(Modifier.height(8.dp))
            }
            if (stripPinned && !top) Box(Modifier.widthIn(max = MAX_CONTENT_DP.dp).padding(horizontal = pad, vertical = 6.dp)) { strip() }
            }
        }
    }
    }

    if (crateOpen) {
        DjCrate(
            target = crateFor,
            freeDeck = session.freeDeck(),
            tapLoads = dj.loadTarget == LoadTarget.FREE_DECK,
            onLoad = session::load,
            onDismiss = { crateOpen = false; crateFor = null },
        )
    }
    if (eqOpen) {
        EqSheet(
            decks = decks,
            mixer = mixer,
            maxBoostDb = dj.eqMaxBoostDb,
            simple = prefs.djSimpleView,
            onEq = session::setEq,
            onVolume = session::setVolume,
            onTrim = session::setTrimDb,
            onMaster = session::setMaster,
            onDismiss = { eqOpen = false },
        )
    }
    if (layoutOpen) DjLayoutSheet(onDismiss = { layoutOpen = false })
    if (setupsOpen) DjSetupsSheet(onDismiss = { setupsOpen = false })
    if (info) {
        HomefiDialog(
            title = stringResource(R.string.dj_badge),
            text = stringResource(R.string.dj_badge_info),
            icon = Icons.Rounded.Info,
            primary = DialogAction(stringResource(R.string.dj_ok)) { info = false },
            onDismiss = { info = false },
        )
    }
}

/** The screen's overflow menu: edit the layout, the saved setups. */
@Composable
private fun DjMenu(onLayout: () -> Unit, onSetups: () -> Unit) {
    var open by remember { mutableStateOf(false) }
    IconButton(onClick = { open = true }) { Icon(Icons.Rounded.MoreVert, stringResource(R.string.djs_menu)) }
    DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
        DropdownMenuItem(
            text = { Text(stringResource(R.string.djs_edit_layout)) },
            leadingIcon = { Icon(Icons.Rounded.ViewAgenda, null) },
            onClick = { open = false; onLayout() },
        )
        DropdownMenuItem(
            text = { Text(stringResource(R.string.djs_setups)) },
            leadingIcon = { Icon(Icons.Rounded.Bookmarks, null) },
            onClick = { open = false; onSetups() },
        )
    }
}

/** The sections of the layout in the rider's order; on a wide screen the decks and the mixer share one row. */
@Composable
private fun DjSections(
    sections: List<DjSectionKind>,
    wide: Boolean,
    mixerPinned: Boolean,
    deck: @Composable (Int, Modifier) -> Unit,
    mixer: @Composable (Modifier) -> Unit,
    pads: @Composable () -> Unit,
    crate: @Composable () -> Unit,
) {
    val inRow = sections.filter { it == DjSectionKind.DECK_A || it == DjSectionKind.MIXER || it == DjSectionKind.DECK_B }
    var rowDrawn = false
    sections.forEach { kind ->
        key(kind) {
            when (kind) {
                DjSectionKind.DECK_A, DjSectionKind.MIXER, DjSectionKind.DECK_B -> when {
                    wide -> if (!rowDrawn) {
                        rowDrawn = true
                        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                            inRow.filter { !mixerPinned || it != DjSectionKind.MIXER }.forEach { part ->
                                when (part) {
                                    DjSectionKind.DECK_A -> deck(0, Modifier.weight(1f))
                                    DjSectionKind.DECK_B -> deck(1, Modifier.weight(1f))
                                    else -> Column(Modifier.weight(MIXER_WEIGHT), verticalArrangement = Arrangement.Center) { mixer(Modifier) }
                                }
                            }
                        }
                    }
                    kind == DjSectionKind.DECK_A -> deck(0, Modifier)
                    kind == DjSectionKind.DECK_B -> deck(1, Modifier)
                    !mixerPinned -> mixer(Modifier)
                }
                DjSectionKind.SAMPLER -> pads()
                DjSectionKind.CRATE -> crate()
            }
        }
    }
}

/** The crate as a section: a wide button that opens the song picker (loads into the free deck). */
@Composable
private fun CrateButton(onClick: () -> Unit) {
    FilledTonalButton(onClick = onClick, modifier = Modifier.fillMaxWidth().height(PAD_HEIGHT)) {
        Icon(Icons.Rounded.LibraryMusic, null, Modifier.size(20.dp))
        Text(stringResource(R.string.djs_section_crate_open), Modifier.padding(start = 8.dp))
    }
}

@Composable
private fun FirstMixCard(onDismiss: () -> Unit) {
    Card(
        Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.secondaryContainer),
    ) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(stringResource(R.string.dj_first_title), style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onSecondaryContainer)
            Text(stringResource(R.string.dj_first_run), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSecondaryContainer)
            TextButton(onClick = onDismiss, modifier = Modifier.align(Alignment.End)) { Text(stringResource(R.string.dj_got_it)) }
        }
    }
}

/** The sampler: [DJ_PADS] pads, each with the sound the settings give it. */
@Composable
private fun Pads(kinds: List<SfxKind>, onPad: (SfxKind) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(stringResource(R.string.dj_sampler), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            kinds.take(DJ_PADS).forEach { kind ->
                FilledTonalButton(onClick = { onPad(kind) }, modifier = Modifier.weight(1f).height(PAD_HEIGHT), contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 4.dp)) {
                    Column(horizontalAlignment = Alignment.CenterHorizontally) {
                        Icon(kind.icon(), contentDescription = null, modifier = Modifier.size(20.dp))
                        Text(stringResource(kind.label()), maxLines = 1, style = MaterialTheme.typography.labelSmall)
                    }
                }
            }
        }
    }
}

/** The top bar's one tap: Homifito on (at the size he had) or off. A tiny mascot: house, headphone band, cups, eyes. */
@Composable
private fun HomifitoToggle(on: Boolean, onClick: () -> Unit) {
    val label = stringResource(R.string.dj_homifito_toggle)
    val hint = stringResource(if (on) R.string.dj_homifito_toggle_off else R.string.dj_homifito_toggle_on)
    val ink = if (on) MaterialTheme.colorScheme.onSecondaryContainer else MaterialTheme.colorScheme.onSurfaceVariant
    IconToggleButton(
        checked = on,
        onCheckedChange = { onClick() },
        modifier = Modifier.clip(CircleShape).background(if (on) MaterialTheme.colorScheme.secondaryContainer else androidx.compose.ui.graphics.Color.Transparent)
            .semantics { contentDescription = "$label. $hint" },
    ) {
        Canvas(Modifier.size(28.dp)) {
            withTransform({ scale(size.width / MASCOT_ART, size.height / MASCOT_ART, Offset.Zero) }) {
                drawPath(Art.house, ink.copy(alpha = 0.5f), style = Fill)
                drawPath(HEADBAND, ink, style = Stroke(70f, cap = StrokeCap.Round))
                drawPath(Art.cupL, ink)
                drawPath(Art.cupR, ink)
                drawPath(Art.eyes, ink)
            }
        }
    }
}

private const val MASCOT_ART = 1024f
private val HEADBAND = androidx.compose.ui.graphics.vector.PathParser().parsePathString("M226 560C226 128 798 128 798 560").toPath()

fun HomifitoMode.labelRes(): Int = when (this) {
    HomifitoMode.OFF -> R.string.dj_homifito_size_off
    HomifitoMode.BOOTH -> R.string.dj_homifito_size_booth
    HomifitoMode.BIG -> R.string.dj_homifito_size_big
}

fun HomifitoPlace.labelRes(): Int = when (this) {
    HomifitoPlace.TOP -> R.string.dj_homifito_place_top
    HomifitoPlace.BOTTOM -> R.string.dj_homifito_place_bottom
}
