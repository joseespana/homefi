package com.perseusoft.homefi.ui.share

import com.perseusoft.homefi.core.Account
import com.perseusoft.homefi.core.LibraryTrack
import com.perseusoft.homefi.core.ShareSkipMode
import com.perseusoft.homefi.player.NowPlaying
import com.perseusoft.homefi.player.PlayerUiState
import com.perseusoft.homefi.player.QueueEntry
import com.perseusoft.homefi.share.GuestUi
import com.perseusoft.homefi.share.ShareManager
import com.perseusoft.homefi.ui.player.PlayerCapabilities
import com.perseusoft.homefi.ui.player.PlayerSource
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.stateIn

/**
 * The hub's song as the player screen sees it, so a guest gets the same
 * layouts as on its own music. Pause and skip go to the hub when its rules
 * allow them (skip may be a vote); seeking, previous, shuffle, repeat and
 * jumping around the queue stay the hub's, and so does what it plays (a guest
 * adds songs from the hub's search, not from its own library yet).
 */
class GuestPlayer(private val share: ShareManager, loggedIn: StateFlow<Account?>, scope: CoroutineScope) : PlayerSource {
    /** The position moves on the hub's clock: look again a few times a second. */
    private val ticks = flow {
        while (true) {
            emit(Unit)
            delay(TICK_MS)
        }
    }

    override val state: StateFlow<PlayerUiState> = combine(share.guest, ticks) { ui, _ -> playerState(ui) }
        .stateIn(scope, SharingStarted.WhileSubscribed(5_000), playerState(share.guest.value))

    override val capabilities: StateFlow<PlayerCapabilities> = combine(share.guest, loggedIn) { ui, account -> capabilities(ui, account != null) }
        .distinctUntilChanged()
        .stateIn(scope, SharingStarted.WhileSubscribed(5_000), capabilities(share.guest.value, loggedIn.value != null))

    private fun playerState(ui: GuestUi): PlayerUiState {
        val s = ui.state ?: return PlayerUiState()
        val now = s.nowPlaying
        // the queue as the layouts list it: the song playing first, then what's next
        val queue = (listOfNotNull(now) + s.queue).map { item ->
            QueueEntry(
                mediaId = item.track.source,
                title = item.track.title,
                artist = item.track.artist,
                artworkUri = share.resolveUrl(item.track.coverUrl),
                queueId = item.queueId.toLong(),
            )
        }
        return PlayerUiState(
            current = now?.let {
                NowPlaying(
                    // `tidal:<id>` keeps the download button; `hub:<path>` is nothing on this phone
                    mediaId = it.track.source,
                    title = it.track.title,
                    artist = it.track.artist,
                    album = it.track.album,
                    artwork = null,
                    artworkUri = share.resolveUrl(it.track.coverUrl),
                    quality = null,
                )
            },
            isPlaying = s.playing,
            positionMs = if (now == null) 0 else share.guestPositionMs(),
            durationMs = now?.track?.durationMs?.toLong() ?: 0,
            queue = queue,
            index = if (now != null) 0 else -1,
        )
    }

    private fun capabilities(ui: GuestUi, loggedIn: Boolean): PlayerCapabilities {
        val permissions = ui.state?.permissions
        return PlayerCapabilities(
            canPlayPause = permissions?.playback == true,
            canNext = ui.state?.nowPlaying != null && permissions?.skip.let { it == ShareSkipMode.EVERYONE || it == ShareSkipMode.VOTE },
            canPrevious = false,
            canSeek = false,
            canShuffleRepeat = false,
            canSkipTo = false,
            canPlayLibrary = false,
            // keeping a Tidal song uses this phone's own account
            canDownload = loggedIn,
            remoteName = ui.sessionName,
        )
    }

    override fun togglePlayPause() {
        if (capabilities.value.canPlayPause) share.toggle()
    }

    /** Skip, or a vote for it when the hub decides by votes. */
    override fun next() {
        when (share.guest.value.state?.permissions?.skip) {
            ShareSkipMode.EVERYONE -> share.skip()
            ShareSkipMode.VOTE -> share.voteSkip()
            else -> Unit
        }
    }

    override fun previous() = Unit
    override fun seekTo(positionMs: Long) = Unit
    override fun skipTo(index: Int) = Unit
    override fun toggleShuffle() = Unit
    override fun cycleRepeat() = Unit
    override fun play(tracks: List<LibraryTrack>, startIndex: Int, source: com.perseusoft.homefi.core.SourceContext?) = Unit
    override fun queueTracks(tracks: List<LibraryTrack>, next: Boolean) = Unit

    private companion object {
        const val TICK_MS = 250L
    }
}
