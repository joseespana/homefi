package com.perseusoft.homefi.tv

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.LazyGridState
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.Album
import androidx.compose.material.icons.rounded.Download
import androidx.compose.material.icons.rounded.DownloadDone
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Shuffle
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.focus.focusRequester
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.ItemKind
import com.perseusoft.homefi.core.SearchHit
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.Sources
import com.perseusoft.homefi.ui.formatDuration
import com.perseusoft.homefi.ui.theme.LocalMotion
import com.perseusoft.homefi.ui.tidal.Collection
import com.perseusoft.homefi.ui.tidal.ListKey
import com.perseusoft.homefi.ui.tidal.Load
import com.perseusoft.homefi.ui.tidal.PagedHits
import com.perseusoft.homefi.ui.tidal.RowContext
import com.perseusoft.homefi.ui.tidal.TidalRoute
import com.perseusoft.homefi.ui.tidal.TidalViewModel
import com.perseusoft.homefi.ui.tidal.album
import com.perseusoft.homefi.ui.tidal.artist
import com.perseusoft.homefi.ui.tidal.icon
import com.perseusoft.homefi.ui.tidal.localKey
import com.perseusoft.homefi.ui.tidal.playlist
import com.perseusoft.homefi.ui.tidal.rememberRowContext
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.filter

/** A TIDAL list is near its end: this many rows (or cards) before it, the next page is asked for. */
private const val LOAD_AHEAD = 12
private const val SLIDE_FRACTION = 12
private val SMALL_CARD = 96.dp

/**
 * "My TIDAL" (docs/TV_UI.md 6.4): favourites, playlists, albums and artists as pills; a list or a grid of covers
 * under them that loads pages as it scrolls; album, artist and playlist pages over it (the phone's
 * [TidalViewModel] stack). TIDAL has no "mixes" listing in the engine yet, so there is no Mixes pill.
 */
@Composable
fun TvTidal(entryKey: Int) {
    val vm: TidalViewModel = viewModel()
    var tab by rememberSaveable { mutableStateOf(Collection.TRACKS) }
    TvTidalPages(vm, entryKey) { restoreKey, nonce ->
        TidalRoot(vm, tab, { tab = it }, entryKey, restoreKey, nonce)
    }
}

/**
 * The browser pages over [root]: the top of the TIDAL stack as a detail page (BACK pops it and the card
 * it came from takes focus back), or [root] when the stack is empty. Search and TIDAL share it.
 */
@Composable
fun TvTidalPages(vm: TidalViewModel, entryKey: Int, root: @Composable (restoreKey: Any?, restoreNonce: Int) -> Unit) {
    val stack by vm.stack.collectAsStateWithLifecycle()
    var restoreKey by remember { mutableStateOf<Any?>(null) }
    var restoreNonce by remember { mutableIntStateOf(0) }
    // what is opened from this screen starts a fresh stack (an older visit's pages are gone)
    LaunchedEffect(Unit) { while (vm.stack.value.isNotEmpty()) vm.pop() }
    BackHandler(enabled = stack.isNotEmpty()) {
        restoreKey = stack.lastOrNull().let { r ->
            when (r) {
                is TidalRoute.Album -> r.id
                is TidalRoute.Artist -> r.id
                is TidalRoute.Playlist -> r.uuid
                null -> null
            }
        }
        restoreNonce++
        vm.pop()
    }
    val motion = LocalMotion.current
    AnimatedContent(
        targetState = stack,
        contentKey = { it.size to it.lastOrNull() },
        transitionSpec = {
            val deeper = targetState.size > initialState.size
            val slide = if (motion.moves) slideInHorizontally(motion.tween(260)) { if (deeper) it / SLIDE_FRACTION else -it / SLIDE_FRACTION } else EnterTransition.None
            (fadeIn(motion.tween(220, delay = 90)) + slide) togetherWith fadeOut(motion.tween(90))
        },
        label = "tv-tidal-stack",
    ) { shown ->
        val route = shown.lastOrNull()
        if (route == null) root(restoreKey, restoreNonce) else TvTidalRoutePage(route, vm, entryKey)
    }
}

