package com.perseusoft.homefi.ui.tidal

import android.app.Application
import android.widget.Toast
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.perseusoft.homefi.core.AlbumPage
import com.perseusoft.homefi.core.ArtistPage
import com.perseusoft.homefi.core.HitPage
import com.perseusoft.homefi.core.ItemKind
import com.perseusoft.homefi.core.PlaylistPage
import com.perseusoft.homefi.core.SearchHit
import com.perseusoft.homefi.data.userMessage
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.Busy
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.Job
import kotlinx.coroutines.async
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** A page of the Tidal browser, pushed on top of the search screen. */
sealed interface TidalRoute {
    data class Artist(val id: String) : TidalRoute
    data class Album(val id: String) : TidalRoute
    data class Playlist(val uuid: String) : TidalRoute
}

/** Lists shown under the search box before typing. */
enum class Collection { PLAYLISTS, TRACKS, ALBUMS, ARTISTS }

sealed interface Load<out T> {
    data object Loading : Load<Nothing>
    data class Ready<T>(val value: T) : Load<T>
    data class Failed(val message: String) : Load<Nothing>
}

/**
 * A Tidal list that may still have pages to load ("Load lists as you scroll").
 * [next] is where the next page starts; `null` when [items] is the whole list.
 */
data class PagedHits(
    val items: List<SearchHit>,
    /** As Tidal counts them, from the first page. */
    val total: Int,
    val next: UInt? = null,
    val loadingMore: Boolean = false,
    /** The last page request failed; the footer offers to retry. */
    val moreError: String? = null,
) {
    val complete: Boolean get() = next == null

    /** What to show as the list's size. */
    val count: Int get() = if (complete) items.size else maxOf(total, items.size)

    fun append(page: HitPage) = copy(
        items = items + page.items,
        total = page.total.toInt(),
        next = page.nextOffset,
        loadingMore = false,
        moreError = null,
    )

    companion object {
        fun of(page: HitPage) = PagedHits(page.items, page.total.toInt(), page.nextOffset)
        fun whole(items: List<SearchHit>) = PagedHits(items, items.size)
    }
}

/** An album or playlist page: its header and its (maybe partly loaded) tracks. */
data class TrackListPage<T>(val info: T, val tracks: PagedHits)

/** Which paged list an action is about. */
sealed interface ListKey {
    data class Page(val route: TidalRoute) : ListKey
    data class Of(val collection: Collection) : ListKey
}

/**
 * Tidal browsing: the page stack, loaded pages, the user's collection and
 * which tracks were queued for download.
 */
class TidalViewModel(app: Application) : AndroidViewModel(app) {
    private val engine = app.homefi.engine
    private val downloads = app.homefi.downloads
    private val settings = app.homefi.settings
    private val player = app.homefi.player

    private val _stack = MutableStateFlow<List<TidalRoute>>(emptyList())
    val stack: StateFlow<List<TidalRoute>> = _stack.asStateFlow()

    private val _pages = MutableStateFlow<Map<TidalRoute, Load<Any>>>(emptyMap())
    val pages: StateFlow<Map<TidalRoute, Load<Any>>> = _pages.asStateFlow()

    private val _collection = MutableStateFlow<Map<Collection, Load<PagedHits>>>(emptyMap())
    val collection: StateFlow<Map<Collection, Load<PagedHits>>> = _collection.asStateFlow()

    /** `kind/id` of everything sent to the download queue in this session. */
    private val _queued = MutableStateFlow<Set<String>>(emptySet())
    val queued: StateFlow<Set<String>> = _queued.asStateFlow()

    /** One page request per list at a time (main thread only). */
    private val moreJobs = mutableMapOf<ListKey, Job>()

    /** The rest of a list being fetched for Play all / Shuffle / Queue (main thread only). */
    private val wholeJobs = mutableMapOf<ListKey, Deferred<List<SearchHit>>>()

    fun open(hit: SearchHit) {
        when (hit.kind) {
            ItemKind.ARTIST -> push(TidalRoute.Artist(hit.id))
            ItemKind.ALBUM -> push(TidalRoute.Album(hit.id))
            ItemKind.PLAYLIST -> push(TidalRoute.Playlist(hit.id))
            else -> Unit
        }
    }

    fun push(route: TidalRoute) {
        _stack.update { it + route }
        if (_pages.value[route] !is Load.Ready) load(route)
    }

