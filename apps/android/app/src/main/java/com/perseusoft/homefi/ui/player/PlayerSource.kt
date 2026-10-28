package com.perseusoft.homefi.ui.player

import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.core.LibraryTrack
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.Haptics
import com.perseusoft.homefi.ui.haptics
import com.perseusoft.homefi.player.PlayerController
import com.perseusoft.homefi.player.PlayerUiState
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * What the player screen can do with what it shows. Everything is allowed on
 * this phone's own player; a guest of someone else's hub only gets what the
 * hub's rules let it do, and the rest is shown greyed out.
 */
@Immutable
data class PlayerCapabilities(
    val canPlayPause: Boolean = true,
    val canNext: Boolean = true,
    val canPrevious: Boolean = true,
    val canSeek: Boolean = true,
    val canShuffleRepeat: Boolean = true,
    /** Jump to a song of the queue. */
    val canSkipTo: Boolean = true,
    /** Play or queue this phone's library (the cover flow's albums). */
    val canPlayLibrary: Boolean = true,
    /** Keep a Tidal song in the library. */
    val canDownload: Boolean = true,
    /** Name of the hub the music comes from (a guest), shown instead of the source. */
    val remoteName: String? = null,
)

/**
 * Feeds the player screen: the state it draws and the buttons' actions. This
 * phone's player by default; a guest gets the hub's song (see GuestPlayer).
 * Main thread only.
 */
@Stable
interface PlayerSource {
    val state: StateFlow<PlayerUiState>
    val capabilities: StateFlow<PlayerCapabilities>

    fun togglePlayPause()
    fun next()
    fun previous()
    fun seekTo(positionMs: Long)
    fun skipTo(index: Int)
    fun toggleShuffle()
    fun cycleRepeat()
    fun play(tracks: List<LibraryTrack>, startIndex: Int = 0, source: com.perseusoft.homefi.core.SourceContext? = null)
    fun queueTracks(tracks: List<LibraryTrack>, next: Boolean)
}

/** This phone's own player: everything goes straight to the [PlayerController]. */
private class ControllerSource(private val player: PlayerController) : PlayerSource {
    override val state get() = player.state
    override val capabilities: StateFlow<PlayerCapabilities> = MutableStateFlow(PlayerCapabilities()).asStateFlow()

    override fun togglePlayPause() = player.togglePlayPause()
    override fun next() = player.next()
    override fun previous() = player.previous()
    override fun seekTo(positionMs: Long) = player.seekTo(positionMs)
    override fun skipTo(index: Int) = player.skipTo(index)
    override fun toggleShuffle() = player.toggleShuffle()
    override fun cycleRepeat() = player.cycleRepeat()
    override fun play(tracks: List<LibraryTrack>, startIndex: Int, source: com.perseusoft.homefi.core.SourceContext?) = player.play(tracks, startIndex, source = source)
    override fun queueTracks(tracks: List<LibraryTrack>, next: Boolean) = player.queueTracks(tracks, next)
}

/**
 * Any source, with its buttons felt: every layout's transport, the mini player and the
 * lyrics go through a source, so they all buzz the same way.
 */
private class FeltSource(val inner: PlayerSource, private val haptics: Haptics) : PlayerSource by inner {
    override fun togglePlayPause() {
        haptics.press()
        inner.togglePlayPause()
    }

    override fun next() {
        haptics.press()
        inner.next()
    }

    override fun previous() {
        haptics.press()
        inner.previous()
    }

    override fun seekTo(positionMs: Long) {
        haptics.tick()
        inner.seekTo(positionMs)
    }

    override fun skipTo(index: Int) {
        haptics.tick()
        inner.skipTo(index)
    }

    override fun toggleShuffle() {
        haptics.toggle(!inner.state.value.shuffle)
        inner.toggleShuffle()
    }

    override fun cycleRepeat() {
        haptics.tick()
        inner.cycleRepeat()
    }
}

/** [this] with its buttons felt (once, however often it's wrapped). */
@Composable
fun PlayerSource.felt(): PlayerSource {
    val haptics = haptics()
    return remember(this, haptics) { if (this is FeltSource) this else FeltSource(this, haptics) }
}

/** The source of the player being drawn; null means this phone's own player. */
val LocalPlayerSource = staticCompositionLocalOf<PlayerSource?> { null }

/** The player the surrounding screen shows (this phone's unless a [LocalPlayerSource] says otherwise). */
@Composable
fun playerSource(): PlayerSource {
    val player = LocalContext.current.homefi.player
    val own = remember(player) { ControllerSource(player) }
    return (LocalPlayerSource.current ?: own).felt()
}

/** What [playerSource] allows right now. */
@Composable
fun playerCapabilities(): PlayerCapabilities {
    val caps by playerSource().capabilities.collectAsStateWithLifecycle()
    return caps
}

/** Alpha of the controls a guest can't use. */
internal const val DISABLED_ALPHA = 0.38f
