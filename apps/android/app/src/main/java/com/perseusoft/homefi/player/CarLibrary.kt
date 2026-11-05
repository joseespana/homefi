package com.perseusoft.homefi.player

import android.net.Uri
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import com.perseusoft.homefi.HomefiApplication
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.LibraryTrack
import com.perseusoft.homefi.data.AppSettings
import com.perseusoft.homefi.library.albumKey
import com.perseusoft.homefi.library.albums
import com.perseusoft.homefi.library.artists
import com.perseusoft.homefi.library.filterQuery
import com.perseusoft.homefi.ui.playlists.mediaItems
import com.perseusoft.homefi.ui.playlists.source
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.withTimeoutOrNull

/**
 * What Android Auto (and any other media browser) can browse and search: recently played,
 * the playlists HomeFi made, albums, artists and songs of the library, each section a setting.
 *
 * Ids: sections are `root`, `recent`, `playlists`, `albums`, `artists`, `songs`; a list is
 * `album:<key>`, `artist:<name>` or `playlist:<id>`; a song inside a list is
 * `in|<list id>|<index>`, so picking it plays the whole list from there ([expand]). Recent
 * songs and search results keep their own media id (a file path or `tidal:<id>`).
 */
class CarLibrary(private val app: HomefiApplication) {

    fun root(): MediaItem = folder(ROOT, app.getString(R.string.app_name))

    /** Children of [parentId], or null when the id is unknown. Waits briefly for a first library scan. */
    suspend fun children(parentId: String): List<MediaItem>? {
        val settings = app.settings.current
        if (!settings.carEnabled) return emptyList()
        return when {
            parentId == ROOT -> sections(settings)
            parentId == RECENT -> app.player.recents.items.value.take(MAX_ITEMS).map { recentItem(it) }
            parentId == PLAYLISTS -> app.playlists.list.value.map { folder(PLAYLIST + it.id, it.name, subtitle = it.description) }
            parentId == ALBUMS -> tracks().albums().take(MAX_ITEMS).map { folder(ALBUM + it.key, it.title, subtitle = it.artist) }
            parentId == ARTISTS -> tracks().artists().take(MAX_ITEMS).map { folder(ARTIST + it.name, it.name) }
            parentId == SONGS -> tracks().take(MAX_ITEMS).mapIndexed { i, t -> inList(SONGS, i, t.toMediaItem()) }
            else -> listItems(parentId)?.mapIndexed { i, item -> inList(parentId, i, item) }
        }
    }

    /** Library songs matching [query] (title, artist, album), for "play … on HomeFi". */
    suspend fun search(query: String): List<MediaItem> =
        if (!app.settings.current.carEnabled) emptyList()
        else tracks().filterQuery(query).take(MAX_ITEMS).map { it.toMediaItem() }

    /**
     * What to play for the items a car asked for: a song picked inside a list becomes the whole
     * list from that song; anything else plays as it is. Null when nothing is recognised.
     */
    suspend fun expand(items: List<MediaItem>): Pair<List<MediaItem>, Int>? {
        val single = items.singleOrNull()?.mediaId
        if (single != null && single.startsWith(IN)) {
            val parts = single.split('|', limit = 3)
            val parentId = parts.getOrNull(1) ?: return null
            val index = parts.getOrNull(2)?.toIntOrNull() ?: 0
            val list = if (parentId == SONGS) tracks().take(MAX_ITEMS).map { it.toMediaItem() } else listItems(parentId)
            return list?.takeIf { it.isNotEmpty() }?.let { it to index.coerceIn(0, it.lastIndex) }
        }
        val playable = items.mapNotNull { playable(it.mediaId) ?: it.takeIf { i -> i.localConfiguration != null } }
        return playable.takeIf { it.isNotEmpty() }?.let { it to 0 }
    }

    /** A recent song or search result by its own media id: a library file or a Tidal track. */
    private fun playable(mediaId: String): MediaItem? = when {
        isFilePath(mediaId) -> app.library.find(mediaId)?.toMediaItem() ?: fileMediaItem(mediaId)
        tidalId(mediaId) != null -> app.player.recents.items.value.firstOrNull { it.mediaId == mediaId }?.let { recentItem(it) }
        else -> null
    }

