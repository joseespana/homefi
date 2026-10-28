package com.perseusoft.homefi.ui

import com.perseusoft.homefi.player.EqPanel
import com.perseusoft.homefi.ui.eq.AssistantScreen
import com.perseusoft.homefi.ui.eq.EqualizerScreen

import com.perseusoft.homefi.ui.haptics
import android.Manifest
import com.perseusoft.homefi.ui.home.HomeScreen
import androidx.compose.foundation.clickable
import androidx.compose.material.icons.rounded.Home
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.ExperimentalSharedTransitionApi
import androidx.compose.animation.SharedTransitionLayout
import androidx.compose.runtime.CompositionLocalProvider
import com.perseusoft.homefi.ui.theme.LocalCoverVisibility
import com.perseusoft.homefi.ui.theme.LocalMotion
import com.perseusoft.homefi.ui.theme.LocalSharedCover
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Download
import androidx.compose.material.icons.rounded.LibraryMusic
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material3.Badge
import androidx.compose.material3.BadgedBox
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.NavigationBarDefaults
import androidx.compose.material3.NavigationBarItemDefaults
import androidx.compose.ui.graphics.Color
import com.perseusoft.homefi.ui.theme.LocalSkin
import com.perseusoft.homefi.ui.theme.chrome
import com.perseusoft.homefi.ui.theme.skinCaps
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.downloads.DownloadsScreen
import com.perseusoft.homefi.ui.library.LibraryScreen
import com.perseusoft.homefi.ui.login.LoginScreen
import com.perseusoft.homefi.ui.login.TermsDialog
import kotlinx.coroutines.launch
import com.perseusoft.homefi.ui.player.MiniPlayer
import com.perseusoft.homefi.ui.player.NowPlayingScreen
import com.perseusoft.homefi.ui.search.SearchScreen
import com.perseusoft.homefi.ui.settings.SettingsScreen
import com.perseusoft.homefi.ui.share.GuestScreen
import com.perseusoft.homefi.ui.share.GuestPlayer
import com.perseusoft.homefi.ui.player.playerSource
import com.perseusoft.homefi.share.GuestStatus
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.remember
import com.perseusoft.homefi.ui.share.JoinDialog
import com.perseusoft.homefi.ui.share.NearbySessions
import com.perseusoft.homefi.ui.share.ShareNotices
import kotlinx.coroutines.flow.Flow
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.key
import com.perseusoft.homefi.ui.sound.HowSoundsSheet
import com.perseusoft.homefi.ui.sound.SoundForDialog
import com.perseusoft.homefi.ui.sound.SoundNotices

enum class Tab(val label: Int, val icon: ImageVector) {
    /** Reached with the logo in the top bar, not from the navigation bar. */
    HOME(R.string.home_title, Icons.Rounded.Home),
    LIBRARY(R.string.tab_library, Icons.Rounded.LibraryMusic),
    SEARCH(R.string.tab_search, Icons.Rounded.Search),
    DOWNLOADS(R.string.tab_downloads, Icons.Rounded.Download),
    SETTINGS(R.string.tab_settings, Icons.Rounded.Settings),
}

/** What the content area shows: a destination, or the login in its place. */
private data class Screen(val tab: Tab, val login: Boolean)

/** Navigation requests coming from intents (shared links, opened files, notification taps). */
enum class Destination { DOWNLOADS, PLAYER }