    fun pop() = _stack.update { it.dropLast(1) }

    fun load(route: TidalRoute) {
        forget(ListKey.Page(route))
        _pages.update { it + (route to Load.Loading) }
        val lazy = settings.current.lazyLists
        viewModelScope.launch {
            val result = runCatching {
                Busy.track {
                    when (route) {
                        is TidalRoute.Artist -> engine.artistPage(route.id)
                        is TidalRoute.Album ->
                            (if (lazy) engine.albumPageFirst(route.id) else engine.albumPage(route.id))
                                .let { TrackListPage(it, PagedHits(it.tracks, it.numberOfTracks.toInt(), it.tracksNextOffset)) }
                        is TidalRoute.Playlist ->
                            (if (lazy) engine.playlistPageFirst(route.uuid) else engine.playlistPage(route.uuid))
                                .let { TrackListPage(it, PagedHits(it.tracks, it.numberOfTracks.toInt(), it.tracksNextOffset)) }
                    }
                }
            }
            _pages.update { pages ->
                pages + (route to result.fold({ Load.Ready(it as Any) }, { Load.Failed(it.userMessage()) }))
            }
        }
    }

    fun loadCollection(which: Collection, force: Boolean = false) {
        if (!force && _collection.value[which] is Load.Ready) return
        forget(ListKey.Of(which))
        _collection.update { it + (which to Load.Loading) }
        val lazy = settings.current.lazyLists
        viewModelScope.launch {
            val result = runCatching {
                Busy.track { if (lazy) PagedHits.of(fetchPage(ListKey.Of(which), 0u)) else PagedHits.whole(fetchWhole(ListKey.Of(which))) }
            }
            _collection.update { it + (which to result.fold({ Load.Ready(it) }, { Load.Failed(it.userMessage()) })) }
        }
    }

    /**
     * Loads the next page of [key], unless one is loading already or the list is complete.
     * Also the retry of a failed page.
     */
    fun loadMore(key: ListKey) {
        if (moreJobs[key]?.isActive == true || wholeJobs[key]?.isActive == true) return
        val start = list(key)?.next ?: return
        updateList(key) { it.copy(loadingMore = true, moreError = null) }
        moreJobs[key] = viewModelScope.launch {
            var offset: UInt? = start
            try {
                while (offset != null) {
                    val page = fetchPage(key, offset)
                    updateList(key) { it.append(page) }
                    // a page of videos only adds no rows: go on so the list doesn't stall
                    offset = page.nextOffset.takeIf { page.items.isEmpty() }
                }
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                updateList(key) { it.copy(loadingMore = false, moreError = e.userMessage()) }
            }
        }
    }

    /**
     * Every track of [key], fetching what isn't loaded yet (and keeping it, so the list is
     * complete afterwards). `null` when that failed; the user sees why.
     */
    suspend fun wholeList(key: ListKey): List<SearchHit>? {
        val current = list(key) ?: return null
        if (current.complete) return current.items
        val job = wholeJobs[key]?.takeIf { it.isActive } ?: viewModelScope.async {
            moreJobs.remove(key)?.cancel()
            val all = Busy.track { fetchWhole(key) }
            updateList(key) { it.copy(items = all, next = null, loadingMore = false, moreError = null) }
            all
        }.also { wholeJobs[key] = it }
        return try {
            job.await()
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            Toast.makeText(getApplication(), e.userMessage(), Toast.LENGTH_LONG).show()
            null
        }
    }

    /**
     * Plays [key] from [start]. A partly loaded list starts at once with what is loaded and
     * the rest follows when it arrives; shuffling (or the shared queue) waits for the whole list.
     */
    fun play(key: ListKey, start: Int = 0, shuffle: Boolean = false) {
        val current = list(key) ?: return
        if (current.items.isEmpty()) return
        if (current.complete) {
            player.playTidal(current.items, start, shuffle)
            return
        }
        if (!shuffle && !player.playsElsewhere) {
            val loaded = current.items
            player.playTidal(loaded, start)
            viewModelScope.launch {
                val all = wholeList(key) ?: return@launch
                // index of the last loaded track in the queue (playTidal queues every row)
                player.appendTidal(loaded.last().id, loaded.lastIndex, all.drop(loaded.size))
            }
            return
        }
        viewModelScope.launch {
            val all = wholeList(key) ?: return@launch
            player.playTidal(all, start, shuffle)
        }
    }