@Composable
private fun TidalRoot(vm: TidalViewModel, tab: Collection, onTab: (Collection) -> Unit, entryKey: Int, restoreKey: Any?, restoreNonce: Int) {
    val safe = tvSafePadding()
    val tabs = Collection.entries.sortedBy { it.order() }
    val motion = LocalMotion.current
    val bodyEntry = remember { androidx.compose.ui.focus.FocusRequester() }
    val songsState = rememberLazyListState()
    val grids = remember { mutableMapOf<Collection, LazyGridState>() }
    Column(Modifier.fillMaxSize().padding(top = safe.calculateTopPadding(), end = safe.calculateRightPadding(androidx.compose.ui.unit.LayoutDirection.Ltr))) {
        TvScreenHeader(stringResource(R.string.tv_nav_tidal), stringResource(R.string.tv_tidal_sub), Modifier.padding(start = 12.dp)) { TvNowPlayingChip() }
        TvTabs(
            labels = tabs.map { stringResource(it.label()) },
            selected = tabs.indexOf(tab),
            onSelect = { onTab(tabs[it]) },
            modifier = Modifier.padding(start = 12.dp, top = 10.dp),
            entryKey = entryKey,
            downTo = bodyEntry,
        )
        LaunchedEffect(tab) { vm.loadCollection(tab) }
        AnimatedContent(
            targetState = tab,
            transitionSpec = {
                val dir = if (targetState.order() >= initialState.order()) 1 else -1
                val slide = if (motion.moves) slideInHorizontally(motion.tween(260)) { dir * it / SLIDE_FRACTION } else EnterTransition.None
                (fadeIn(motion.tween(220, delay = 90)) + slide) togetherWith fadeOut(motion.tween(90))
            },
            label = "tv-tidal-tab",
            modifier = Modifier.weight(1f),
        ) { shown -> CollectionBody(vm, shown, entryKey, songsState, grids.getOrPut(shown) { LazyGridState() }, restoreKey, restoreNonce, bodyEntry) }
    }
}

private fun Collection.order(): Int = when (this) {
    Collection.TRACKS -> 0
    Collection.PLAYLISTS -> 1
    Collection.ALBUMS -> 2
    Collection.ARTISTS -> 3
}

private fun Collection.label(): Int = when (this) {
    Collection.PLAYLISTS -> R.string.tidal_my_playlists
    Collection.TRACKS -> R.string.tidal_favorite_tracks
    Collection.ALBUMS -> R.string.tidal_favorite_albums
    Collection.ARTISTS -> R.string.tidal_favorite_artists
}

@Composable
internal fun CollectionBody(
    vm: TidalViewModel,
    which: Collection,
    entryKey: Int,
    songs: androidx.compose.foundation.lazy.LazyListState,
    grid: LazyGridState,
    restoreKey: Any?,
    restoreNonce: Int,
    bodyEntry: androidx.compose.ui.focus.FocusRequester,
) {
    val collection by vm.collection.collectAsStateWithLifecycle()
    val key = ListKey.Of(which)
    when (val load = collection[which]) {
        null, Load.Loading -> TvSkeleton(round = which == Collection.ARTISTS)
        is Load.Failed -> TvErrorState(load.message, { vm.loadCollection(which, force = true) }, entryKey)
        is Load.Ready -> {
            val list = load.value
            if (list.items.isEmpty() && list.complete) {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    Text(stringResource(R.string.tidal_collection_empty), style = TvType.section, color = LocalTvLook.current.ink2)
                }
            } else if (which == Collection.TRACKS) {
                TidalTrackList(vm, list, key, songs, restoreKey, restoreNonce, bodyEntry)
            } else {
                TidalCoverGrid(vm, list, key, grid, which == Collection.ARTISTS, restoreKey, restoreNonce, bodyEntry)
            }
        }
    }
}

