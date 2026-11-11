package com.perseusoft.homefi.ui.playlists

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.core.LibraryTrack
import com.perseusoft.homefi.core.SearchHit
import com.perseusoft.homefi.core.SongSeed
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.library.LibraryRepository
import com.perseusoft.homefi.player.NowPlaying
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/** What asks the library to open an AI chat. */
sealed interface ChatRequest {
    /** "Playlist like this": a new chat that opens with this song. */
    data class LikeSong(val seed: SongSeed) : ChatRequest

    /** A chat that is already open (from the chats menu or a notice). */
    data class Resume(val id: String) : ChatRequest
}

/**
 * A request picked in a menu or a notice waits here until the library opens the chat. Held in a
 * flow so it survives the player closing and the tab changing.
 */
class ChatRequests {
    private val _request = MutableStateFlow<ChatRequest?>(null)
    val request: StateFlow<ChatRequest?> = _request.asStateFlow()

    fun open(request: ChatRequest) {
        _request.value = request
    }

    /** The library took the request: it won't open the chat again. */
    fun consume() {
        _request.value = null
    }
}

/**
 * What a song menu does for "Playlist like this": null (the item stays hidden) when the setting is
 * off, else a callback that sends the song [seed] builds to the AI chat.
 */
@Composable
fun songSeedAction(seed: () -> SongSeed): (() -> Unit)? {
    val app = LocalContext.current.homefi
    val enabled by app.settings.playlists.collectAsStateWithLifecycle()
    return if (enabled.chatFromSong) ({ app.chatRequests.open(ChatRequest.LikeSong(seed())) }) else null
}

private fun String.orNullIfBlank(): String? = takeIf { it.isNotBlank() }

fun LibraryTrack.toSeed() = SongSeed(
    title = title,
    artist = artist,
    album = album.orNullIfBlank(),
    year = year?.toUShort(),
    genre = genre.orNullIfBlank(),
)

/** A Tidal result knows its title, artist and album only. */
fun SearchHit.toSeed() = SongSeed(title = title, artist = artist, album = album.orNullIfBlank(), year = null, genre = null)

/** The playing song; its year comes from the library file when it is one. */
fun NowPlaying.toSeed(library: LibraryRepository) = SongSeed(
    title = title,
    artist = artist,
    album = album.orNullIfBlank(),
    year = library.find(mediaId)?.year?.toUShort(),
    genre = genre?.orNullIfBlank(),
)