@OptIn(ExperimentalMaterial3Api::class, ExperimentalSharedTransitionApi::class)
@Composable
fun HomefiRoot(destinations: Flow<Destination>) {
    val app = LocalContext.current.homefi
    // read once off the main thread, again after logging in or out
    val loggedIn = app.account.account.collectAsStateWithLifecycle().value != null
    val accountKnown by app.account.loaded.collectAsStateWithLifecycle()
    var tab by rememberSaveable { mutableStateOf(Tab.HOME) }
    var playerOpen by rememberSaveable { mutableStateOf(false) }
    val downloads by app.downloads.state.collectAsStateWithLifecycle()
    val playing by app.player.state.collectAsStateWithLifecycle()
    val busy by Busy.count.collectAsStateWithLifecycle()
    val library by app.library.state.collectAsStateWithLifecycle()
    val guestOpen by app.share.guestOpen.collectAsStateWithLifecycle()
    // while this phone is in someone's room, "now playing" is the room's until it leaves
    val guestUi by app.share.guest.collectAsStateWithLifecycle()
    val inRoom = guestUi.status == GuestStatus.CONNECTED
    val scope = rememberCoroutineScope()
    val roomPlayer = remember(app.share) { GuestPlayer(app.share, loggedIn = app.account.account, scope = scope) }
    val ownPlayer = playerSource()
    val openPlayer = { if (app.share.guest.value.status == GuestStatus.CONNECTED) app.share.openGuest(true) else playerOpen = true }
    val joinRequest by app.share.joinRequest.collectAsStateWithLifecycle()
    val skin = LocalSkin.current

    val notifications = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) {}
    LaunchedEffect(loggedIn) {
        if (loggedIn && Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            notifications.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
    }
    LaunchedEffect(Unit) {
        app.player.connect() // restores a session that kept playing in the background
        destinations.collect {
            when (it) {
                Destination.DOWNLOADS -> tab = Tab.DOWNLOADS
                Destination.PLAYER -> openPlayer()
            }
        }
    }

    val motion = LocalMotion.current
    val nowPlayingShown = playerOpen && !inRoom && playing.current != null
    val body: @Composable () -> Unit = {
    Box(Modifier.fillMaxSize()) {
        Scaffold(
            topBar = {
                Column {
                val bar = skin.topBar
                TopAppBar(
                    title = {
                        // the logo goes home, like on the web
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            modifier = Modifier.clickable(onClickLabel = stringResource(R.string.home_title)) { tab = Tab.HOME },
                        ) {
                            HomefiLogo(28.dp)
                            Spacer(Modifier.width(10.dp))
                            Text(stringResource(R.string.app_name), fontWeight = FontWeight.Bold)
                        }
                    },
                    colors = if (bar == null) {
                        TopAppBarDefaults.topAppBarColors()
                    } else {
                        TopAppBarDefaults.topAppBarColors(
                            containerColor = Color.Transparent,
                            titleContentColor = bar.content,
                            navigationIconContentColor = bar.content,
                            actionIconContentColor = bar.content,
                        )
                    },
                    modifier = if (bar == null) Modifier else Modifier.chrome(bar, edgeAtTop = false),
                )
                LoadingBar(busy > 0 || library.scanning)
                }
            },
            bottomBar = {
                Column {
                    // its cover steps aside while a full-screen panel shows it (and grows into it)
                    MiniPlayer(source = if (inRoom) roomPlayer else ownPlayer, onOpen = openPlayer, coverShown = !nowPlayingShown && !guestOpen)
                    val bar = skin.navBar
                    NavigationBar(
                        containerColor = if (bar == null) NavigationBarDefaults.containerColor else Color.Transparent,
                        modifier = if (bar == null) Modifier else Modifier.chrome(bar, edgeAtTop = true),
                    ) {
                        val colors = if (bar == null) {
                            NavigationBarItemDefaults.colors()
                        } else {
                            NavigationBarItemDefaults.colors(
                                selectedIconColor = bar.onIndicator,
                                selectedTextColor = bar.onIndicator,
                                indicatorColor = bar.indicator,
                                unselectedIconColor = bar.contentVariant,
                                unselectedTextColor = bar.contentVariant,
                            )
                        }
                        val haptics = haptics()
                        Tab.entries.filter { it != Tab.HOME }.forEach { t ->
                            NavigationBarItem(
                                selected = tab == t,
                                onClick = {
                                    if (tab != t) haptics.tick()
                                    tab = t
                                },
                                icon = {
                                    if (t == Tab.DOWNLOADS && downloads.isBusy) {
                                        BadgedBox(badge = { Badge { Text("${downloads.active.size}") } }) { Icon(t.icon, null) }
                                    } else {
                                        Icon(t.icon, null)
                                    }
                                },
                                label = { Text(skinCaps(stringResource(t.label)), maxLines = 1) },
                                colors = colors,
                            )
                        }
                    }
                }
            },
        ) { padding ->
            // destinations slide along the navigation bar's order (back = the other way);
            // logging in swaps the screen in place
            // not before the account is known: a signed-in user would see the login flash by
            val screen = Screen(tab, login = accountKnown && !loggedIn && (tab == Tab.SEARCH || tab == Tab.DOWNLOADS))
            AnimatedContent(
                targetState = screen,
                modifier = Modifier.padding(padding),
                transitionSpec = {
                    if (initialState.tab == targetState.tab) {
                        motion.fadeThrough()
                    } else {
                        motion.screens(forward = targetState.tab.ordinal > initialState.tab.ordinal)
                    }
                },
                label = "tabs",
            ) { shown ->
                when {
                    shown.login -> LoginScreen(onLoggedIn = { if (!app.account.isLoggedIn) app.account.refresh() })
                    shown.tab == Tab.HOME -> HomeScreen()
                    shown.tab == Tab.LIBRARY -> LibraryScreen()
                    shown.tab == Tab.SEARCH -> SearchScreen(onQueued = { tab = Tab.DOWNLOADS })
                    shown.tab == Tab.DOWNLOADS -> DownloadsScreen()
                    else -> SettingsScreen(
                        loggedIn = loggedIn,
                        onLogin = { tab = Tab.SEARCH },
                        onLoggedOut = { app.account.signedOut() },
                    )
                }
            }
        }

        AnimatedVisibility(
            visible = nowPlayingShown,
            enter = motion.panelEnter(),
            exit = motion.panelExit(),
        ) {
            CompositionLocalProvider(LocalCoverVisibility provides this) {
                NowPlayingScreen(onClose = { playerOpen = false })
            }
        }

        // a guest's remote for someone else's hub
        AnimatedVisibility(
            visible = guestOpen,
            enter = motion.panelEnter(),
            exit = motion.panelExit(),
        ) {
            CompositionLocalProvider(LocalCoverVisibility provides this) {
                GuestScreen(player = roomPlayer, onClose = { app.share.openGuest(false) })
            }
        }

        // the equalizer and the sound assistant, over everything (player included)
        val eqPanel by app.equalizer.panel.collectAsStateWithLifecycle()
        AnimatedVisibility(
            visible = eqPanel != null,
            enter = motion.panelEnter(),
            exit = motion.panelExit(),
        ) {
            // the last panel stays drawn while it slides away
            var shown by remember { mutableStateOf(eqPanel ?: EqPanel.EQUALIZER) }
            LaunchedEffect(eqPanel) { eqPanel?.let { shown = it } }
            Box(Modifier.fillMaxSize().clickable(enabled = false) {}) {
                AnimatedContent(
                    targetState = shown,
                    transitionSpec = { motion.screens(forward = targetState == EqPanel.ASSISTANT) },
                    label = "eq-panel",
                ) { panel ->
                    when (panel) {
                        EqPanel.EQUALIZER -> EqualizerScreen(
                            onClose = app.equalizer::close,
                            onAssistant = { app.equalizer.open(EqPanel.ASSISTANT) },
                        )
                        EqPanel.ASSISTANT -> AssistantScreen(
                            onClose = app.equalizer::back,
                            onEqualizer = { app.equalizer.open(EqPanel.EQUALIZER) },
                        )
                    }
                }
            }
        }

        // "How does it sound?" and "Sound for this…", from the player or any list
        val howOpen by app.sound.howOpen.collectAsStateWithLifecycle()
        if (howOpen) HowSoundsSheet(onDismiss = { app.sound.openHow(false) })
        val soundFor by app.sound.soundFor.collectAsStateWithLifecycle()
        soundFor?.let { req -> key(req) { SoundForDialog(req.scope, onDismiss = { app.sound.soundFor(null) }) } }
        // what a reaction changed, with Undo, over everything (the player included)
        val soundSnackbar = remember { SnackbarHostState() }
        SoundNotices(soundSnackbar)
        SnackbarHost(soundSnackbar, Modifier.align(Alignment.BottomCenter).navigationBarsPadding().padding(bottom = 72.dp))

        // the hub's discreet notices, over every screen
        ShareNotices(Modifier.align(Alignment.TopCenter))

        // sessions nearby: searched while the app is open, offered here outside Home
        NearbySessions(offer = tab != Tab.HOME || playerOpen || guestOpen, Modifier.align(Alignment.TopCenter))
    }
    }

    // one shared scope, so the mini player's cover grows into the player's. Always there (the
    // motion setting only decides whether the cover uses it): adding or removing it would
    // rebuild the whole UI while that very setting is being changed
    SharedTransitionLayout(Modifier.fillMaxSize()) {
        CompositionLocalProvider(LocalSharedCover provides this) { body() }
    }

    joinRequest?.let { JoinDialog(it) }

    // signed in before the personal-use notice existed (or it changed): shown once, accept or sign out
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    if (loggedIn && !settings.termsAccepted) {
        TermsDialog(
            onAccept = { app.settings.acceptTerms() },
            onSignOut = {
                scope.launch {
                    app.downloads.cancelAll()
                    app.engine.logout()
                    app.account.signedOut()
                }
            },
        )
    }
}
