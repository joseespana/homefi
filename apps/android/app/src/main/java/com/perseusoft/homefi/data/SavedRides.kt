package com.perseusoft.homefi.data

import android.content.Context
import android.util.Log
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.ExerciseRides
import com.perseusoft.homefi.core.HomePlaylist
import com.perseusoft.homefi.core.LibraryRef
import com.perseusoft.homefi.core.PlaylistOrigin
import com.perseusoft.homefi.core.RideBuilt
import com.perseusoft.homefi.core.RideOrigin
import com.perseusoft.homefi.core.RidePlan
import com.perseusoft.homefi.core.RideRequest
import com.perseusoft.homefi.core.SavedRide
import com.perseusoft.homefi.core.SavedRideSummary
import com.perseusoft.homefi.core.exerciseRideExtension
import com.perseusoft.homefi.library.LibraryRepository
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.util.UUID

private const val TAG = "SavedRides"

/** The playlist format the core writes today (`PLAYLIST_FORMAT_VERSION`). */
private const val PLAYLIST_VERSION = 1u

/** The library as the core matches songs against it (a ride's local songs are found again by ISRC, artist and title). */
fun LibraryRepository.refs(): List<LibraryRef> =
    state.value.tracks.map { LibraryRef(it.path, it.title, it.artist, it.album, it.isrc, it.durationMs, bpm = it.bpm, genre = it.genre) }

/**
 * The rides saved in HomeFi's own folder (`<app dir>/exercise/rides`, the core's store): every successful
 * build is saved by itself, the rider keeps, renames and favorites the ones they like and rides them again
 * without another AI call. Every core call runs off Main; failures become plain messages in [notes].
 */
class SavedRides(
    private val context: Context,
    appDir: String,
    private val scope: CoroutineScope,
    private val notes: Notes,
    private val libraryRefs: () -> List<LibraryRef>,
) {
    private val store: ExerciseRides = ExerciseRides(appDir)

    private val _list = MutableStateFlow<List<SavedRideSummary>>(emptyList())

    /** Favorites first, then the most recent. */
    val list: StateFlow<List<SavedRideSummary>> = _list.asStateFlow()

    fun refresh() {
        scope.launch { reload() }
    }

    private suspend fun reload() {
        _list.value = withContext(Dispatchers.IO) { runCatching { store.list() }.getOrDefault(emptyList()) }
    }

    /** Saves a finished build by itself and prunes the history to [historyMax]. Null when it could not be saved (the ride still works). */
    suspend fun autoSave(request: RideRequest, built: RideBuilt, language: String, historyMax: UShort): SavedRideSummary? =
        runCatching {
            withContext(Dispatchers.IO) { store.autoSave(request, built.plan, built.playlist.id, language, historyMax) }
        }.onFailure {
            if (it is CancellationException) throw it
            Log.w(TAG, "auto-save failed", it)
        }.getOrNull().also { reload() }

    /** The saved ride ready to start: its plan and a playlist of its songs (no build, no AI). Null (with a message) when it is gone or damaged. */
    suspend fun load(id: String): Loaded? {
        val ride = guarded { withContext(Dispatchers.IO) { store.get(id) } } ?: return null
        return Loaded(ride, ride.toBuilt())
    }

    class Loaded(val ride: SavedRide, val built: RideBuilt)

    suspend fun keep(id: String, name: String? = null): SavedRideSummary? =
        guarded { withContext(Dispatchers.IO) { store.keep(id, name) } }.also { reload() }

    suspend fun rename(id: String, name: String): SavedRideSummary? =
        guarded { withContext(Dispatchers.IO) { store.rename(id, name) } }.also { reload() }

    fun setFavorite(id: String, favorite: Boolean) {
        scope.launch { guarded { withContext(Dispatchers.IO) { store.setFavorite(id, favorite) } }; reload() }
    }

    /** Counts a ride started from a saved one; nobody needs to be told when it fails. */
    suspend fun markRidden(id: String) {
        runCatching { withContext(Dispatchers.IO) { store.markRidden(id) } }
        reload()
    }

    fun delete(id: String) {
        scope.launch {
            guarded { withContext(Dispatchers.IO) { store.delete(id) } }
            reload()
        }
    }

    /** The `.homefiride` text of a saved ride, or null (with a message). */
    suspend fun export(id: String): String? = guarded { withContext(Dispatchers.IO) { store.export(id) } }

    /** Adds a ride from `.homefiride` text as a new kept ride; says what happened. */
    suspend fun import(json: String): SavedRideSummary? {
        val summary = guarded { store.import(json, libraryRefs()) } ?: return null
        reload()
        notes.post(context.getString(R.string.rides_imported, summary.name))
        return summary
    }

    /** Runs a core call; a failure goes to [notes] in plain words and gives null. */
    private suspend fun <T> guarded(block: suspend () -> T): T? = try {
        block()
    } catch (e: CancellationException) {
        throw e
    } catch (e: Exception) {
        notes.post(e.userMessage())
        null
    }

    companion object {
        /** `homefiride`, from the core. */
        val extension: String by lazy { exerciseRideExtension() }
    }
}

/** What the player plays for a saved ride: a playlist of the plan's songs, under the ride's own playlist id when it has one. */
private fun SavedRide.toBuilt(): RideBuilt {
    val now = System.currentTimeMillis().toULong()
    val playlist = HomePlaylist(
        version = PLAYLIST_VERSION,
        id = playlistId?.takeIf { it.isNotBlank() } ?: UUID.randomUUID().toString(),
        name = plan.workout.name,
        description = "",
        createdMs = now,
        updatedMs = now,
        origin = PlaylistOrigin.Ride(RideOrigin(plan.workout.id, plan.wish)),
        entries = plan.songs.map { it.entry },
        unmatched = emptyList(),
        sources = emptyList(),
        searchWidgetHtml = null,
        tidalUuid = null,
    )
    return RideBuilt(plan, playlist)
}

