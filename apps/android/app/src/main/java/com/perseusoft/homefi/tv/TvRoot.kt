package com.perseusoft.homefi.tv

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.DrawerValue
import androidx.tv.material3.ModalNavigationDrawer
import androidx.tv.material3.rememberDrawerState
import com.perseusoft.homefi.R
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.HomefiLogo
import com.perseusoft.homefi.tv.train.TvTrainController
import com.perseusoft.homefi.tv.train.TvTrainSection
import com.perseusoft.homefi.tv.train.TvRideOverlay
import com.perseusoft.homefi.player.exercise.TvClientUi
import com.perseusoft.homefi.data.UpdateStatus
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import com.perseusoft.homefi.R as AppR
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull

private const val MS_PER_SECOND = 1_000L

/** How long a chosen song may take to show up before the player opens anyway. */
private const val OPEN_PLAYER_WAIT_MS = 1_500L
private val SCRIM = Brush.horizontalGradient(listOf(Color(0xE6070A14), Color(0x99070A14), Color.Transparent))

/** A request to open a section (an intent, a test), optionally on one Settings page. */
data class TvOpenRequest(val section: TvSection, val settings: TvSettingsCategory? = null)

/**
 * The TV app: the drawer with every section, the section on the right, and over them the
 * full-screen player and the ambient screensaver. Section screens read the same state as the phone's.
 * [openPlayer] emits when an intent asks for the player (the media notification, a file).
 */
