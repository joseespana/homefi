package com.perseusoft.homefi.ui.home

import com.perseusoft.homefi.HomefiApplication
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.LocalPlaylist
import com.perseusoft.homefi.library.EmbeddedCover
import com.perseusoft.homefi.library.LibraryState
import com.perseusoft.homefi.player.Recent
import com.perseusoft.homefi.player.isFilePath

// What Home shows, decided without drawing anything: the phone's HomeScreen and the TV's home read it.

/** Songs on the "recent songs" shelf. */
internal const val RECENT_SONGS_MAX = 20

/** Albums on the "recent albums" shelf. */
internal const val RECENT_ALBUMS_MAX = 12

/** Playlists on the home shelf. */
internal const val HOME_PLAYLISTS_MAX = 20

/** Image of a recent song: the file's own cover for library songs, Tidal's for streamed ones. */
internal fun Recent.cover(): Any? = if (isFilePath(mediaId)) EmbeddedCover(mediaId) else artworkUri

/** The recent songs shelf. */
internal fun recentSongs(recents: List<Recent>): List<Recent> = recents.take(RECENT_SONGS_MAX)

/** One entry per album, in the order they were last heard. */
internal fun recentAlbums(recents: List<Recent>): List<Recent> =
    recents.filter { it.album.isNotBlank() }.distinctBy { "${it.artist}\u0000${it.album}".lowercase() }.take(RECENT_ALBUMS_MAX)

/** The playlists shelf. */
internal fun homePlaylists(playlists: List<LocalPlaylist>): List<LocalPlaylist> = playlists.take(HOME_PLAYLISTS_MAX)

/** The greeting for an hour of the day (0..23). */
internal fun greetingFor(hourOfDay: Int): Int = when (hourOfDay) {
    in 5..11 -> R.string.home_morning
    in 12..18 -> R.string.home_afternoon
    else -> R.string.home_evening
}

/** Plays the album of [r]: the library's copy as a whole album when there is one, else the songs of it heard so far. */
internal fun HomefiApplication.playRecentAlbum(r: Recent, recents: List<Recent>, library: LibraryState) {
    val local = library.tracks.filter { it.album.equals(r.album, true) && it.artist.equals(r.artist, true) }
    if (local.isNotEmpty()) {
        player.play(local.sortedWith(compareBy({ it.discNumber }, { it.trackNumber })))
    } else {
        player.playRecents(recents.filter { it.album == r.album && it.artist == r.artist }, 0)
    }
}

/** Plays a playlist found in the library; false when none of its songs is there any more. */
internal fun HomefiApplication.playPlaylist(p: LocalPlaylist): Boolean {
    val tracks = p.tracks.mapNotNull(library::find)
    if (tracks.isEmpty()) return false
    player.play(tracks)
    return true
}
