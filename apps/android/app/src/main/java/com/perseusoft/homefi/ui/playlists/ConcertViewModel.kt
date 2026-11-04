package com.perseusoft.homefi.ui.playlists

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.perseusoft.homefi.HomefiApplication
import com.perseusoft.homefi.core.ConcertBuilt
import com.perseusoft.homefi.core.ConcertListener
import com.perseusoft.homefi.core.ConcertMode
import com.perseusoft.homefi.core.ConcertOrigin
import com.perseusoft.homefi.core.ConcertProgress
import com.perseusoft.homefi.core.ConcertRequest
import com.perseusoft.homefi.core.ConcertThresholds
import com.perseusoft.homefi.core.HomePlaylist
import com.perseusoft.homefi.core.LibraryRef
import com.perseusoft.homefi.core.PlaylistOrigin
import com.perseusoft.homefi.data.userMessage
import com.perseusoft.homefi.homefi
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.util.Locale

/** What the concert screen (or a saved playlist's "Search again") is doing. */
sealed interface ConcertUi {
    data object Idle : ConcertUi

    /** [progress] is null until the core reports the first step; [aiName] is the provider searching. */
    data class Building(val progress: ConcertProgress?, val aiName: String?) : ConcertUi

    /** A draft (not saved yet) or, once [saved], the playlist as stored. */
    data class Built(val built: ConcertBuilt, val saved: Boolean = false) : ConcertUi

    data class Failed(val message: String) : ConcertUi
}

/** Where the user's AI stands, derived from the settings and the key vault (no network). */
sealed interface AiStand {
    data object None : AiStand
    data class NoWebSearch(val name: String) : AiStand
    data class Searching(val name: String, val searches: Int) : AiStand
}

/**
 * Builds concert playlists through the core. The listener runs on a Rust thread, so it only
 * touches the thread-safe state flow. A cancelled coroutine aborts the Rust task.
 */
class ConcertViewModel(app: Application) : AndroidViewModel(app) {
    private val home = app.homefi
    private val _ui = MutableStateFlow<ConcertUi>(ConcertUi.Idle)
    val ui: StateFlow<ConcertUi> = _ui.asStateFlow()

    private val _ai = MutableStateFlow<AiStand>(AiStand.None)
    val ai: StateFlow<AiStand> = _ai.asStateFlow()

    private var job: Job? = null

    private val listener = object : ConcertListener {
        override fun onProgress(progress: ConcertProgress) {
            _ui.update { if (it is ConcertUi.Building) it.copy(progress = progress) else it }
        }
    }

    /** Reads the AI choice again (after the user set one up). */
    fun refreshAi() {
        viewModelScope.launch {
            val ready = withContext(Dispatchers.IO) { home.ai.ready() }
            val budget = home.settings.playlists.value.searchBudget.toInt()
            _ai.value = when {
                ready == null -> AiStand.None
                !ready.info.webSearch -> AiStand.NoWebSearch(ready.info.name)
                else -> AiStand.Searching(ready.info.name, budget)
            }
        }
    }

    /**
     * Looks up [artist]'s recent shows and builds the playlist. With [replaceId] (Search again on a
     * saved playlist) the result keeps that id and is saved at once.
     */
    fun build(artist: String, shows: Int, popularOnly: Boolean, replaceId: String? = null) {
        job?.cancel()
        job = viewModelScope.launch {
            val ready = if (popularOnly) null else withContext(Dispatchers.IO) { home.ai.ready() }
            _ui.value = ConcertUi.Building(null, ready?.info?.name)
            try {
                val settings = home.settings.playlists.value
                val request = ConcertRequest(artist.trim(), shows.toUByte(), settings.concertThresholds, ConcertMode.Tour, popularOnly)
                val built = home.engine.concertBuild(
                    request, settings, ready?.config, ready?.key, language(), libraryRefs(), replaceId, listener,
                )
                if (replaceId != null) finishSaved(built) else _ui.value = ConcertUi.Built(built)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                _ui.value = ConcertUi.Failed(e.userMessage())
            }
        }
    }

    /**
     * Re-aggregates the shows stored in [playlist] (no AI call): one show or the tour, fewer shows,
     * other thresholds. A saved playlist is saved again; a draft stays a draft.
     */
    fun rearrange(playlist: HomePlaylist, mode: ConcertMode, shows: Int, thresholds: ConcertThresholds, saved: Boolean) {
        val origin = (playlist.origin as? PlaylistOrigin.Concert)?.origin ?: return
        job?.cancel()
        job = viewModelScope.launch {
            _ui.value = ConcertUi.Building(null, null)
            try {
                val request = ConcertRequest(origin.artist, shows.toUByte(), thresholds, mode, false)
                val result = home.engine.concertRearrange(
                    playlist, request, home.settings.playlists.value, language(), libraryRefs(), listener,
                )
                if (saved) finishSaved(ConcertBuilt(result, null)) else _ui.value = ConcertUi.Built(ConcertBuilt(result, null))
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                _ui.value = ConcertUi.Failed(e.userMessage())
            }
        }
    }

    fun cancel() {
        job?.cancel()
        _ui.value = ConcertUi.Idle
    }

    /** Saves the draft in HomeFi (only there; nothing goes to Tidal). */
    fun save() {
        val built = (_ui.value as? ConcertUi.Built)?.takeIf { !it.saved } ?: return
        viewModelScope.launch {
            val error = home.playlists.save(built.built.playlist)
            _ui.value = if (error == null) built.copy(saved = true) else ConcertUi.Failed(error)
        }
    }

    fun discard() {
        _ui.value = ConcertUi.Idle
    }

    private suspend fun finishSaved(built: ConcertBuilt) {
        val error = home.playlists.save(built.playlist)
        _ui.value = if (error == null) ConcertUi.Built(built, saved = true) else ConcertUi.Failed(error)
    }

    private suspend fun libraryRefs(): List<LibraryRef> = home.libraryRefs()

    private fun language(): String = languageTag()
}

/** The scanned library as the core's references (off the main thread); shared by the playlist builders. */
internal suspend fun HomefiApplication.libraryRefs(): List<LibraryRef> = withContext(Dispatchers.Default) {
    library.state.value.tracks.map {
        LibraryRef(it.path, it.title, it.artist, it.album, it.isrc, it.durationMs)
    }
}

/** The language the core words its notes and prompts in. */
internal fun languageTag(): String = Locale.getDefault().toLanguageTag()

/** The concert origin of a playlist, when it has one. */
val HomePlaylist.concert: ConcertOrigin?
    get() = (origin as? PlaylistOrigin.Concert)?.origin