@Composable
fun TvRoot(idle: TvIdle, train: TvTrainController, openPlayer: Flow<Unit>, openSection: Flow<TvOpenRequest>, onExit: () -> Unit) {
    TvTheme {
        val look = LocalTvLook.current
        val motion = com.perseusoft.homefi.ui.theme.LocalMotion.current
        val bannerIn = slideInVertically(motion.tween(300)) { -it / 2 } + fadeIn(motion.tween(260))
        val bannerOut = slideOutVertically(motion.tween(200)) { -it / 2 } + fadeOut(motion.tween(160))
        val app = LocalContext.current.homefi
        val account by app.account.account.collectAsStateWithLifecycle()
        val accountLoaded by app.account.loaded.collectAsStateWithLifecycle()
        val settings by app.settings.settings.collectAsStateWithLifecycle()
        val player by app.player.state.collectAsStateWithLifecycle()
        val ambient by idle.ambient.collectAsStateWithLifecycle()
        val activity by idle.activity.collectAsStateWithLifecycle()
        val mouse = rememberMouseConnected()
        val tv = settings.tv
        val signedIn = account != null

        var section by rememberSaveable { mutableStateOf(TvSection.HOME) }
        // the Settings page to open on ("set up an AI" from the playlist chat); null = where the user left it
        var settingsPage by remember { mutableStateOf<TvSettingsCategory?>(null) }
        val navScope = androidx.compose.runtime.rememberCoroutineScope()
        val backdropCover = remember { TvBackdropCover() }
        val coverTint = rememberCoverTint(backdropCover.cover)
        // each section starts from the plain light
        LaunchedEffect(section) { backdropCover.cover = null }
        var playerOpen by rememberSaveable { mutableStateOf(false) }
        // asks the section that just came to put focus on its first element
        var entryKey by remember { mutableIntStateOf(0) }
        val drawerState = rememberDrawerState(DrawerValue.Closed)
        val itemFocus = remember { TvSection.entries.associateWith { FocusRequester() } }
        val sections = visibleSections(signedIn, tv.djEntry)

        // updates: look when the app comes to the front, download in the background, ask once to install
        val update by app.updates.state.collectAsStateWithLifecycle()
        val notesPending by app.whatsNew.pending.collectAsStateWithLifecycle()
        var updateNotes by remember { mutableStateOf(false) }
        LifecycleEventEffect(Lifecycle.Event.ON_RESUME) { app.updates.onForeground() }
        LaunchedEffect(update.status, update.release?.version, tv.autoDownloadUpdates) {
            if (update.status == UpdateStatus.AVAILABLE && tv.autoDownloadUpdates) app.updates.download(installWhenReady = false)
        }

        // a phone's ride arrived while the user is somewhere else
        val ride by train.client.ui.collectAsStateWithLifecycle()
        val watching = ride as? TvClientUi.Watching
        var rideDismissed by remember { mutableStateOf<String?>(null) }
        val rideKey = watching?.endpoint?.let { "${it.host}:${it.port}" }
        val rideBanner = watching != null && section != TvSection.TRAIN && rideKey != rideDismissed

        LaunchedEffect(Unit) { openSection.collect { section = it.section; settingsPage = it.settings; entryKey++ } }
        LaunchedEffect(Unit) { openPlayer.collect { if (app.player.state.value.current != null) playerOpen = true } }
        // TIDAL is hidden while signed out: leave it if that happens under the user
        LaunchedEffect(section, sections) { if (section !in sections) section = TvSection.HOME }
        // screensaver: after the quiet time while music plays, and gone when it stops
        LaunchedEffect(player.isPlaying, tv.ambientSeconds, activity) {
            if (player.isPlaying && tv.ambientSeconds > 0) {
                delay(tv.ambientSeconds * MS_PER_SECOND)
                idle.enterAmbient()
            }
        }
        LaunchedEffect(player.isPlaying) { if (!player.isPlaying) idle.leaveAmbient() }

        // a download going to a USB drive without "All files access" asks for it; coming back from Android's page resumes it
        val downloads by app.downloads.state.collectAsStateWithLifecycle()
        val djOpen by app.dj.isOpen.collectAsStateWithLifecycle()
        val rideOpen by app.ride.isOpen.collectAsStateWithLifecycle()
        // closing DJ mode or the ride returns the focus to its page
        LaunchedEffect(djOpen, rideOpen) { if (!djOpen && !rideOpen) entryKey++ }
        val context = LocalContext.current
        LifecycleEventEffect(Lifecycle.Event.ON_RESUME) {
            val state = app.downloads.state.value
            if (state.pausedForAccess && com.perseusoft.homefi.data.Storage.hasSharedAccess(context)) app.downloads.resume()
        }

        val modal = update.needsPermission || updateNotes || notesPending.isNotEmpty() || downloads.askingAccess != null
        val loginShown = accountLoaded && !signedIn && (section == TvSection.HOME || section == TvSection.TIDAL)
        BackHandler(enabled = !playerOpen && !ambient && !modal) {
            when {
                // the sign-in screen is the top: back leaves the app from here
                loginShown && drawerState.currentValue == DrawerValue.Closed -> onExit()
                drawerState.currentValue == DrawerValue.Open ->
                    if (section == TvSection.HOME) onExit() else entryKey++
                else -> runCatching { itemFocus.getValue(section).requestFocus() }
            }
        }

        Box(Modifier.fillMaxSize().background(look.bg)) {
            TvBackdrop(Modifier.fillMaxSize(), tint = coverTint)
            // the shell stays composed under the player (its place is kept) but cannot take focus
            Box(Modifier.fillMaxSize().focusProperties { canFocus = !playerOpen && !modal && !djOpen && !rideOpen }) {
                ModalNavigationDrawer(
                    drawerContent = { value ->
                        val open = value == DrawerValue.Open
                        TvDrawer(
                            open = open,
                            current = section,
                            sections = sections,
                            djUsable = djAvailable(tv.djEntry, mouse),
                            requesters = itemFocus,
                            onPick = { picked ->
                                section = picked
                                entryKey++
                            },
                        )
                    },
                    drawerState = drawerState,
                    scrimBrush = SCRIM,
                ) {
                    androidx.compose.runtime.CompositionLocalProvider(
                        LocalTvDrawerEntry provides itemFocus.getValue(section),
                        LocalTvBackdropCover provides backdropCover,
                        LocalTvNav provides remember { TvNav(go = { target -> section = target; entryKey++ }, openSettings = { page -> settingsPage = page; section = TvSection.SETTINGS; entryKey++ }, openPlayer = {
                            // the song that was just chosen arrives a moment later: open when it does (or at once if the same one is already there)
                            val before = app.player.state.value.current?.mediaId
                            navScope.launch {
                                withTimeoutOrNull(OPEN_PLAYER_WAIT_MS) { app.player.state.first { it.current != null && it.current?.mediaId != before } }
                                if (app.player.state.value.current != null) playerOpen = true
                            }
                        }) },
                    ) {
                    Box(Modifier.fillMaxSize().padding(start = TvDimens.contentStart)) {
                      // fade-through between sections, with a short slide when motion allows
                      AnimatedContent(
                        // the entry key rides in the state, so a screen that is leaving keeps its own and never steals focus back
                        targetState = Triple(section, loginShown, entryKey),
                        contentKey = { it.first to it.second },
                        transitionSpec = {
                            val m = motion
                            val slide = if (m.moves) slideInHorizontally(m.tween(260)) { it / 14 } else null
                            val enter = fadeIn(m.tween(220, delay = 90)) + (slide ?: androidx.compose.animation.EnterTransition.None)
                            enter togetherWith fadeOut(m.tween(90))
                        },
                        label = "tv-section",
                      ) { (shownSection, shownLogin, shownKey) ->
                        SectionContent(
                            section = shownSection,
                            loginShown = shownLogin,
                            accountLoaded = accountLoaded,
                            djUsable = djAvailable(tv.djEntry, mouse),
                            settingsPage = settingsPage,
                            entryKey = shownKey,
                            onOpenPlayer = { playerOpen = true },
                            onGo = { target ->
                                section = target
                                entryKey++
                            },
                            onLoggedIn = { if (!app.account.isLoggedIn) app.account.refresh() },
                            train = train,
                        )
                      }
                    }
                    }
                }
            }
            val safe = tvSafePadding()
            // banners over the shell (not over the full-screen player)
            Column(Modifier.align(Alignment.TopEnd).padding(safe).padding(top = 6.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                AnimatedVisibility(!playerOpen && !modal && updateCardVisible(update), enter = bannerIn, exit = bannerOut) {
                    TvUpdateCard(update, onWhatsNew = { updateNotes = true })
                }
                AnimatedVisibility(rideBanner && !modal, enter = bannerIn, exit = bannerOut) {
                    TvRideBanner(
                        phone = watching?.endpoint?.name.orEmpty(),
                        autoOpenSeconds = tv.rideAutoOpenSeconds,
                        paused = playerOpen,
                        onWatch = { rideDismissed = rideKey; playerOpen = false; section = TvSection.TRAIN; entryKey++ },
                        onLater = { rideDismissed = rideKey },
                    )
                }
            }
            AnimatedVisibility(
                playerOpen,
                enter = fadeIn(motion.tween(280)) + (if (motion.moves) scaleIn(motion.tween(320), initialScale = 0.94f) else androidx.compose.animation.EnterTransition.None),
                exit = fadeOut(motion.tween(200)) + (if (motion.moves) scaleOut(motion.tween(220), targetScale = 0.97f) else androidx.compose.animation.ExitTransition.None),
            ) {
                TvNowPlaying(
                    onClose = {
                        playerOpen = false
                        entryKey++
                    },
                    onSound = {
                        playerOpen = false
                        section = TvSection.SOUND
                        entryKey++
                    },
                )
            }
            // DJ mode: the phone's two decks over the TV, for the mouse
            AnimatedVisibility(djOpen, enter = fadeIn(motion.tween(260)), exit = fadeOut(motion.tween(200))) {
                Box(Modifier.fillMaxSize().background(look.bg)) { com.perseusoft.homefi.ui.dj.DjScreen(onClose = app.dj::close) }
            }
            // a ride run on this TV: the core's page, then its summary
            AnimatedVisibility(rideOpen, enter = fadeIn(motion.tween(260)), exit = fadeOut(motion.tween(200))) { TvRideOverlay() }
            AnimatedVisibility(ambient && player.current != null, enter = fadeIn(motion.tween(900)), exit = fadeOut(motion.tween(250))) { TvAmbient(player) }
            if (downloads.askingAccess != null) {
                TvUsbGuide(
                    onOpenSettings = {
                        app.downloads.answerAccess(com.perseusoft.homefi.data.UsbAccessAnswer.OPEN_SETTINGS)
                        openUsbAccessSettings(context)
                    },
                    onAppFolder = { app.downloads.answerAccess(com.perseusoft.homefi.data.UsbAccessAnswer.APP_FOLDER) },
                )
            } else if (update.needsPermission) {
                TvInstallPermission(onLater = app.updates::skipPermission)
            } else if (updateNotes) {
                TvTextDialog(
                    stringResource(AppR.string.update_whats_new_title, update.release?.version.orEmpty()),
                    updateNotesAnnotated(update.release?.notes.orEmpty(), stringResource(AppR.string.update_no_notes)),
                ) { updateNotes = false }
            } else if (notesPending.isNotEmpty()) {
                TvTextDialog(
                    stringResource(AppR.string.update_whats_new),
                    releaseNotesAnnotated(notesPending),
                    onClose = app.whatsNew::dismiss,
                )
            }
        }
    }
}

@Composable
private fun SectionContent(
    section: TvSection,
    loginShown: Boolean,
    accountLoaded: Boolean,
    djUsable: Boolean,
    settingsPage: TvSettingsCategory?,
    entryKey: Int,
    onOpenPlayer: () -> Unit,
    onGo: (TvSection) -> Unit,
    onLoggedIn: () -> Unit,
    train: TvTrainController,
) {
    val home = { onGo(TvSection.HOME) }
    when {
        (section == TvSection.HOME || section == TvSection.TIDAL) && !accountLoaded -> TvLoading()
        loginShown -> TvLogin(entryKey, onLoggedIn)
        section == TvSection.HOME -> TvHome(entryKey, onOpenPlayer) { onGo(TvSection.LIBRARY) }
        section == TvSection.LIBRARY -> TvLibrary(entryKey)
        section == TvSection.SEARCH -> TvSearch(entryKey)
        section == TvSection.TIDAL -> TvTidal(entryKey)
        section == TvSection.DOWNLOADS -> TvDownloads(entryKey)
        section == TvSection.PLAYLISTS -> TvPlaylists(entryKey)
        section == TvSection.SOUND -> TvSound(entryKey)
        section == TvSection.PARTY -> TvParty(entryKey)
        section == TvSection.TRAIN -> TvTrainSection(train, entryKey, Modifier.fillMaxSize())
        section == TvSection.DJ -> TvDj(entryKey, djUsable, home)
        section == TvSection.SETTINGS -> TvSettings(entryKey, settingsPage)
        else -> TvComingSoon(section, entryKey, home)
    }
}

// ------------------------------------------------------------------ drawer

private const val DRAWER_MS = 250
private val ITEM_HEIGHT = 34.dp

/**
 * The rail (icons, the current section marked by an accent bar and a tinted circle) that opens
 * to icons and names while it has focus; the focused item is a white pill with the accent ring.
 */
@OptIn(androidx.compose.ui.ExperimentalComposeUiApi::class)
@Composable
private fun TvDrawer(
    open: Boolean,
    current: TvSection,
    sections: List<TvSection>,
    djUsable: Boolean,
    requesters: Map<TvSection, FocusRequester>,
    onPick: (TvSection) -> Unit,
) {
    val look = LocalTvLook.current
    val motion = com.perseusoft.homefi.ui.theme.LocalMotion.current
    val width by animateDpAsState(if (open) TvDimens.drawerOpen else TvDimens.rail, motion.tween(DRAWER_MS), label = "tv-drawer")
    val backdrop = if (open) {
        Brush.horizontalGradient(listOf(Color(0xFF070A14), Color(0xFF070A14), Color(0xD9070A14), Color.Transparent))
    } else {
        Brush.horizontalGradient(listOf(Color(0xF5060810), Color(0xBF060810), Color.Transparent))
    }
    Column(
        Modifier
            .width(width + 36.dp)
            .fillMaxHeight()
            .background(backdrop)
            .padding(start = 24.dp, top = TvDimens.safeVertical, bottom = TvDimens.safeVertical, end = 36.dp)
            // coming in from the content lands on the current section, not on the first item
            .focusProperties { enter = { requesters.getValue(current) } },
        verticalArrangement = Arrangement.Top,
    ) {
        Row(Modifier.height(36.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Box(Modifier.width(TvDimens.itemHeight), contentAlignment = Alignment.Center) { HomefiLogo(28.dp) }
            if (open) Text(stringResource(R.string.app_name), style = TvType.display.copy(fontSize = 20.sp), color = look.ink, maxLines = 1)
        }
        Spacer(Modifier.height(9.dp))
        sections.forEach { s ->
            DrawerItem(
                section = s,
                open = open,
                selected = s == current,
                dimmed = s == TvSection.DJ && !djUsable,
                requester = requesters.getValue(s),
                onClick = { onPick(s) },
            )
        }
    }
}

@Composable
private fun DrawerItem(section: TvSection, open: Boolean, selected: Boolean, dimmed: Boolean, requester: FocusRequester, onClick: () -> Unit) {
    val look = LocalTvLook.current
    val label = stringResource(section.label)
    Box(Modifier.height(ITEM_HEIGHT).fillMaxWidth()) {
        TvFocusable(
            onClick = onClick,
            modifier = Modifier
                .focusRequester(requester)
                .height(ITEM_HEIGHT)
                .then(if (open) Modifier.fillMaxWidth() else Modifier.width(TvDimens.itemHeight)),
            shape = CircleShape,
            focusedScale = 1.04f,
        ) { focused ->
            val tint = Modifier.background(
                when {
                    focused -> Color.White
                    selected -> look.accent.copy(alpha = 0.22f)
                    else -> Color.Transparent
                },
                CircleShape,
            )
            val ink = when {
                focused -> look.onFocus
                selected -> Color.White
                else -> look.ink2
            }
            Row(
                tint.fillMaxSize().alpha(if (dimmed) 0.55f else 1f).padding(start = 8.dp, end = if (open) 14.dp else 0.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                Icon(section.icon, label, tint = if (selected && !focused) look.accent2 else ink, modifier = Modifier.size(20.dp))
                if (open) {
                    Text(label, style = TvType.body, color = ink, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
                    if (dimmed) TvTag(stringResource(R.string.tv_nav_badge_mouse))
                }
            }
        }
        // the current section's accent bar on the rail's edge
        if (selected) {
            Box(
                Modifier
                    .align(Alignment.CenterStart)
                    .offset(x = (-14).dp)
                    .size(width = 3.dp, height = 16.dp)
                    .background(look.gradient, RoundedCornerShape(2.dp)),
            )
        }
    }
}