    private suspend fun listItems(parentId: String): List<MediaItem>? = when {
        parentId.startsWith(ALBUM) -> {
            val key = parentId.removePrefix(ALBUM)
            val album = tracks().filter { albumKey(it) == key }.sortedWith(compareBy({ it.discNumber }, { it.trackNumber }))
            album.takeIf { it.isNotEmpty() }?.map { it.toMediaItem().withSource(Sources.album(key, it.album)) }
        }
        parentId.startsWith(ARTIST) -> {
            val name = parentId.removePrefix(ARTIST)
            tracks().filter { it.artist == name || it.albumArtist == name }.take(MAX_ITEMS)
                .map { it.toMediaItem().withSource(Sources.artist("", name)) }
        }
        parentId.startsWith(PLAYLIST) -> {
            val playlist = app.playlists.list.value.firstOrNull { it.id == parentId.removePrefix(PLAYLIST) } ?: return null
            playlist.mediaItems(app.library::find).map { it.withSource(playlist.source()) }
        }
        else -> null
    }

    private fun sections(settings: AppSettings): List<MediaItem> = buildList {
        if (settings.carRecent) add(folder(RECENT, app.getString(R.string.car_recent), playableChildren = true))
        if (settings.carPlaylists) add(folder(PLAYLISTS, app.getString(R.string.car_playlists)))
        if (settings.carAlbums) add(folder(ALBUMS, app.getString(R.string.car_albums)))
        if (settings.carArtists) add(folder(ARTISTS, app.getString(R.string.car_artists)))
        if (settings.carSongs) add(folder(SONGS, app.getString(R.string.car_songs), playableChildren = true))
    }

    /** The library's songs; started by a car before the app was ever opened, it scans first. */
    private suspend fun tracks(): List<LibraryTrack> {
        val state = app.library.state.value
        if (state.scanned) return state.tracks
        if (!state.scanning) app.library.refresh()
        return withTimeoutOrNull(SCAN_WAIT_MS) { app.library.state.first { it.scanned } }?.tracks ?: app.library.state.value.tracks
    }

    private fun recentItem(recent: Recent): MediaItem =
        app.library.find(recent.mediaId)?.toMediaItem()
            ?: tidalId(recent.mediaId)?.let { tidalMediaItem(it, recent.title, recent.artist, recent.album, recent.artworkUri) }
            ?: fileMediaItem(recent.mediaId)

    /** A playable [item] shown inside list [parentId] at [index]: picking it plays the list from there. */
    /** Without its address: the session sees a car's pick and expands it (see `onSetMediaItems`). */
    private fun inList(parentId: String, index: Int, item: MediaItem): MediaItem =
        item.buildUpon()
            .setMediaId("$IN$parentId|$index")
            .setUri(null as Uri?)
            .setRequestMetadata(MediaItem.RequestMetadata.EMPTY)
            .build()

    // no artwork on lists: covers are embedded in the files and a car can't read a file path
    // (the song playing shows its cover, which the player reads from the file)
    private fun folder(id: String, title: String, subtitle: String? = null, playableChildren: Boolean = false): MediaItem =
        MediaItem.Builder()
            .setMediaId(id)
            .setMediaMetadata(
                MediaMetadata.Builder()
                    .setTitle(title)
                    .setSubtitle(subtitle?.ifBlank { null })
                    .setIsBrowsable(true)
                    .setIsPlayable(false)
                    .setMediaType(if (playableChildren) MediaMetadata.MEDIA_TYPE_PLAYLIST else MediaMetadata.MEDIA_TYPE_FOLDER_MIXED)
                    .build(),
            )
            .build()

    companion object {
        const val ROOT = "root"
        private const val RECENT = "recent"
        private const val PLAYLISTS = "playlists"
        private const val ALBUMS = "albums"
        private const val ARTISTS = "artists"
        private const val SONGS = "songs"
        private const val ALBUM = "album:"
        private const val ARTIST = "artist:"
        private const val PLAYLIST = "playlist:"
        private const val IN = "in|"
        /** Cars show a few hundred rows at most; longer lists only cost memory. */
        private const val MAX_ITEMS = 500
        private const val SCAN_WAIT_MS = 8_000L
    }
}
