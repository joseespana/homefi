package com.perseusoft.homefi.share

import com.perseusoft.homefi.HomefiApplication
import com.perseusoft.homefi.core.HubCommand
import com.perseusoft.homefi.core.HubEvent
import com.perseusoft.homefi.core.HubHostCallback
import com.perseusoft.homefi.core.ItemKind
import com.perseusoft.homefi.core.SearchHit
import com.perseusoft.homefi.core.ShareBrowseKind
import com.perseusoft.homefi.core.ShareCollection
import com.perseusoft.homefi.core.SharePage
import com.perseusoft.homefi.core.ShareSearchResults
import com.perseusoft.homefi.core.readCover
import com.perseusoft.homefi.library.albumKey
import com.perseusoft.homefi.library.albums
import com.perseusoft.homefi.library.artists
import com.perseusoft.homefi.library.filterQuery
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runBlocking

/**
 * The hub's answers to guests: search and browse the hub's library and its
 * Tidal account, covers of local songs, and player commands (handed to
 * [ShareManager]). Rust calls `search`, `browse` and `cover` on a blocking
 * thread, the rest from its network threads.
 */
internal class HubHost(private val app: HomefiApplication, private val manager: ShareManager) : HubHostCallback {
    private val tracks get() = app.library.state.value.tracks
    private val loggedIn get() = app.account.isLoggedIn

    override fun search(q: String): ShareSearchResults? {
        val query = q.trim()
        if (query.isEmpty()) return ShareSearchResults(q, emptyList(), emptyList(), emptyList(), emptyList())
        val local = tracks.filterQuery(query)
        val localAlbums = local.albums().take(LOCAL_COLLECTIONS).map { a ->
            ShareCollection(ShareBrowseKind.ALBUM, LOCAL_COLLECTION + encodeId(a.key), a.title, a.artist, localCoverUrl(a.tracks.first().path))
        }
        val localArtists = local.artists().filter { it.name.contains(query, true) }.take(LOCAL_COLLECTIONS).map { a ->
            ShareCollection(ShareBrowseKind.ARTIST, LOCAL_COLLECTION + encodeId(a.name), a.name, "", localCoverUrl(a.tracks.first().path))
        }
        val tidal = if (loggedIn) runCatching { blocking { app.engine.search(query) } }.getOrNull() else null
        if (tidal == null && local.isEmpty() && loggedIn) return null
        return ShareSearchResults(
            q = q,
            tracks = local.take(LOCAL_TRACKS).map { it.toShareTrack() } +
                tidal?.tracks.orEmpty().filter { it.kind == ItemKind.TRACK }.map(SearchHit::toShareTrack),
            albums = localAlbums + tidal?.albums.orEmpty().mapNotNull(SearchHit::toCollection),
            artists = localArtists + tidal?.artists.orEmpty().mapNotNull(SearchHit::toCollection),
            playlists = tidal?.playlists.orEmpty().mapNotNull(SearchHit::toCollection),
        )
    }

    override fun browse(kind: ShareBrowseKind, id: String): SharePage? {
        if (id.startsWith(LOCAL_COLLECTION)) return browseLocal(kind, decodeId(id.removePrefix(LOCAL_COLLECTION)) ?: return null, id)
        if (!loggedIn) return null
        return runCatching {
            blocking {
                when (kind) {
                    ShareBrowseKind.ALBUM -> app.engine.albumPage(id).let { p ->
                        SharePage(ShareCollection(kind, id, p.title, p.artist, p.coverUrl), p.tracks.tracksOnly())
                    }
                    ShareBrowseKind.ARTIST -> app.engine.artistPage(id).let { p ->
                        SharePage(ShareCollection(kind, id, p.name, "", p.pictureUrl), p.topTracks.tracksOnly())
                    }
                    ShareBrowseKind.PLAYLIST -> app.engine.playlistPage(id).let { p ->
                        SharePage(ShareCollection(kind, id, p.title, p.description.orEmpty(), p.coverUrl), p.tracks.tracksOnly())
                    }
                    ShareBrowseKind.UNKNOWN -> null
                }
            }
        }.getOrNull()
    }

    private fun browseLocal(kind: ShareBrowseKind, key: String, id: String): SharePage? {
        val list = when (kind) {
            ShareBrowseKind.ALBUM -> tracks.filter { albumKey(it) == key }.albums().firstOrNull()?.let { a ->
                ShareCollection(kind, id, a.title, a.artist, localCoverUrl(a.tracks.first().path)) to a.tracks
            }
            ShareBrowseKind.ARTIST -> tracks.artists().firstOrNull { it.name == key }?.let { a ->
                val sorted = a.tracks.albums().flatMap { it.tracks }
                ShareCollection(kind, id, a.name, "", localCoverUrl(a.tracks.first().path)) to sorted
            }
            else -> null
        } ?: return null
        return SharePage(list.first, list.second.map { it.toShareTrack() })
    }

    /** Embedded art of a library song (nothing else on the phone is served). */
    override fun cover(id: String): ByteArray? {
        val path = decodeId(id) ?: return null
        if (app.library.find(path) == null && !manager.isPlaying(path)) return null
        return runCatching { readCover(path) }.getOrNull()
    }

    override fun onCommand(command: HubCommand) = manager.onHubCommand(command)

    override fun onEvent(event: HubEvent) = manager.onHubEvent(event)

    private fun List<SearchHit>.tracksOnly() = filter { it.kind == ItemKind.TRACK }.map(SearchHit::toShareTrack)

    private fun <T> blocking(block: suspend () -> T): T = runBlocking(Dispatchers.IO) { block() }

    private companion object {
        const val LOCAL_TRACKS = 30
        const val LOCAL_COLLECTIONS = 10
    }
}