    /** Adds every track of [key] to the queue (next or at the end). */
    fun queue(key: ListKey, next: Boolean) {
        viewModelScope.launch {
            val all = wholeList(key) ?: return@launch
            player.queueTidal(all, next)
        }
    }

    fun download(hit: SearchHit) = download(hit.title, "${hit.kind.name.lowercase()}/${hit.id}")

    fun download(label: String, link: String) {
        downloads.enqueue(label, listOf(link))
        _queued.update { it + link }
    }

    /** Downloads every track of [key] that isn't queued yet (fetching the rest first). */
    fun downloadAll(label: String, key: ListKey) {
        viewModelScope.launch {
            val tracks = wholeList(key) ?: return@launch
            val links = tracks.filter { it.kind == ItemKind.TRACK }.map { "track/${it.id}" }.filterNot { it in _queued.value }
            if (links.isEmpty()) return@launch
            downloads.enqueue(label, links)
            _queued.update { it + links }
        }
    }

    private fun forget(key: ListKey) {
        moreJobs.remove(key)?.cancel()
        wholeJobs.remove(key)?.cancel()
    }

    private suspend fun fetchPage(key: ListKey, offset: UInt): HitPage = when (key) {
        is ListKey.Of -> when (key.collection) {
            Collection.PLAYLISTS -> engine.myPlaylistsPage(offset)
            Collection.TRACKS -> engine.favoriteTracksPage(offset)
            Collection.ALBUMS -> engine.favoriteAlbumsPage(offset)
            Collection.ARTISTS -> engine.favoriteArtistsPage(offset)
        }
        is ListKey.Page -> when (val r = key.route) {
            is TidalRoute.Album -> engine.albumItemsPage(r.id, offset)
            is TidalRoute.Playlist -> engine.playlistItemsPage(r.uuid, offset)
            is TidalRoute.Artist -> HitPage(emptyList(), 0u, null)
        }
    }

    /** The whole list in one go (the core fetches its pages in parallel). */
    private suspend fun fetchWhole(key: ListKey): List<SearchHit> = when (key) {
        is ListKey.Of -> when (key.collection) {
            Collection.PLAYLISTS -> engine.myPlaylists()
            Collection.TRACKS -> engine.favoriteTracks()
            Collection.ALBUMS -> engine.favoriteAlbums()
            Collection.ARTISTS -> engine.favoriteArtists()
        }
        is ListKey.Page -> when (val r = key.route) {
            is TidalRoute.Album -> engine.albumPage(r.id).tracks
            is TidalRoute.Playlist -> engine.playlistPage(r.uuid).tracks
            is TidalRoute.Artist -> (_pages.value[r] as? Load.Ready)?.value.let { (it as? ArtistPage)?.topTracks }.orEmpty()
        }
    }

    private fun list(key: ListKey): PagedHits? = when (key) {
        is ListKey.Of -> (_collection.value[key.collection] as? Load.Ready)?.value
        is ListKey.Page -> ((_pages.value[key.route] as? Load.Ready)?.value as? TrackListPage<*>)?.tracks
    }

    private fun updateList(key: ListKey, transform: (PagedHits) -> PagedHits) {
        when (key) {
            is ListKey.Of -> _collection.update { map ->
                val ready = map[key.collection] as? Load.Ready ?: return@update map
                map + (key.collection to Load.Ready(transform(ready.value)))
            }
            is ListKey.Page -> _pages.update { map ->
                val page = (map[key.route] as? Load.Ready)?.value as? TrackListPage<*> ?: return@update map
                map + (key.route to Load.Ready(page.copy(tracks = transform(page.tracks))))
            }
        }
    }
}

/** Pages are typed by route. */
@Suppress("UNCHECKED_CAST")
fun Load<Any>.artist(): Load<ArtistPage> = this as Load<ArtistPage>

@Suppress("UNCHECKED_CAST")
fun Load<Any>.album(): Load<TrackListPage<AlbumPage>> = this as Load<TrackListPage<AlbumPage>>

@Suppress("UNCHECKED_CAST")
fun Load<Any>.playlist(): Load<TrackListPage<PlaylistPage>> = this as Load<TrackListPage<PlaylistPage>>
