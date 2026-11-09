package com.perseusoft.homefi

import android.content.Intent
import android.content.res.Configuration
import android.graphics.Color
import android.net.Uri
import android.os.Bundle
import android.provider.OpenableColumns
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.lifecycleScope
import com.perseusoft.homefi.core.parseLink
import com.perseusoft.homefi.pair.TvPairLink
import com.perseusoft.homefi.data.AppEntry
import com.perseusoft.homefi.data.DeviceForm
import com.perseusoft.homefi.data.FileExchange
import com.perseusoft.homefi.tv.TvActivity
import com.perseusoft.homefi.data.PlayerLayout
import com.perseusoft.homefi.data.ThemeMode
import com.perseusoft.homefi.ui.player.coverModel
import com.perseusoft.homefi.ui.theme.fixedDark
import com.perseusoft.homefi.ui.theme.rememberCoverAccent
import com.perseusoft.homefi.ui.theme.skinOf
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import com.perseusoft.homefi.ui.Destination
import com.perseusoft.homefi.ui.HomefiRoot
import com.perseusoft.homefi.ui.theme.HomefiTheme
import com.perseusoft.homefi.ui.theme.ProvideMotion
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.platform.LocalView
import com.perseusoft.homefi.ui.Haptics
import com.perseusoft.homefi.ui.LocalHaptics
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.launch
import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.repeatOnLifecycle
import com.perseusoft.homefi.player.PictureInPicture
import com.perseusoft.homefi.player.PipSpec
import com.perseusoft.homefi.ui.player.PipPlayer
import com.perseusoft.homefi.ui.player.PipSourceRect
import kotlinx.coroutines.flow.combine

