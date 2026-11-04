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

/**
 * "Playlist like this": a song picked in a menu waits here until the library opens the AI chat
 * with it. Held in a flow so it survives the player closing and the tab changing.
 */
class SongSeeds {
    private val _request = MutableStateFlow<SongSeed?>(null)
    val request: StateFlow<SongSeed?> = _request.asStateFlow()

    fun open(seed: SongSeed) {
        _request.value = seed
    }

    /** The library took the song: it won't open the chat again. */
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
    return if (enabled.chatFromSong) ({ app.songSeeds.open(seed()) }) else null
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
