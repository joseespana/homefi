package com.perseusoft.homefi.tv

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Album
import androidx.compose.material.icons.rounded.History
import androidx.compose.material.icons.rounded.Mic
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.focus.focusRequester
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.LibraryTrack
import com.perseusoft.homefi.core.SearchHit
import com.perseusoft.homefi.core.SearchResults
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.library.EmbeddedCover
import com.perseusoft.homefi.library.albums
import com.perseusoft.homefi.library.filterQuery
import com.perseusoft.homefi.player.Sources
import com.perseusoft.homefi.ui.formatDuration
import com.perseusoft.homefi.ui.search.SearchViewModel
import com.perseusoft.homefi.ui.tidal.RowContext
import com.perseusoft.homefi.ui.tidal.TidalViewModel
import com.perseusoft.homefi.ui.tidal.rememberRowContext
import com.perseusoft.homefi.ui.theme.LocalMotion

private const val TIDAL_SONGS = 8
private const val TIDAL_CARDS = 10
private const val LIBRARY_SONGS = 6
private const val LIBRARY_ALBUMS = 10
private val RESULT_CARD = 96.dp

/** What the right pane shows under the keyboard's query. */
private enum class SearchBody { HINT, LOADING, ERROR, EMPTY, RESULTS }

/**
 * Search (docs/TV_UI.md 6.3): the field with its mic and the on-screen keyboard on the left, the recent
 * searches under it; on the right the results from TIDAL (when signed in) and the library. Queries run
 * through the phone's [SearchViewModel]; a TIDAL album / artist / playlist opens its page here.
 */
@Composable
fun TvSearch(entryKey: Int) {
    val vm: SearchViewModel = viewModel()
    val tidal: TidalViewModel = viewModel()
    TvTidalPages(tidal, entryKey) { restoreKey, nonce -> SearchRoot(vm, tidal, entryKey, restoreKey, nonce) }
}

@OptIn(ExperimentalLayoutApi::class, ExperimentalFoundationApi::class)
@Composable
private fun SearchRoot(vm: SearchViewModel, tidal: TidalViewModel, entryKey: Int, restoreKey: Any?, restoreNonce: Int) {
    val app = LocalContext.current.homefi
    val state by vm.state.collectAsStateWithLifecycle()
    val library by app.library.state.collectAsStateWithLifecycle()
    val account by app.account.account.collectAsStateWithLifecycle()
    val recents by app.recentSearches.items.collectAsStateWithLifecycle()
    var query by rememberSaveable { mutableStateOf("") }
    // RIGHT from the keys lands on the first result, LEFT from a result on the first key
    val resultsEntry = remember { androidx.compose.ui.focus.FocusRequester() }
    val firstKey = remember { androidx.compose.ui.focus.FocusRequester() }
    val signedIn = account != null
    val look = LocalTvLook.current
    val safe = tvSafePadding()
    val mic = rememberVoiceInput { query = it; app.recentSearches.add(it) }

    // the view model debounces typing; signed out, there is nothing to ask TIDAL
    LaunchedEffect(query, signedIn) { if (signedIn) vm.onQueryChange(query) }

    val localTracks = remember(library.tracks, query) { if (query.isBlank()) emptyList() else library.tracks.filterQuery(query) }
    val results = state.results.takeIf { signedIn && query.isNotBlank() && state.query == query }
    val body = when {
        query.isBlank() -> SearchBody.HINT
        signedIn && state.error != null -> SearchBody.ERROR
        signedIn && state.loading && results == null && localTracks.isEmpty() -> SearchBody.LOADING
        results.isEmpty() && localTracks.isEmpty() && !(signedIn && (state.loading || results == null)) -> SearchBody.EMPTY
        else -> SearchBody.RESULTS
    }

    Row(
        Modifier.fillMaxSize().padding(top = safe.calculateTopPadding(), end = safe.calculateRightPadding(androidx.compose.ui.unit.LayoutDirection.Ltr)),
        horizontalArrangement = Arrangement.spacedBy(22.dp),
    ) {
        Column(Modifier.width(TV_KEYBOARD_WIDTH + 12.dp).padding(start = 12.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            TvScreenHeader(stringResource(R.string.tv_nav_search), null)
            TvField(query, stringResource(R.string.tv_search_hint), Modifier.fillMaxWidth(), onMic = mic)
            TvKeyboard(
                text = query,
                onText = { query = it },
                onSubmit = {
                    vm.submit()
                    app.recentSearches.add(query)
                },
                firstFocus = Modifier.tvInitialFocus(firstKey, key = entryKey),
                rightTo = resultsEntry,
                submitLabel = stringResource(R.string.tv_kb_search),
            )
            if (mic != null) TvPill(stringResource(R.string.tv_kb_speak), mic, Modifier.tvLeftToDrawer(), icon = Icons.Rounded.Mic, height = 30.dp)
            if (recents.isNotEmpty()) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    Icon(Icons.Rounded.History, null, tint = look.ink3, modifier = Modifier.width(16.dp))
                    Text(stringResource(R.string.tv_search_recent).uppercase(), style = TvType.caps, color = look.ink3)
                }
                FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    recents.forEachIndexed { i, q ->
                        TvPill(q, { query = q }, Modifier.then(if (i == 0) Modifier.tvLeftToDrawer() else Modifier), height = 26.dp, labelStyle = TvType.secondary.copy(fontSize = 12.sp))
                    }
                }
            }
        }
        val motion = LocalMotion.current
        AnimatedContent(
            targetState = body,
            transitionSpec = { fadeIn(motion.tween(200, delay = 80)) togetherWith fadeOut(motion.tween(80)) },
            label = "tv-search-body",
            modifier = Modifier.weight(1f).fillMaxSize(),
        ) { shown ->
            when (shown) {
                SearchBody.HINT -> SearchHint(signedIn)
                SearchBody.LOADING -> TvSkeleton()
                SearchBody.ERROR -> TvErrorState(state.error.orEmpty(), vm::submit, entryKey)
                SearchBody.EMPTY -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        Text(stringResource(R.string.tv_search_empty, query), style = TvType.section.copy(fontSize = 22.sp), color = look.ink, textAlign = TextAlign.Center)
                        Text(stringResource(R.string.tv_search_empty_hint), style = TvType.secondary.copy(fontSize = 13.sp), color = look.ink2, textAlign = TextAlign.Center)
                    }
                }
                SearchBody.RESULTS -> SearchResultsPane(tidal, results, localTracks, query, restoreKey, restoreNonce, resultsEntry, firstKey)
            }
        }
    }
}