class MainActivity : ComponentActivity() {
    private val destinations = Channel<Destination>(Channel.BUFFERED)
    private val pip by lazy { PictureInPicture(this, homefi.player) }
    private var inPip by mutableStateOf(false)
    private var pipSpec: PipSpec? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // on a TV the D-pad app is the front door: hand everything over (a link, a file, the player)
        if (homefi.deviceForm.mustHandOver(activityIsTv = false)) {
            startActivity(Intent(intent).setClass(this, TvActivity::class.java))
            finish()
            return
        }
        setContent {
            val settings by homefi.settings.settings.collectAsStateWithLifecycle()
            val themeDark = when (settings.theme) {
                ThemeMode.SYSTEM -> isSystemInDarkTheme()
                ThemeMode.LIGHT -> false
                ThemeMode.DARK -> true
            }
            // the player layout skins the whole app, unless only the player should change
            val layout = if (settings.layoutEverywhere) settings.playerLayout else PlayerLayout.CLASSIC
            val dark = layout.fixedDark() ?: themeDark
            // immersive: the album playing behind everything, its colour as the accent
            val cover by remember { homefi.player.state.map { it.current?.coverModel() }.distinctUntilChanged() }
                .collectAsStateWithLifecycle(null)
            val art = if (layout == PlayerLayout.IMMERSIVE) cover else null
            val accent = rememberCoverAccent(art)
            val skin = remember(layout, dark, accent, art) { skinOf(layout, dark, accent, art) }
            // status / navigation bar icons follow the skin's bars, not only the system theme
            val statusDark = skin.topBar?.dark ?: dark
            val navDark = skin.navBar?.dark ?: dark
            LaunchedEffect(statusDark, navDark) {
                enableEdgeToEdge(
                    statusBarStyle = SystemBarStyle.auto(Color.TRANSPARENT, Color.TRANSPARENT) { statusDark },
                    navigationBarStyle = SystemBarStyle.auto(Color.TRANSPARENT, Color.TRANSPARENT) { navDark },
                )
            }
            HomefiTheme(skin) {
                // transitions follow the motion setting and the skin's era
                // the buttons' buzz reads the setting when it fires, so it never rebuilds the UI
                val hapticsOn by rememberUpdatedState(settings.haptics)
                val view = LocalView.current
                val haptics = remember(view) { Haptics(view) { hapticsOn } }
                CompositionLocalProvider(LocalHaptics provides haptics) {
                    ProvideMotion(settings.motion, layout) {
                        // the app stays composed underneath, so its place is where it was when the window grows back
                        Box {
                            HomefiRoot(destinations.receiveAsFlow())
                            if (inPip) PipPlayer()
                        }
                    }
                }
            }
        }
        if (savedInstanceState == null) handleIntent(intent)
        keepPipParamsCurrent()
    }

    override fun onStart() {
        super.onStart()
        pip.start()
    }

    override fun onStop() {
        pip.stop()
        super.onStop()
    }

    /** Android 8-11 float the player from here; 12+ do it by themselves (auto-enter in the params). */
    override fun onUserLeaveHint() {
        super.onUserLeaveHint()
        pipSpec?.let(pip::enterIfArmed)
    }

    override fun onPictureInPictureModeChanged(isInPictureInPictureMode: Boolean, newConfig: Configuration) {
        super.onPictureInPictureModeChanged(isInPictureInPictureMode, newConfig)
        inPip = isInPictureInPictureMode
    }

    /** Re-hands the system the window's parameters whenever one of its inputs changes. */
    private fun keepPipParamsCurrent() {
        if (!pip.supported) return
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.CREATED) {
                combine(homefi.settings.settings, homefi.player.state, PipSourceRect.bounds) { settings, state, rect ->
                    PipSpec.of(settings, state, rect)
                }
                    .distinctUntilChanged()
                    .collect { spec ->
                        pipSpec = spec
                        pip.update(spec)
                    }
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleIntent(intent)
    }

    private fun handleIntent(intent: Intent?) {
        intent ?: return
        if (intent.getBooleanExtra(AppEntry.EXTRA_OPEN_PLAYER, false)) {
            destinations.trySend(Destination.PLAYER)
            return
        }
        val data = intent.data
        when {
            intent.action == Intent.ACTION_VIEW && data?.scheme == "homefi" && data.host == "tv" -> tvLink(intent.dataString)
            intent.action == Intent.ACTION_VIEW && data?.scheme == "homefi" -> joinLink(intent.dataString)
            intent.action == Intent.ACTION_VIEW && data != null && data.scheme in LOCAL_SCHEMES && isHomefiFile(data, intent.type) ->
                homefi.incoming.offer(data)
            intent.action == Intent.ACTION_VIEW && data != null && data.scheme in LOCAL_SCHEMES -> playFile(data, intent.type)
            intent.action == Intent.ACTION_SEND -> queueLink(intent.getStringExtra(Intent.EXTRA_TEXT))
            intent.action == Intent.ACTION_VIEW -> queueLink(intent.dataString)
        }
    }

    /** A `.homefiride` / `.homefidj` export, or JSON from a provider that hides the file name. */
    private fun isHomefiFile(uri: Uri, type: String?): Boolean {
        val name = uri.lastPathSegment.orEmpty().lowercase()
        return name.endsWith(".$RIDE_EXT") || name.endsWith(".$SETUP_EXT") || type == FileExchange.MIME_JSON
    }

    /** "Open with HomeFi" from a file manager or another app. */
    private fun playFile(uri: Uri, type: String?) {
        val mime = type ?: contentResolver.getType(uri)
        if (mime != null && !mime.startsWith("audio/") && mime != "application/ogg") {
            Toast.makeText(this, R.string.open_unsupported, Toast.LENGTH_SHORT).show()
            return
        }
        val path = uri.path
        if (uri.scheme == "file" && path != null) {
            homefi.player.playFile(path) // tags, cover and exact quality from the Rust core
        } else {
            homefi.player.playUri(uri, displayName(uri))
        }
        destinations.trySend(Destination.PLAYER)
    }

    private fun displayName(uri: Uri): String? =
        if (uri.scheme == "file") {
            uri.lastPathSegment?.substringBeforeLast('.')
        } else {
            runCatching {
                contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
                    if (c.moveToFirst()) c.getString(0)?.substringBeforeLast('.') else null
                }
            }.getOrNull()
        }

    /** `homefi://tv?...` (the TV's QR, opened by the system camera): pairs that TV, sending the ride that runs now. */
    private fun tvLink(link: String?) {
        val pair = TvPairLink.parse(link)
        if (pair == null) {
            Toast.makeText(this, R.string.tv_pair_invalid, Toast.LENGTH_SHORT).show()
            return
        }
        homefi.tvPairings.connect(pair)
    }

    /** `homefi://join?...` from a scanned QR code: ask for the name, then join. */
    private fun joinLink(link: String?) {
        val hub = link?.let(homefi.share::parseLink)
        if (hub == null) {
            Toast.makeText(this, R.string.sharing_link_invalid, Toast.LENGTH_SHORT).show()
            return
        }
        homefi.share.requestJoin(hub)
    }

    /** Queues Tidal links shared to the app or opened with it. */
    private fun queueLink(text: String?) {
        text ?: return
        val link = LINK.find(text)?.value?.let { runCatching { parseLink(it) }.getOrNull() }
        // a link shared to a cold app may arrive before the account was read: wait for it
        lifecycleScope.launch {
            if (link == null || homefi.account.current() == null) {
                Toast.makeText(this@MainActivity, R.string.share_invalid, Toast.LENGTH_SHORT).show()
                return@launch
            }
            val path = "${link.kind.name.lowercase()}/${link.id}"
            homefi.downloads.enqueue(path, listOf(path))
            destinations.trySend(Destination.DOWNLOADS)
            Toast.makeText(this@MainActivity, getString(R.string.share_queued, path), Toast.LENGTH_SHORT).show()
        }
    }

    companion object {
        private const val RIDE_EXT = "homefiride"
        private const val SETUP_EXT = "homefidj"
        private val LOCAL_SCHEMES = setOf("content", "file")
        private val LINK = Regex("""https?://\S*tidal\.com/\S+""")
    }
}
