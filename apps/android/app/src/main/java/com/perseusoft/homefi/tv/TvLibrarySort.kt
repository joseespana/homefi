package com.perseusoft.homefi.tv

import androidx.annotation.StringRes
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.LibraryTrack
import com.perseusoft.homefi.library.Album
import com.perseusoft.homefi.library.Artist

/** The views of the TV library, in the order of the pills. */
enum class TvLibraryTab(@StringRes val label: Int) {
    SONGS(R.string.library_songs),
    ALBUMS(R.string.library_albums),
    ARTISTS(R.string.library_artists),
    PLAYLISTS(R.string.library_playlists),
}

/** How a library view is ordered; the sort pill cycles through [TvLibraryTab.sorts]. */
enum class TvSort(@StringRes val label: Int) {
    TITLE(R.string.tv_sort_title),
    ARTIST(R.string.tv_sort_artist),
    ALBUM(R.string.tv_sort_album),
    YEAR(R.string.tv_sort_year),
    COUNT(R.string.tv_sort_count),
}

/** The orders a view offers, the default first. Playlists keep the order they were found in. */
val TvLibraryTab.sorts: List<TvSort>
    get() = when (this) {
        TvLibraryTab.SONGS -> listOf(TvSort.TITLE, TvSort.ARTIST, TvSort.ALBUM, TvSort.YEAR)
        TvLibraryTab.ALBUMS -> listOf(TvSort.ARTIST, TvSort.TITLE, TvSort.YEAR)
        TvLibraryTab.ARTISTS -> listOf(TvSort.TITLE, TvSort.COUNT)
        TvLibraryTab.PLAYLISTS -> emptyList()
    }

/** [sort] when this view offers it, else its default. */
fun TvLibraryTab.sortOrDefault(sort: TvSort): TvSort = if (sort in sorts) sort else sorts.firstOrNull() ?: sort

/** The next order after [sort] in this view (wraps around). */
fun TvLibraryTab.nextSort(sort: TvSort): TvSort {
    val all = sorts
    if (all.isEmpty()) return sort
    return all[(all.indexOf(sort) + 1).mod(all.size)]
}

private fun String.key() = lowercase()

fun List<LibraryTrack>.ordered(sort: TvSort): List<LibraryTrack> = when (sort) {
    TvSort.TITLE -> sortedBy { it.title.key() }
    TvSort.ARTIST -> sortedWith(compareBy({ it.artist.key() }, { it.album.key() }, { it.discNumber }, { it.trackNumber }))
    TvSort.ALBUM -> sortedWith(compareBy({ it.album.key() }, { it.discNumber }, { it.trackNumber }))
    // newest first, the untagged last
    TvSort.YEAR -> sortedWith(compareByDescending<LibraryTrack> { it.year ?: 0u }.thenBy { it.title.key() })
    TvSort.COUNT -> this
}

@JvmName("orderedAlbums")
fun List<Album>.ordered(sort: TvSort): List<Album> = when (sort) {
    TvSort.TITLE -> sortedBy { it.title.key() }
    TvSort.YEAR -> sortedWith(compareByDescending<Album> { it.year ?: 0u }.thenBy { it.title.key() })
    else -> sortedWith(compareBy({ it.artist.key() }, { it.title.key() }))
}

@JvmName("orderedArtists")
fun List<Artist>.ordered(sort: TvSort): List<Artist> = when (sort) {
    TvSort.COUNT -> sortedWith(compareByDescending<Artist> { it.tracks.size }.thenBy { it.name.key() })
    else -> sortedBy { it.name.key() }
}