private fun SearchResults?.isEmpty(): Boolean = this == null || (topHit == null && tracks.isEmpty() && albums.isEmpty() && artists.isEmpty() && playlists.isEmpty())

@Composable
private fun SearchHint(signedIn: Boolean) {
    val look = LocalTvLook.current
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Icon(Icons.Rounded.Search, null, tint = look.accent2, modifier = Modifier.width(44.dp))
            Text(stringResource(R.string.tv_search_try), style = TvType.section, color = look.ink, textAlign = TextAlign.Center)
            Text(
                stringResource(if (signedIn) R.string.tv_search_scope else R.string.tv_search_scope_local),
                style = TvType.secondary.copy(fontSize = 13.sp),
                color = look.ink2,
                textAlign = TextAlign.Center,
            )
        }
    }
}

@Composable
private fun SearchResultsPane(
    tidal: TidalViewModel,
    results: SearchResults?,
    local: List<LibraryTrack>,
    query: String,
    restoreKey: Any?,
    restoreNonce: Int,
    resultsEntry: androidx.compose.ui.focus.FocusRequester,
    firstKey: androidx.compose.ui.focus.FocusRequester,
) {
    val app = LocalContext.current.homefi
    val ctx = rememberRowContext(tidal)
    val library by app.library.state.collectAsStateWithLifecycle()
    val playing by app.player.state.collectAsStateWithLifecycle()
    val localAlbums = remember(local) { local.albums().take(LIBRARY_ALBUMS) }
    val nav = LocalTvNav.current
    var menu by remember { mutableStateOf<LibraryTrack?>(null) }
    LazyColumn(
        Modifier.fillMaxSize(),
        contentPadding = PaddingValues(start = 12.dp, end = 24.dp, top = 12.dp, bottom = 28.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        if (results != null) {
            tidalSongs(tidal, ctx, results, app.recentSearches::add, query, restoreKey, restoreNonce, resultsEntry, firstKey)
            hitShelf(tidal, R.string.search_albums, results.albums.take(TIDAL_CARDS), app.recentSearches::add, query)
            hitShelf(tidal, R.string.search_artists, results.artists.take(TIDAL_CARDS), app.recentSearches::add, query)
            hitShelf(tidal, R.string.search_playlists, results.playlists.take(TIDAL_CARDS), app.recentSearches::add, query)
        }
        if (local.isNotEmpty()) {
            item(key = "lib-title") { SectionLabel(stringResource(R.string.tv_search_library)) }
            val songs = local.take(LIBRARY_SONGS)
            itemsIndexed(songs, key = { _, t -> "lib-${t.path}" }) { i, track ->
                val gone = library.unavailable(track.path)
                TvRow(
                    title = track.title,
                    subtitle = listOf(track.artist, track.album).filter(String::isNotBlank).joinToString(" · "),
                    cover = EmbeddedCover(track.path),
                    playing = track.path == playing.current?.mediaId,
                    dimmed = gone,
                    tag = if (gone) stringResource(R.string.tv_lib_unavailable) else track.format,
                    trailing = if (track.durationMs > 0uL) formatDuration(track.durationMs.toLong()) else null,
                    onClick = {
                        if (!gone) {
                            app.recentSearches.add(query)
                            app.player.play(songs.filterNot { library.unavailable(it.path) }, songs.filterNot { library.unavailable(it.path) }.indexOf(track).coerceAtLeast(0), source = Sources.library())
                            nav.openPlayer()
                        }
                    },
                    onLongClick = { menu = track },
                    modifier = (if (i == 0 && results?.tracks.isNullOrEmpty()) Modifier.focusRequester(resultsEntry) else Modifier).tvLeftTo(firstKey),
                )
            }
        }
    }
    menu?.let { track -> TvActionMenu(track.title, track.artist, libraryTrackActions(track)) { menu = null } }
}

private fun LazyListScope.tidalSongs(
    vm: TidalViewModel,
    ctx: RowContext,
    results: SearchResults,
    record: (String) -> Unit,
    query: String,
    restoreKey: Any?,
    restoreNonce: Int,
    resultsEntry: androidx.compose.ui.focus.FocusRequester,
    firstKey: androidx.compose.ui.focus.FocusRequester,
) {
    val songs = results.tracks.take(TIDAL_SONGS)
    if (songs.isEmpty()) return
    item(key = "tidal-title") { SectionLabel(stringResource(R.string.tv_search_tidal) + " · " + stringResource(R.string.search_tracks)) }
    itemsIndexed(songs, key = { i, h -> "t-$i-${h.id}" }) { i, hit ->
        TvHitRow(vm, ctx, hit, songs, i, key = null, restoreKey = restoreKey, restoreNonce = restoreNonce, onOpened = { record(query) }, entry = resultsEntry.takeIf { i == 0 }, leftTo = firstKey)
    }
}

/** A titled row of cover cards (TIDAL albums, artists, playlists). */
private fun LazyListScope.hitShelf(vm: TidalViewModel, title: Int, hits: List<SearchHit>, record: (String) -> Unit, query: String) {
    if (hits.isEmpty()) return
    item(key = "shelf-$title") {
        var menu by remember { mutableStateOf<SearchHit?>(null) }
        Column(Modifier.padding(top = 8.dp)) {
            SectionLabel(stringResource(title))
            LazyRow(contentPadding = PaddingValues(start = 4.dp, end = 18.dp, top = 12.dp, bottom = 16.dp), horizontalArrangement = Arrangement.spacedBy(TvDimens.gap)) {
                itemsIndexed(hits, key = { i, h -> "$i-${h.kind}-${h.id}" }) { _, hit ->
                    TvHitCard(hit, RESULT_CARD, onClick = { record(query); vm.open(hit) }, onLongClick = { menu = hit })
                }
            }
        }
        menu?.let { hit -> TvActionMenu(hit.title, hit.subtitle, tidalHitActions(hit, vm)) { menu = null } }
    }
}

@Composable
private fun SectionLabel(text: String) {
    Text(text, style = TvType.section, color = LocalTvLook.current.ink, modifier = Modifier.fillMaxWidth().padding(start = 4.dp, top = 2.dp, bottom = 4.dp))
}
