package com.perseusoft.homefi.ui.playlists

import androidx.media3.common.MediaItem
import com.perseusoft.homefi.core.HomePlaylist
import com.perseusoft.homefi.core.LibraryTrack
import com.perseusoft.homefi.core.SourceContext
import com.perseusoft.homefi.player.PlayerController
import com.perseusoft.homefi.player.Sources
import com.perseusoft.homefi.player.tidalMediaItem
import com.perseusoft.homefi.player.toMediaItem
import com.perseusoft.homefi.player.withSource

/** What a playlist plays: its entries in order, each as a library file or a Tidal stream. */
class PlaylistQueue(
    val items: List<MediaItem>,
    /** For each entry of the playlist, its index in [items], or -1 when it can't play. */
    private val itemIndexOfEntry: IntArray,
) {
    /** The queue position of entry [entryIndex], or null when that entry has nothing to play. */
    fun startOf(entryIndex: Int): Int? = itemIndexOfEntry.getOrNull(entryIndex)?.takeIf { it >= 0 }

    /** The playlist entry that queue item [itemIndex] plays (the reverse of [startOf]), or null. */
    fun entryOfItem(itemIndex: Int): Int? = itemIndexOfEntry.indexOf(itemIndex).takeIf { it >= 0 }
}

/**
 * Library copy first (when the file is still there), else the Tidal track, else skipped.
 * [find] looks a file up by path.
 */
fun HomePlaylist.queue(find: (String) -> LibraryTrack?): PlaylistQueue {
    val items = ArrayList<MediaItem>(entries.size)
    val map = IntArray(entries.size) { -1 }
    entries.forEachIndexed { index, entry ->
        val local = entry.localPath?.let(find)
        val item = when {
            local != null -> local.toMediaItem()
            entry.tidalId != null -> tidalMediaItem(entry.tidalId!!, entry.title, entry.artist, entry.album, entry.coverUrl)
            else -> null
        }
        if (item != null) {
            map[index] = items.size
            items += item
        }
    }
    return PlaylistQueue(items, map)
}

fun HomePlaylist.mediaItems(find: (String) -> LibraryTrack?): List<MediaItem> = queue(find).items

fun HomePlaylist.source(): SourceContext = Sources.playlist(id, name)

/** Plays [playlist] from entry [entryIndex] (the first that can play when null), tagged so the user's Sounds apply. */
fun PlayerController.playPlaylist(
    playlist: HomePlaylist,
    find: (String) -> LibraryTrack?,
    entryIndex: Int = 0,
    shuffle: Boolean = false,
) {
    val queue = playlist.queue(find)
    if (queue.items.isEmpty()) return
    playItems(queue.items, queue.startOf(entryIndex) ?: 0, shuffle, playlist.source())
}

/** Adds the playlist after the current song ([next]) or at the end of the queue. */
fun PlayerController.enqueuePlaylist(playlist: HomePlaylist, find: (String) -> LibraryTrack?, next: Boolean) {
    val source = playlist.source()
    enqueue(playlist.mediaItems(find).map { it.withSource(source) }, next)
}
