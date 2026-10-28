package com.perseusoft.homefi.ui.search

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.perseusoft.homefi.core.ItemKind
import com.perseusoft.homefi.core.Link
import com.perseusoft.homefi.core.SearchHit
import com.perseusoft.homefi.core.SearchResults
import com.perseusoft.homefi.core.parseLink
import com.perseusoft.homefi.data.userMessage
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.Busy
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class SearchUiState(
    val query: String = "",
    val link: Link? = null,
    val loading: Boolean = false,
    val results: SearchResults? = null,
    val error: String? = null,
)

fun ItemKind.path(): String = name.lowercase()

class SearchViewModel(app: Application) : AndroidViewModel(app) {
    private val engine = app.homefi.engine
    private val downloads = app.homefi.downloads

    private val _state = MutableStateFlow(SearchUiState())
    val state: StateFlow<SearchUiState> = _state.asStateFlow()
    private var job: Job? = null

    fun onQueryChange(query: String) {
        val link = runCatching { parseLink(query) }.getOrNull()
        _state.update { it.copy(query = query, link = link, error = null) }
        job?.cancel()
        if (link != null || query.isBlank()) {
            _state.update { it.copy(results = if (query.isBlank()) null else it.results, loading = false) }
            return
        }
        job = viewModelScope.launch {
            delay(350) // debounce typing
            search(query)
        }
    }

    fun submit() {
        val s = _state.value
        when {
            s.link != null -> downloadLink(s.link)
            s.query.isNotBlank() -> {
                job?.cancel()
                job = viewModelScope.launch { search(s.query) }
            }
        }
    }

    private suspend fun search(query: String) {
        _state.update { it.copy(loading = true, error = null) }
        try {
            val results = Busy.track { engine.search(query) }
            _state.update { it.copy(results = results, loading = false) }
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (e: Exception) {
            _state.update { it.copy(loading = false, error = e.userMessage()) }
        }
    }

    fun download(hit: SearchHit) {
        downloads.enqueue(hit.title, listOf("${hit.kind.path()}/${hit.id}"))
    }

    fun downloadLink(link: Link) {
        downloads.enqueue("${link.kind.path()}/${link.id}", listOf("${link.kind.path()}/${link.id}"))
        _state.update { it.copy(query = "", link = null) }
    }
}
