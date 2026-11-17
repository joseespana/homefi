package com.perseusoft.homefi.data

import com.perseusoft.homefi.core.PlannedSong
import com.perseusoft.homefi.core.RideSongSource
import com.perseusoft.homefi.library.LibraryRepository
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.update

/** What a ride's "Download for offline" button shows. */
sealed interface RideOffline {
    /** The ride has no song that can play: no button. */
    data object Empty : RideOffline

    /** Every song has a file here: the ride plays with no internet. */
    data object Ready : RideOffline

    /** [count] TIDAL songs still need the internet. */
    data class Missing(val count: Int) : RideOffline

    /** Its download waits for its turn ([started] false) or runs: [done] of [of] songs. */
    data class Downloading(val done: Int, val of: Int, val started: Boolean) : RideOffline
}

/** How ready a ride is to play with no internet: the core's `playlist::OfflineStatus`, over the phone's library. */
private data class OfflineStatus(val songs: Int, val ready: Int, val toDownload: List<String>)

/** A song of a plan as [RideDownloads] looks at it (the same record the saved rides' list carries). */
fun PlannedSong.source(): RideSongSource = RideSongSource(entry.tidalId, entry.isrc, entry.localPath)

/**
 * "Download for offline" for rides: queues the ride's TIDAL songs that have no file here (labelled with the ride's
 * name) and tells each ride's button where that stands. A song counts as here through
 * [LibraryRepository.localCopy] (its own file, or a downloaded file with its ISRC), so once the downloads are in the
 * library the ride plays every song from the phone.
 */
class RideDownloads(private val downloads: DownloadRepository, private val library: LibraryRepository) {
    /** The download job of each ride (by the ride's key: its saved id, else its playlist id). */
    private val jobs = MutableStateFlow<Map<String, Long>>(emptyMap())

    /** The button's state for the ride [key] with these songs, kept up to date with the downloads and the library. */
    fun state(key: String, songs: List<RideSongSource>): Flow<RideOffline> =
        combine(jobs, downloads.state, library.state) { jobs, queue, shelf ->
            val status = status(songs)
            val jobId = jobs[key]
            val current = queue.current
            when {
                status.songs == 0 -> RideOffline.Empty
                status.toDownload.isEmpty() -> RideOffline.Ready
                jobId != null && current?.id == jobId ->
                    RideOffline.Downloading(queue.currentDone.coerceAtMost(current.links.size), current.links.size, started = true)
                jobId != null && queue.pending.any { it.id == jobId } -> RideOffline.Downloading(0, status.toDownload.size, started = false)
                // the files are written and the library is reading them: almost ready
                jobId != null && shelf.scanning -> RideOffline.Downloading(status.toDownload.size, status.toDownload.size, started = true)
                else -> RideOffline.Missing(status.toDownload.size)
            }
        }.distinctUntilChanged()

    /** Queues the songs of ride [key] that need the internet; nothing when they are all here or already queued. */
    fun download(key: String, name: String, songs: List<RideSongSource>) {
        val queue = downloads.state.value
        val queued = jobs.value[key]?.let { id -> queue.current?.id == id || queue.pending.any { it.id == id } } == true
        if (queued) return
        val missing = status(songs).toDownload
        if (missing.isEmpty()) return
        val jobId = downloads.enqueue(name, missing.map { "$TRACK_LINK$it" })
        jobs.update { it + (key to jobId) }
    }

    /** Counts the songs with a file here and lists, once each, the TIDAL songs to download for the rest. */
    private fun status(songs: List<RideSongSource>): OfflineStatus {
        var playable = 0
        var ready = 0
        val toDownload = LinkedHashSet<String>()
        for (song in songs) {
            val tidalId = song.tidalId?.takeIf { it.isNotBlank() }
            when {
                library.localCopy(song.localPath, song.isrc) != null -> { playable++; ready++ }
                tidalId != null -> { playable++; toDownload += tidalId }
            }
        }
        return OfflineStatus(playable, ready, toDownload.toList())
    }

    private companion object {
        /** A TIDAL track as the download queue takes it ("track/<id>"). */
        const val TRACK_LINK = "track/"
    }
}
