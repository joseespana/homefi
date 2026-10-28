package com.perseusoft.homefi.library

import android.content.Context
import com.perseusoft.homefi.core.HomefiEngine
import com.perseusoft.homefi.core.LibraryTrack
import com.perseusoft.homefi.core.LocalPlaylist
import com.perseusoft.homefi.core.findPlaylists
import com.perseusoft.homefi.data.DownloadRepository
import com.perseusoft.homefi.data.Storage
import com.perseusoft.homefi.homefi
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.filter
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import java.io.File

data class LibraryState(
    val tracks: List<LibraryTrack> = emptyList(),
    val scanning: Boolean = false,
    val scanned: Boolean = false,
    /** `false` when only the app's own folder can be read. */
    val hasAccess: Boolean = false,
    /** `.m3u` playlists found in the library folders (tiddl makes these). */
    val playlists: List<LocalPlaylist> = emptyList(),
)

data class Album(val key: String, val title: String, val artist: String, val year: UInt?, val tracks: List<LibraryTrack>) {
    val cover: EmbeddedCover get() = EmbeddedCover(tracks.first().path)
}

data class Artist(val name: String, val albums: Int, val tracks: List<LibraryTrack>)

/** Local music found by the Rust scanner (HomeFi downloads + the phone's Music / Download folders). */
class LibraryRepository(
    private val context: Context,
    private val engine: HomefiEngine,
    downloads: DownloadRepository,
    private val scope: CoroutineScope,
) {
    private val _state = MutableStateFlow(LibraryState())
    val state: StateFlow<LibraryState> = _state.asStateFlow()
    private val mutex = Mutex()

    @Volatile
    private var byPath: Map<String, LibraryTrack> = emptyMap()

    init {
        refresh()
        // pick up new downloads once a download run finishes
        scope.launch {
            downloads.state.map { it.isBusy }.distinctUntilChanged().drop(1).filter { !it }.collect { refresh() }
        }
    }

    fun find(path: String): LibraryTrack? = byPath[path]

    fun refresh() {
        scope.launch {
            mutex.withLock {
                _state.update { it.copy(scanning = true, hasAccess = Storage.hasAudioReadAccess(context)) }
                val roots = Storage.libraryRoots(context, context.homefi.settings.current.downloadDir).map(File::getAbsolutePath)
                val tracks = engine.scanLibrary(roots)
                val playlists = withContext(Dispatchers.IO) { runCatching { findPlaylists(roots) }.getOrDefault(emptyList()) }
                byPath = tracks.associateBy { it.path }
                _state.update { it.copy(tracks = tracks, playlists = playlists, scanning = false, scanned = true) }
            }
        }
    }
}

fun albumKey(track: LibraryTrack): String {
    val album = track.album.ifBlank { File(track.path).parentFile?.name.orEmpty() }
    return "${track.albumArtist.lowercase()}\u0000${album.lowercase()}"
}

fun List<LibraryTrack>.albums(): List<Album> =
    groupBy(::albumKey).map { (key, tracks) ->
        val first = tracks.first()
        Album(
            key = key,
            title = first.album.ifBlank { File(first.path).parentFile?.name.orEmpty() },
            artist = first.albumArtist,
            year = tracks.firstNotNullOfOrNull { it.year },
            tracks = tracks.sortedWith(compareBy({ it.discNumber }, { it.trackNumber }, { it.title.lowercase() })),
        )
    }.sortedWith(compareBy({ it.artist.lowercase() }, { it.title.lowercase() }))

fun List<LibraryTrack>.artists(): List<Artist> =
    groupBy { it.albumArtist.ifBlank { it.artist }.trim() }.map { (name, tracks) ->
        Artist(name, tracks.map(::albumKey).distinct().size, tracks)
    }.sortedBy { it.name.lowercase() }

/** Case-insensitive match on title, artist and album. */
fun List<LibraryTrack>.filterQuery(query: String): List<LibraryTrack> {
    val q = query.trim()
    if (q.isEmpty()) return this
    return filter {
        it.title.contains(q, true) || it.artist.contains(q, true) || it.album.contains(q, true) ||
            it.albumArtist.contains(q, true)
    }
}
