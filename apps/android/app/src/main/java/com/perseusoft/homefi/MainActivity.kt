package com.perseusoft.homefi

import android.content.Intent
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
import com.perseusoft.homefi.core.parseLink
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

class MainActivity : ComponentActivity() {
    private val destinations = Channel<Destination>(Channel.BUFFERED)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
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
                    ProvideMotion(settings.motion, layout) { HomefiRoot(destinations.receiveAsFlow()) }
                }
            }
        }
        if (savedInstanceState == null) handleIntent(intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleIntent(intent)
    }

    private fun handleIntent(intent: Intent?) {
        intent ?: return
        if (intent.getBooleanExtra(EXTRA_OPEN_PLAYER, false)) {
            destinations.trySend(Destination.PLAYER)
            return
        }
        val data = intent.data
        when {
            intent.action == Intent.ACTION_VIEW && data?.scheme == "homefi" -> joinLink(intent.dataString)
            intent.action == Intent.ACTION_VIEW && data != null && data.scheme in LOCAL_SCHEMES -> playFile(data, intent.type)
            intent.action == Intent.ACTION_SEND -> queueLink(intent.getStringExtra(Intent.EXTRA_TEXT))
            intent.action == Intent.ACTION_VIEW -> queueLink(intent.dataString)
        }
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
        if (link == null || homefi.engine.account() == null) {
            Toast.makeText(this, R.string.share_invalid, Toast.LENGTH_SHORT).show()
            return
        }
        val path = "${link.kind.name.lowercase()}/${link.id}"
        homefi.downloads.enqueue(path, listOf(path))
        destinations.trySend(Destination.DOWNLOADS)
        Toast.makeText(this, getString(R.string.share_queued, path), Toast.LENGTH_SHORT).show()
    }

    companion object {
        const val EXTRA_OPEN_PLAYER = "com.perseusoft.homefi.OPEN_PLAYER"
        private val LOCAL_SCHEMES = setOf("content", "file")
        private val LINK = Regex("""https?://\S*tidal\.com/\S+""")
    }
}