@Composable
private fun TidalTrackList(
    vm: TidalViewModel,
    list: PagedHits,
    key: ListKey,
    state: androidx.compose.foundation.lazy.LazyListState,
    restoreKey: Any?,
    restoreNonce: Int,
    bodyEntry: androidx.compose.ui.focus.FocusRequester,
) {
    val ctx = rememberRowContext(vm)
    LoadMoreRows(state, list) { vm.loadMore(key) }
    LazyColumn(
        state = state,
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(start = 12.dp, end = 24.dp, top = 12.dp, bottom = 28.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        hitRows(vm, ctx, list.items, key, "fav", restoreKey, restoreNonce, edge = true, firstEntry = bodyEntry)
        pagingFooter(list) { vm.loadMore(key) }
    }
}

@Composable
private fun TidalCoverGrid(
    vm: TidalViewModel,
    list: PagedHits,
    key: ListKey,
    grid: LazyGridState,
    round: Boolean,
    restoreKey: Any?,
    restoreNonce: Int,
    bodyEntry: androidx.compose.ui.focus.FocusRequester,
) {
    LoadMoreGrid(grid, list) { vm.loadMore(key) }
    var menu by remember { mutableStateOf<SearchHit?>(null) }
    TvCoverGrid(list.items, key = { "${it.kind}-${it.id}" }, state = grid, restoreKey = restoreKey, restoreNonce = restoreNonce, firstItem = bodyEntry) { hit, size, mod ->
        TvHitCard(hit, size, mod, onClick = { vm.open(hit) }, onLongClick = { menu = hit })
    }
    menu?.let { hit -> TvActionMenu(hit.title, hit.subtitle, tidalHitActions(hit, vm)) { menu = null } }
}

/** A cover card of an album, artist or playlist result: open on OK, the menu on a long OK. */
@Composable
fun TvHitCard(hit: SearchHit, size: androidx.compose.ui.unit.Dp, modifier: Modifier = Modifier, onClick: () -> Unit, onLongClick: () -> Unit) {
    TvCoverCard(
        cover = hit.coverUrl,
        title = hit.title,
        subtitle = hit.subtitle.ifBlank { hit.artist },
        icon = hit.kind.icon(),
        round = hit.kind == ItemKind.ARTIST,
        size = size,
        modifier = modifier,
        onClick = onClick,
        onLongClick = onLongClick,
    )
}

/** Song rows of a TIDAL list: OK plays the list from that song (the phone's rule), the playing one is teal, saved ones say so. */
private fun LazyListScope.hitRows(
    vm: TidalViewModel,
    ctx: RowContext,
    hits: List<SearchHit>,
    key: ListKey,
    prefix: String,
    restoreKey: Any?,
    restoreNonce: Int,
    edge: Boolean,
    firstEntry: androidx.compose.ui.focus.FocusRequester? = null,
    leftTo: androidx.compose.ui.focus.FocusRequester? = null,
) {
    itemsIndexed(hits, key = { i, h -> "$prefix-$i-${h.kind}-${h.id}" }) { i, hit ->
        TvHitRow(vm, ctx, hit, hits, i, key, restoreKey, restoreNonce, edge, entry = firstEntry.takeIf { i == 0 }, leftTo = leftTo)
    }
}

@Composable
fun TvHitRow(
    vm: TidalViewModel,
    ctx: RowContext,
    hit: SearchHit,
    list: List<SearchHit>,
    index: Int,
    key: ListKey?,
    restoreKey: Any? = null,
    restoreNonce: Int = 0,
    edge: Boolean = false,
    onOpened: () -> Unit = {},
    /** The first row of a list is where the header above (or the pills beside) send the focus. */
    entry: androidx.compose.ui.focus.FocusRequester? = null,
    /** LEFT from a row goes here (the pills beside the list). */
    leftTo: androidx.compose.ui.focus.FocusRequester? = null,
) {
    val app = LocalContext.current.homefi
    val nav = LocalTvNav.current
    val isTrack = hit.kind == ItemKind.TRACK
    val playing = isTrack && hit.id == ctx.playingId
    val onDevice = isTrack && localKey(hit.artist, hit.title) in ctx.local
    var menu by remember { mutableStateOf(false) }
    TvRow(
        title = hit.title,
        subtitle = hit.subtitle,
        cover = hit.coverUrl,
        coverIcon = hit.kind.icon(),
        round = hit.kind == ItemKind.ARTIST,
        playing = playing,
        tag = if (onDevice) stringResource(R.string.tv_tidal_saved) else hit.quality?.takeIf { isTrack },
        trailing = hit.durationMs?.takeIf { isTrack && it > 0uL }?.let { formatDuration(it.toLong()) },
        onClick = {
            onOpened()
            when {
                playing -> app.player.togglePlayPause()
                isTrack && key != null -> { vm.play(key, index); nav.openPlayer() }
                isTrack -> { app.player.playTidal(list, index); nav.openPlayer() }
                else -> vm.open(hit)
            }
        },
        onLongClick = { menu = true },
        modifier = (if (entry != null) Modifier.focusRequester(entry) else Modifier)
            .then(if (edge) Modifier.tvLeftToDrawer() else Modifier)
            .tvLeftTo(leftTo)
            .tvRestoreFocus(restoreKey == hit.id, restoreNonce),
    )
    if (menu) TvActionMenu(hit.title, hit.subtitle, tidalHitActions(hit, vm)) { menu = false }
}

/** Under a list that still has pages: a small spinner, or why the next page failed with a retry. */
private fun LazyListScope.pagingFooter(list: PagedHits, onRetry: () -> Unit) {
    if (list.complete) return
    item(key = "paging-footer") {
        Box(Modifier.fillMaxWidth().padding(12.dp), contentAlignment = Alignment.Center) {
            val error = list.moreError
            if (error == null) {
                CircularProgressIndicator(Modifier.size(22.dp), color = LocalTvLook.current.accent2, strokeWidth = 2.dp)
            } else {
                TvPill(stringResource(R.string.tv_retry), onRetry, height = 30.dp)
            }
        }
    }
}

/** Asks for the next page when the last visible row gets within [LOAD_AHEAD] rows of the end. */
@Composable
private fun LoadMoreRows(state: androidx.compose.foundation.lazy.LazyListState, list: PagedHits, onLoadMore: () -> Unit) {
    val current by rememberUpdatedState(list)
    val load by rememberUpdatedState(onLoadMore)
    LaunchedEffect(state) {
        snapshotFlow {
            val info = state.layoutInfo
            val last = info.visibleItemsInfo.lastOrNull()?.index ?: -1
            val l = current
            !l.complete && !l.loadingMore && l.moreError == null && info.totalItemsCount > 0 && last >= info.totalItemsCount - 1 - LOAD_AHEAD
        }.distinctUntilChanged().filter { it }.collect { load() }
    }
}

@Composable
private fun LoadMoreGrid(state: LazyGridState, list: PagedHits, onLoadMore: () -> Unit) {
    val current by rememberUpdatedState(list)
    val load by rememberUpdatedState(onLoadMore)
    LaunchedEffect(state) {
        snapshotFlow {
            val info = state.layoutInfo
            val last = info.visibleItemsInfo.lastOrNull()?.index ?: -1
            val l = current
            !l.complete && !l.loadingMore && l.moreError == null && info.totalItemsCount > 0 && last >= info.totalItemsCount - 1 - LOAD_AHEAD
        }.distinctUntilChanged().filter { it }.collect { load() }
    }
}

// ------------------------------------------------------------------ pages

/** An album, playlist or artist page of TIDAL, drawn like the library's: pills on the left, songs on the right. */
@Composable
fun TvTidalRoutePage(route: TidalRoute, vm: TidalViewModel, entryKey: Int) {
    val pages by vm.pages.collectAsStateWithLifecycle()
    val ctx = rememberRowContext(vm)
    val app = LocalContext.current.homefi
    val nav = LocalTvNav.current
    val key = ListKey.Page(route)
    var queueMenu by remember { mutableStateOf(false) }
    val first = remember { androidx.compose.ui.focus.FocusRequester() }
    val rowsEntry = remember { androidx.compose.ui.focus.FocusRequester() }
    when (val load = pages[route]) {
        null, Load.Loading -> TvSkeleton(Modifier.padding(top = 60.dp))
        is Load.Failed -> TvErrorState(load.message, { vm.load(route) }, entryKey)
        is Load.Ready -> when (route) {
            is TidalRoute.Album -> {
                val page = (load.album() as Load.Ready).value
                val album = page.info
                val state = rememberLazyListState()
                LoadMoreRows(state, page.tracks) { vm.loadMore(key) }
                TvDetailPage(
                    title = album.title,
                    subtitle = listOfNotNull(album.artist, album.year?.toString()).joinToString(" · "),
                    cover = album.coverUrl,
                    coverIcon = Icons.Rounded.Album,
                    tags = listOfNotNull(album.quality),
                    pills = {
                        PagePills(first, rowsEntry, entryKey, onPlay = { shuffle -> vm.play(key, shuffle = shuffle); nav.openPlayer() }, onQueue = { queueMenu = true }, onDownload = { vm.download(album.title, "album/${album.id}") })
                    },
                ) { PageRows(vm, ctx, page.tracks, key, state, rowsEntry, first) }
                if (queueMenu) TvActionMenu(album.title, null, queueActions { next -> vm.queue(key, next) }) { queueMenu = false }
            }
            is TidalRoute.Playlist -> {
                val page = (load.playlist() as Load.Ready).value
                val playlist = page.info
                val state = rememberLazyListState()
                LoadMoreRows(state, page.tracks) { vm.loadMore(key) }
                TvDetailPage(
                    title = playlist.title,
                    subtitle = playlist.description.orEmpty().ifBlank { stringResource(R.string.library_count, playlist.numberOfTracks.toInt()) },
                    cover = playlist.coverUrl,
                    coverIcon = Icons.AutoMirrored.Rounded.QueueMusic,
                    pills = {
                        PagePills(first, rowsEntry, entryKey, onPlay = { shuffle -> vm.play(key, shuffle = shuffle); nav.openPlayer() }, onQueue = { queueMenu = true }, onDownload = { vm.download(playlist.title, "playlist/${playlist.uuid}") })
                    },
                ) { PageRows(vm, ctx, page.tracks, key, state, rowsEntry, first) }
                if (queueMenu) TvActionMenu(playlist.title, null, queueActions { next -> vm.queue(key, next) }) { queueMenu = false }
            }
            is TidalRoute.Artist -> {
                val artist = (load.artist() as Load.Ready).value
                val source = Sources.artist(artist.id, artist.name)
                TvDetailPage(
                    title = artist.name,
                    subtitle = stringResource(R.string.tidal_top_tracks),
                    cover = artist.pictureUrl,
                    coverIcon = Icons.Rounded.Person,
                    round = true,
                    pills = {
                        PagePills(first, rowsEntry, entryKey, onPlay = { shuffle -> app.player.playTidal(artist.topTracks, shuffle = shuffle, source = source); nav.openPlayer() }, onQueue = { queueMenu = true }, onDownload = null)
                    },
                ) {
                    LazyColumn(
                        Modifier.fillMaxSize(),
                        contentPadding = PaddingValues(start = 6.dp, end = 24.dp, top = 12.dp, bottom = 28.dp),
                        verticalArrangement = Arrangement.spacedBy(4.dp),
                    ) {
                        hitRows(vm, ctx, artist.topTracks, ListKey.Page(route), "top", null, 0, edge = false, firstEntry = rowsEntry, leftTo = first)
                        shelf(vm, R.string.search_albums, artist.albums)
                        shelf(vm, R.string.tidal_eps_singles, artist.epsAndSingles)
                    }
                }
                if (queueMenu) TvActionMenu(artist.name, null, queueActions { next -> app.player.queueTidal(artist.topTracks, next) }) { queueMenu = false }
            }
        }
    }
}

@Composable
private fun PagePills(first: androidx.compose.ui.focus.FocusRequester, rowsEntry: androidx.compose.ui.focus.FocusRequester, entryKey: Int, onPlay: (shuffle: Boolean) -> Unit, onQueue: () -> Unit, onDownload: (() -> Unit)?) {
    TvPill(stringResource(R.string.library_play_all), { onPlay(false) }, Modifier.tvInitialFocus(first, entryKey, PAGE_FOCUS_DELAY_MS).tvLeftToDrawer().tvRightTo(rowsEntry).width(DETAIL_PILL_WIDTH), icon = Icons.Rounded.PlayArrow, primary = true)
    TvPill(stringResource(R.string.library_shuffle), { onPlay(true) }, Modifier.tvLeftToDrawer().tvRightTo(rowsEntry).width(DETAIL_PILL_WIDTH), icon = Icons.Rounded.Shuffle)
    TvPill(stringResource(R.string.queue_add), onQueue, Modifier.tvLeftToDrawer().tvRightTo(rowsEntry).width(DETAIL_PILL_WIDTH), icon = Icons.AutoMirrored.Rounded.QueueMusic)
    if (onDownload != null) TvPill(stringResource(R.string.download), onDownload, Modifier.tvLeftToDrawer().tvRightTo(rowsEntry).width(DETAIL_PILL_WIDTH), icon = Icons.Rounded.Download)
}

@Composable
private fun PageRows(vm: TidalViewModel, ctx: RowContext, tracks: PagedHits, key: ListKey, state: androidx.compose.foundation.lazy.LazyListState, rowsEntry: androidx.compose.ui.focus.FocusRequester, first: androidx.compose.ui.focus.FocusRequester) {
    LazyColumn(
        state = state,
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(start = 6.dp, end = 24.dp, top = 12.dp, bottom = 28.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        hitRows(vm, ctx, tracks.items, key, "page", null, 0, edge = false, firstEntry = rowsEntry, leftTo = first)
        pagingFooter(tracks) { vm.loadMore(key) }
    }
}

/** A titled row of cover cards (an artist's albums, EPs). */
private fun LazyListScope.shelf(vm: TidalViewModel, title: Int, hits: List<SearchHit>) {
    if (hits.isEmpty()) return
    item(key = "shelf-$title") {
        var menu by remember { mutableStateOf<SearchHit?>(null) }
        Column(Modifier.padding(top = 10.dp)) {
            Text(stringResource(title), style = TvType.section, color = LocalTvLook.current.ink, modifier = Modifier.padding(start = 4.dp, bottom = 4.dp))
            LazyRow(contentPadding = PaddingValues(start = 6.dp, end = 18.dp, top = 12.dp, bottom = 18.dp), horizontalArrangement = Arrangement.spacedBy(TvDimens.gap)) {
                itemsIndexed(hits, key = { i, h -> "$i-${h.id}" }) { _, hit ->
                    TvHitCard(hit, SMALL_CARD, onClick = { vm.open(hit) }, onLongClick = { menu = hit })
                }
            }
        }
        menu?.let { hit -> TvActionMenu(hit.title, hit.subtitle, tidalHitActions(hit, vm)) { menu = null } }
    }
}
