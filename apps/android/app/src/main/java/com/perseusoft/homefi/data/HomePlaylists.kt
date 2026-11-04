package com.perseusoft.homefi.data

import com.perseusoft.homefi.core.HomePlaylist
import com.perseusoft.homefi.core.HomefiEngine
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * The playlists HomeFi built, kept as JSON by the core in its own folder (never in the Tidal
 * account). Every disk call runs on the IO dispatcher.
 */
class HomePlaylists(private val engine: HomefiEngine, private val scope: CoroutineScope) {
    private val _list = MutableStateFlow<List<HomePlaylist>>(emptyList())

    /** Most recently updated first. */
    val list: StateFlow<List<HomePlaylist>> = _list.asStateFlow()

    /** Reads the folder again (when the Playlists tab shows). */
    fun refresh() {
        scope.launch { reload() }
    }

    /** Saves [playlist] (new, a rebuild or an undone delete) and updates [list]. Returns the error text, or null. */
    suspend fun save(playlist: HomePlaylist): String? = runCatching {
        withContext(Dispatchers.IO) { engine.homePlaylistSave(playlist) }
        reload()
    }.exceptionOrNull()?.userMessage()

    /** Removes HomeFi's own file for [id]; the music and the Tidal account aren't touched. */
    suspend fun delete(id: String): String? = runCatching {
        withContext(Dispatchers.IO) { engine.homePlaylistDelete(id) }
        reload()
    }.exceptionOrNull()?.userMessage()

    private suspend fun reload() {
        val fresh = runCatching { withContext(Dispatchers.IO) { engine.homePlaylists() } }.getOrNull() ?: return
        _list.value = fresh
    }
}
