package com.perseusoft.homefi.data

import android.util.Log
import com.perseusoft.homefi.core.HomePlaylist
import com.perseusoft.homefi.core.HomefiEngine
import com.perseusoft.homefi.core.PlaylistSettings
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.util.Locale

/** The last save of one playlist to TIDAL (the button or the automatic one). */
sealed interface TidalSave {
    val id: String

    data class Saving(override val id: String) : TidalSave
    /** [summary]: "Saved 24 songs to TIDAL · 2 aren't on TIDAL", worded by the core. */
    data class Saved(override val id: String, val summary: String, val skipped: List<String>) : TidalSave
    data class Failed(override val id: String, val message: String) : TidalSave
}

/**
 * The playlists HomeFi built, kept as JSON by the core in its own folder. The TIDAL account
 * is written only to save one there ("Save to TIDAL", or [PlaylistSettings.tidalAutoSave]) and
 * to delete that copy ([PlaylistSettings.tidalDeleteWith]). Every core call runs off Main.
 */
class HomePlaylists(
    private val engine: HomefiEngine,
    private val scope: CoroutineScope,
    private val settings: StateFlow<PlaylistSettings>,
    /** Signed in to Tidal, as far as known now (never blocks). */
    private val signedIn: () -> Boolean,
) {
    private val _list = MutableStateFlow<List<HomePlaylist>>(emptyList())

    /** Most recently updated first. */
    val list: StateFlow<List<HomePlaylist>> = _list.asStateFlow()

    private val _tidal = MutableStateFlow<TidalSave?>(null)

    /** The last save to TIDAL, for the playlist it names. */
    val tidal: StateFlow<TidalSave?> = _tidal.asStateFlow()

    /** Reads the folder again (when the Playlists tab shows). */
    fun refresh() {
        scope.launch { reload() }
    }

    /**
     * Saves [playlist] (new, a rebuild or an undone delete) and updates [list]. With "save to
     * TIDAL automatically" on it is then saved to TIDAL in the background. Returns the error
     * text, or null.
     */
    suspend fun save(playlist: HomePlaylist): String? {
        val failure = runCatching {
            withContext(Dispatchers.IO) { engine.homePlaylistSave(playlist) }
            reload()
        }.exceptionOrNull()?.userMessage()
        if (failure == null && settings.value.tidalAutoSave && signedIn()) {
            scope.launch { saveToTidal(playlist.id) }
        }
        return failure
    }

    /** "Save to TIDAL" / "Update on TIDAL": its TIDAL copy gets the same songs, in order. */
    suspend fun saveToTidal(id: String) {
        if (_tidal.value.let { it is TidalSave.Saving && it.id == id }) return
        _tidal.value = TidalSave.Saving(id)
        _tidal.value = runCatching { engine.homePlaylistSaveToTidal(id, Locale.getDefault().toLanguageTag()) }.fold(
            onSuccess = { done ->
                _list.value = _list.value.map { if (it.id == id) done.playlist else it }
                TidalSave.Saved(id, done.report.summary, done.report.skipped)
            },
            onFailure = { TidalSave.Failed(id, it.userMessage()) },
        )
    }

    /** Removes HomeFi's own file for [id]; the music isn't touched (see [deleteTidalCopy]). */
    suspend fun delete(id: String): String? = runCatching {
        withContext(Dispatchers.IO) { engine.homePlaylistDelete(id) }
        reload()
    }.exceptionOrNull()?.userMessage()

    /** True when deleting [playlist] would also delete its TIDAL copy (for the confirmation). */
    fun deletesTidalCopy(playlist: HomePlaylist): Boolean = playlist.tidalUuid != null && settings.value.tidalDeleteWith

    /**
     * After a delete that wasn't undone: with "delete the TIDAL copy too" on, the playlist's
     * copy in the TIDAL account goes as well. Best effort: a failure is only logged.
     */
    suspend fun deleteTidalCopy(playlist: HomePlaylist) {
        if (!deletesTidalCopy(playlist)) return
        runCatching { engine.homePlaylistDeleteOnTidal(playlist) }
            .onFailure { Log.w(TAG, "could not delete the TIDAL copy of ${playlist.id}: ${it.userMessage()}") }
    }

    private suspend fun reload() {
        val fresh = runCatching { withContext(Dispatchers.IO) { engine.homePlaylists() } }.getOrNull() ?: return
        _list.value = fresh
    }

    private companion object {
        const val TAG = "HomePlaylists"
    }
}
