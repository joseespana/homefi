package com.perseusoft.homefi.tv

import android.widget.Toast
import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.grid.LazyGridState
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.automirrored.rounded.Sort
import androidx.compose.material.icons.rounded.Album
import androidx.compose.material.icons.rounded.Clear
import androidx.compose.material.icons.rounded.FilterList
import androidx.compose.material.icons.rounded.LibraryMusic
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Shuffle
import androidx.compose.runtime.Composable
import androidx.compose.ui.focus.focusRequester
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.LibraryTrack
import com.perseusoft.homefi.core.LocalPlaylist
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.library.Album
import com.perseusoft.homefi.library.Artist
import com.perseusoft.homefi.library.EmbeddedCover
import com.perseusoft.homefi.library.albums
import com.perseusoft.homefi.library.artists
import com.perseusoft.homefi.library.filterQuery
import com.perseusoft.homefi.player.Sources
import com.perseusoft.homefi.ui.formatDuration
import com.perseusoft.homefi.ui.theme.LocalMotion

/** What the library shows: its views, or an album / artist / playlist opened from them. */
private sealed interface LibraryPage {
    data object Root : LibraryPage
    data class AlbumPage(val key: String) : LibraryPage
    data class ArtistPage(val name: String) : LibraryPage
    data class PlaylistPage(val path: String) : LibraryPage
}

private const val SLIDE_FRACTION = 12

/**
 * The library (docs/TV_UI.md 6.2): Songs / Albums / Artists / Playlists as pills, a grid of covers or a
 * list of rows under them, sort and filter on the right, album / artist / playlist pages that slide
 * over it. Reads the phone's `app.library` state; long press of OK opens the row's menu.
 */
@Composable
fun TvLibrary(entryKey: Int) {
    val app = LocalContext.current.homefi
    val library by app.library.state.collectAsStateWithLifecycle()
    var query by rememberSaveable { mutableStateOf("") }
    var tab by rememberSaveable { mutableStateOf(TvLibraryTab.SONGS) }
    var sort by rememberSaveable { mutableStateOf(TvSort.TITLE) }
    var albumKey by rememberSaveable { mutableStateOf<String?>(null) }
    var artistName by rememberSaveable { mutableStateOf<String?>(null) }
    var playlistPath by rememberSaveable { mutableStateOf<String?>(null) }
    // asks the item that was open to take the focus back when its page closes
    var restoreNonce by remember { mutableIntStateOf(0) }
    var restoreKey by remember { mutableStateOf<Any?>(null) }

    val tracks = remember(library.tracks, query) { library.tracks.filterQuery(query) }
    val albums = remember(tracks) { tracks.albums() }
    val artists = remember(tracks) { tracks.artists() }

    val page: LibraryPage = when {
        albumKey != null && albums.any { it.key == albumKey } -> LibraryPage.AlbumPage(albumKey!!)
        artistName != null && artists.any { it.name == artistName } -> LibraryPage.ArtistPage(artistName!!)
        playlistPath != null && library.playlists.any { it.path == playlistPath } -> LibraryPage.PlaylistPage(playlistPath!!)
        else -> LibraryPage.Root
    }
    val close = {
        restoreKey = albumKey ?: artistName ?: playlistPath
        restoreNonce++
        albumKey = null
        artistName = null
        playlistPath = null
    }
    BackHandler(enabled = page != LibraryPage.Root, onBack = close)

    // the lists keep their place across a page: hoisted above the page switch
    val songsState = rememberLazyListState()
    val albumsGrid = rememberLazyGridState()
    val artistsGrid = rememberLazyGridState()
    val playlistsGrid = rememberLazyGridState()

    val motion = LocalMotion.current
    AnimatedContent(
        targetState = page,
        transitionSpec = {
            val deeper = targetState != LibraryPage.Root
            val slide = if (motion.moves) slideInHorizontally(motion.tween(260)) { if (deeper) it / SLIDE_FRACTION else -it / SLIDE_FRACTION } else EnterTransition.None
            (fadeIn(motion.tween(220, delay = 90)) + slide) togetherWith (fadeOut(motion.tween(90)) + ExitTransition.None)
        },
        label = "tv-library-page",
    ) { shown ->
        when (shown) {
            is LibraryPage.AlbumPage -> albums.firstOrNull { it.key == shown.key }?.let { album ->
                TvTrackPage(
                    title = album.title.ifBlank { stringResource(R.string.unknown_album) },
                    subtitle = listOfNotNull(album.artist.ifBlank { null }, album.year?.toString()).joinToString(" · "),
                    cover = album.cover,
                    coverIcon = Icons.Rounded.Album,
                    tags = listOfNotNull(album.tracks.firstOrNull()?.format),
                    tracks = album.tracks,
                    source = Sources.album("", album.title),
                    numbered = true,
                )
            }
            is LibraryPage.ArtistPage -> artists.firstOrNull { it.name == shown.name }?.let { artist ->
                TvTrackPage(
                    title = artist.name.ifBlank { stringResource(R.string.unknown_artist) },
                    subtitle = stringResource(R.string.library_count, artist.tracks.size),
                    cover = artist.tracks.firstOrNull()?.let { EmbeddedCover(it.path) },
                    coverIcon = Icons.Rounded.Person,
                    round = true,
                    tracks = artist.tracks.ordered(TvSort.ALBUM),
                    source = Sources.artist("", artist.name),
                )
            }
            is LibraryPage.PlaylistPage -> library.playlists.firstOrNull { it.path == shown.path }?.let { playlist ->
                // entries resolve to scanned files; ones that are not on the device are counted, not shown
                val found = remember(playlist, library.tracks) { playlist.tracks.mapNotNull(app.library::find) }
                val missing = playlist.tracks.size - found.size
                TvTrackPage(
                    title = playlist.name,
                    subtitle = stringResource(R.string.library_count, found.size) +
                        if (missing > 0) " · " + stringResource(R.string.library_playlist_missing, missing) else "",
                    cover = found.firstOrNull()?.let { EmbeddedCover(it.path) },
                    coverIcon = Icons.AutoMirrored.Rounded.QueueMusic,
                    tracks = found,
                    source = Sources.playlist("", playlist.name),
                )
            }
            LibraryPage.Root -> LibraryRoot(
                entryKey = entryKey,
                tab = tab,
                onTab = { tab = it; sort = it.sortOrDefault(sort) },
                sort = sort,
                onSort = { sort = tab.nextSort(sort) },
                query = query,
                onQuery = { query = it },
                tracks = tracks,
                albums = albums,
                artists = artists,
                scanned = library.scanned,
                scanning = library.scanning,
                lists = RootLists(songsState, albumsGrid, artistsGrid, playlistsGrid),
                restoreKey = restoreKey,
                restoreNonce = restoreNonce,
                onAlbum = { albumKey = it.key; restoreKey = it.key },
                onArtist = { artistName = it.name; restoreKey = it.name },
                onPlaylist = { playlistPath = it.path; restoreKey = it.path },
            )
        }
    }
}

private class RootLists(val songs: LazyListState, val albums: LazyGridState, val artists: LazyGridState, val playlists: LazyGridState)

@Composable
private fun LibraryRoot(
    entryKey: Int,
    tab: TvLibraryTab,
    onTab: (TvLibraryTab) -> Unit,
    sort: TvSort,
    onSort: () -> Unit,
    query: String,
    onQuery: (String) -> Unit,
    tracks: List<LibraryTrack>,
    albums: List<Album>,
    artists: List<Artist>,
    scanned: Boolean,
    scanning: Boolean,
    lists: RootLists,
    restoreKey: Any?,
    restoreNonce: Int,
    onAlbum: (Album) -> Unit,
    onArtist: (Artist) -> Unit,
    onPlaylist: (LocalPlaylist) -> Unit,
) {
    val app = LocalContext.current.homefi
    val library by app.library.state.collectAsStateWithLifecycle()
    var filtering by remember { mutableStateOf(false) }
    val safe = tvSafePadding()
    val sorts = tab.sorts
    val subtitle = stringResource(R.string.tv_lib_summary, library.tracks.size, library.tracks.albums().size)
    val tabs = TvLibraryTab.entries
    val bodyEntry = remember { androidx.compose.ui.focus.FocusRequester() }

    Column(Modifier.fillMaxSize().padding(top = safe.calculateTopPadding(), end = safe.calculateRightPadding(androidx.compose.ui.unit.LayoutDirection.Ltr))) {
        TvScreenHeader(stringResource(R.string.tv_nav_library), subtitle, Modifier.padding(start = 12.dp)) { TvNowPlayingChip() }
        Row(Modifier.fillMaxWidth().padding(start = 12.dp, top = 10.dp), verticalAlignment = Alignment.CenterVertically) {
            TvTabs(
                labels = tabs.map { stringResource(it.label) },
                selected = tab.ordinal,
                onSelect = { onTab(tabs[it]) },
                entryKey = entryKey,
                downTo = bodyEntry,
            )
            Row(Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End), verticalAlignment = Alignment.CenterVertically) {
                if (sorts.isNotEmpty()) {
                    TvPill(stringResource(sort.label), onSort, Modifier.tvDownTo(bodyEntry), icon = Icons.AutoMirrored.Rounded.Sort, height = 30.dp, labelStyle = TvType.body.copy(fontSize = androidx.compose.ui.unit.TextUnit(14f, androidx.compose.ui.unit.TextUnitType.Sp)))
                }
                TvPill(
                    query.ifBlank { stringResource(R.string.tv_lib_filter) },
                    { filtering = true },
                    icon = if (query.isBlank()) Icons.Rounded.FilterList else Icons.Rounded.Clear,
                    selected = query.isNotBlank(),
                    height = 30.dp,
                    labelStyle = TvType.body.copy(fontSize = androidx.compose.ui.unit.TextUnit(14f, androidx.compose.ui.unit.TextUnitType.Sp)),
                    modifier = Modifier.width(FILTER_WIDTH).tvDownTo(bodyEntry),
                )
            }
        }
        val body = when {
            !scanned -> LibraryBody.LOADING
            library.tracks.isEmpty() -> LibraryBody.EMPTY
            else -> LibraryBody.TAB
        }
        val motion = LocalMotion.current
        AnimatedContent(
            targetState = if (body == LibraryBody.TAB) tab.name else body.name,
            transitionSpec = {
                val m = motion
                val dir = if (targetState > initialState) 1 else -1
                val slide = if (m.moves) slideInHorizontally(m.tween(260)) { dir * it / SLIDE_FRACTION } else EnterTransition.None
                (fadeIn(m.tween(220, delay = 90)) + slide) togetherWith fadeOut(m.tween(90))
            },
            label = "tv-library-body",
            modifier = Modifier.weight(1f),
        ) { shown ->
            when (shown) {
                LibraryBody.LOADING.name -> TvSkeleton()
                LibraryBody.EMPTY.name -> TvLibraryEmpty(entryKey)
                else -> when (TvLibraryTab.valueOf(shown)) {
                    TvLibraryTab.SONGS -> SongsList(tracks, sort, lists.songs, restoreKey, restoreNonce, query, bodyEntry)
                    TvLibraryTab.ALBUMS -> AlbumsGrid(albums.ordered(sort), lists.albums, restoreKey, restoreNonce, onAlbum, query, bodyEntry)
                    TvLibraryTab.ARTISTS -> ArtistsGrid(artists.ordered(sort), lists.artists, restoreKey, restoreNonce, onArtist, query, bodyEntry)
                    TvLibraryTab.PLAYLISTS -> PlaylistsGrid(library.playlists, lists.playlists, restoreKey, restoreNonce, onPlaylist, query, bodyEntry)
                }
            }
        }
    }
    if (filtering) {
        TvTextPrompt(
            title = stringResource(R.string.tv_lib_filter_title),
            initial = query,
            onDone = { onQuery(it); filtering = false },
            onDismiss = { filtering = false },
        )
    }
    // scanning shows nowhere else: a tiny hint while the first scan runs is the skeleton above
    if (scanning && scanned) Unit
}

private val FILTER_WIDTH = 170.dp

private enum class LibraryBody { LOADING, EMPTY, TAB }

@Composable
private fun TvLibraryEmpty(entryKey: Int) {
    val nav = LocalTvNav.current
    androidx.compose.foundation.layout.Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        TvEmptyState(
            title = stringResource(R.string.tv_empty_title),
            body = stringResource(R.string.tv_empty_body),
            cta = stringResource(R.string.tv_lib_empty_cta),
            ctaIcon = Icons.Rounded.LibraryMusic,
            icon = Icons.Rounded.LibraryMusic,
            entryKey = entryKey,
            onCta = { nav.go(TvSection.DOWNLOADS) },
        )
    }
}

/** Nothing matches the filter. */
@Composable
private fun NothingFor(query: String) {
    androidx.compose.foundation.layout.Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        androidx.compose.material3.Text(
            stringResource(R.string.tv_search_empty, query),
            style = TvType.section,
            color = LocalTvLook.current.ink2,
        )
    }
}

@Composable
private fun SongsList(
    tracks: List<LibraryTrack>,
    sort: TvSort,
    state: LazyListState,
    restoreKey: Any?,
    restoreNonce: Int,
    query: String,
    firstRow: androidx.compose.ui.focus.FocusRequester,
) {
    val app = LocalContext.current.homefi
    val nav = LocalTvNav.current
    val context = LocalContext.current
    val library by app.library.state.collectAsStateWithLifecycle()
    val playing by app.player.state.collectAsStateWithLifecycle()
    val currentId = playing.current?.mediaId
    val sorted = remember(tracks, sort) { tracks.ordered(sort) }
    var menu by remember { mutableStateOf<LibraryTrack?>(null) }
    if (sorted.isEmpty()) return NothingFor(query)
    // the row that was focused before a page keeps its focus when the list returns
    val unavailable = stringResource(R.string.tv_lib_unavailable)
    LazyColumn(
        state = state,
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(start = 12.dp, end = 24.dp, top = 12.dp, bottom = 28.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        itemsIndexed(sorted, key = { _, t -> t.path }) { i, track ->
            val gone = library.unavailable(track.path)
            TvRow(
                title = track.title,
                subtitle = listOf(track.artist, track.album).filter(String::isNotBlank).joinToString(" · ").ifBlank { stringResource(R.string.unknown_artist) },
                cover = EmbeddedCover(track.path),
                playing = track.path == currentId,
                dimmed = gone,
                tag = if (gone) unavailable else track.format,
                trailing = if (track.durationMs > 0uL) formatDuration(track.durationMs.toLong()) else null,
                onClick = {
                    when {
                        gone -> Toast.makeText(context, unavailable, Toast.LENGTH_SHORT).show()
                        track.path == currentId -> app.player.togglePlayPause()
                        else -> {
                            app.player.play(sorted.filterNot { library.unavailable(it.path) }, sorted.filterNot { library.unavailable(it.path) }.indexOf(track).coerceAtLeast(0), source = Sources.library())
                            nav.openPlayer()
                        }
                    }
                },
                onLongClick = { menu = track },
                modifier = (if (i == 0) Modifier.focusRequester(firstRow) else Modifier).tvLeftToDrawer().tvRestoreFocus(restoreKey == track.path, restoreNonce),
            )
        }
    }
    menu?.let { track ->
        TvActionMenu(track.title, track.artist, libraryTrackActions(track)) { menu = null }
    }
}

@Composable
private fun AlbumsGrid(albums: List<Album>, state: LazyGridState, restoreKey: Any?, restoreNonce: Int, onOpen: (Album) -> Unit, query: String, firstItem: androidx.compose.ui.focus.FocusRequester) {
    val app = LocalContext.current.homefi
    val nav = LocalTvNav.current
    if (albums.isEmpty()) return NothingFor(query)
    var menu by remember { mutableStateOf<Album?>(null) }
    TvCoverGrid(albums, key = { it.key }, state = state, restoreKey = restoreKey, restoreNonce = restoreNonce, firstItem = firstItem) { album, size, mod ->
        TvCoverCard(
            cover = album.cover,
            title = album.title.ifBlank { stringResource(R.string.unknown_album) },
            subtitle = album.artist,
            icon = Icons.Rounded.Album,
            size = size,
            modifier = mod,
            onClick = { onOpen(album) },
            onLongClick = { menu = album },
        )
    }
    menu?.let { album ->
        val actions = listOf(
            TvAction(Icons.Rounded.PlayArrow, stringResource(R.string.player_play)) { app.player.play(album.tracks, source = Sources.album("", album.title)); nav.openPlayer() },
            TvAction(Icons.Rounded.Shuffle, stringResource(R.string.library_shuffle)) { app.player.play(album.tracks, shuffle = true, source = Sources.album("", album.title)); nav.openPlayer() },
        ) + queueActions { next -> app.player.queueTracks(album.tracks, next) }
        TvActionMenu(album.title, album.artist, actions) { menu = null }
    }
}

@Composable
private fun ArtistsGrid(artists: List<Artist>, state: LazyGridState, restoreKey: Any?, restoreNonce: Int, onOpen: (Artist) -> Unit, query: String, firstItem: androidx.compose.ui.focus.FocusRequester) {
    val app = LocalContext.current.homefi
    val nav = LocalTvNav.current
    if (artists.isEmpty()) return NothingFor(query)
    var menu by remember { mutableStateOf<Artist?>(null) }
    TvCoverGrid(artists, key = { it.name }, state = state, restoreKey = restoreKey, restoreNonce = restoreNonce, firstItem = firstItem) { artist, size, mod ->
        TvCoverCard(
            cover = artist.tracks.firstOrNull()?.let { EmbeddedCover(it.path) },
            title = artist.name.ifBlank { stringResource(R.string.unknown_artist) },
            subtitle = stringResource(R.string.tv_lib_artist_sub, artist.albums, artist.tracks.size),
            icon = Icons.Rounded.Person,
            round = true,
            size = size,
            modifier = mod,
            onClick = { onOpen(artist) },
            onLongClick = { menu = artist },
        )
    }
    menu?.let { artist ->
        val actions = listOf(
            TvAction(Icons.Rounded.PlayArrow, stringResource(R.string.player_play)) { app.player.play(artist.tracks, source = Sources.artist("", artist.name)); nav.openPlayer() },
            TvAction(Icons.Rounded.Shuffle, stringResource(R.string.library_shuffle)) { app.player.play(artist.tracks, shuffle = true, source = Sources.artist("", artist.name)); nav.openPlayer() },
        ) + queueActions { next -> app.player.queueTracks(artist.tracks, next) }
        TvActionMenu(artist.name, null, actions) { menu = null }
    }
}

@Composable
private fun PlaylistsGrid(playlists: List<LocalPlaylist>, state: LazyGridState, restoreKey: Any?, restoreNonce: Int, onOpen: (LocalPlaylist) -> Unit, query: String, firstItem: androidx.compose.ui.focus.FocusRequester) {
    val app = LocalContext.current.homefi
    val nav = LocalTvNav.current
    val shown = remember(playlists, query) { if (query.isBlank()) playlists else playlists.filter { it.name.contains(query.trim(), ignoreCase = true) } }
    if (shown.isEmpty()) return if (playlists.isEmpty()) TvLibraryNoPlaylists() else NothingFor(query)
    var menu by remember { mutableStateOf<LocalPlaylist?>(null) }
    TvCoverGrid(shown, key = { it.path }, state = state, restoreKey = restoreKey, restoreNonce = restoreNonce, firstItem = firstItem) { playlist, size, mod ->
        TvCoverCard(
            cover = playlist.tracks.firstOrNull()?.let(::EmbeddedCover),
            title = playlist.name,
            subtitle = stringResource(R.string.library_count, playlist.tracks.size),
            icon = Icons.AutoMirrored.Rounded.QueueMusic,
            size = size,
            modifier = mod,
            onClick = { onOpen(playlist) },
            onLongClick = { menu = playlist },
        )
    }
    menu?.let { playlist ->
        val tracks = playlist.tracks.mapNotNull(app.library::find)
        val source = Sources.playlist("", playlist.name)
        val actions = listOf(
            TvAction(Icons.Rounded.PlayArrow, stringResource(R.string.player_play)) { app.player.play(tracks, source = source); nav.openPlayer() },
            TvAction(Icons.Rounded.Shuffle, stringResource(R.string.library_shuffle)) { app.player.play(tracks, shuffle = true, source = source); nav.openPlayer() },
        ) + queueActions { next -> app.player.queueTracks(tracks, next) }
        TvActionMenu(playlist.name, stringResource(R.string.library_count, tracks.size), actions) { menu = null }
    }
}

@Composable
private fun TvLibraryNoPlaylists() {
    androidx.compose.foundation.layout.Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        androidx.compose.material3.Text(stringResource(R.string.tv_lib_no_playlists), style = TvType.section, color = LocalTvLook.current.ink2)
    }
}

/**
 * An album / artist / playlist page: Play, Shuffle and Queue pills on the left, the songs on the
 * right (a song on an unplugged drive is dimmed). BACK is handled by the library.
 */
@Composable
private fun TvTrackPage(
    title: String,
    subtitle: String,
    cover: Any?,
    coverIcon: androidx.compose.ui.graphics.vector.ImageVector,
    tracks: List<LibraryTrack>,
    source: com.perseusoft.homefi.core.SourceContext,
    round: Boolean = false,
    tags: List<String> = emptyList(),
    numbered: Boolean = false,
) {
    val app = LocalContext.current.homefi
    val nav = LocalTvNav.current
    val context = LocalContext.current
    val library by app.library.state.collectAsStateWithLifecycle()
    val playing by app.player.state.collectAsStateWithLifecycle()
    val playable = remember(tracks, library.missingRoots) { tracks.filterNot { library.unavailable(it.path) } }
    val unavailable = stringResource(R.string.tv_lib_unavailable)
    var queueMenu by remember { mutableStateOf(false) }
    var rowMenu by remember { mutableStateOf<LibraryTrack?>(null) }
    val firstFocus = remember { androidx.compose.ui.focus.FocusRequester() }
    val rowsEntry = remember { androidx.compose.ui.focus.FocusRequester() }
    TvDetailPage(
        title = title,
        subtitle = subtitle,
        cover = cover,
        round = round,
        coverIcon = coverIcon,
        tags = tags,
        pills = {
            TvPill(stringResource(R.string.library_play_all), { app.player.play(playable, source = source); nav.openPlayer() }, Modifier.tvInitialFocus(firstFocus, afterMs = PAGE_FOCUS_DELAY_MS).tvLeftToDrawer().tvRightTo(rowsEntry).width(DETAIL_PILL_WIDTH), icon = Icons.Rounded.PlayArrow, primary = true)
            TvPill(stringResource(R.string.library_shuffle), { app.player.play(playable, shuffle = true, source = source); nav.openPlayer() }, Modifier.tvLeftToDrawer().tvRightTo(rowsEntry).width(DETAIL_PILL_WIDTH), icon = Icons.Rounded.Shuffle)
            TvPill(stringResource(R.string.queue_add), { queueMenu = true }, Modifier.tvLeftToDrawer().tvRightTo(rowsEntry).width(DETAIL_PILL_WIDTH), icon = Icons.AutoMirrored.Rounded.QueueMusic)
        },
    ) {
        LazyColumn(
            Modifier.fillMaxSize(),
            contentPadding = PaddingValues(start = 6.dp, end = 24.dp, top = 12.dp, bottom = 28.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            itemsIndexed(tracks, key = { _, t -> t.path }) { index, track ->
                val gone = library.unavailable(track.path)
                val current = track.path == playing.current?.mediaId
                TvRow(
                    title = if (numbered && track.trackNumber > 0u) "${track.trackNumber}.  ${track.title}" else track.title,
                    subtitle = if (numbered) track.artist.takeIf { it.isNotBlank() && !subtitle.startsWith(it) }.orEmpty() else listOf(track.artist, track.album).filter(String::isNotBlank).joinToString(" · "),
                    cover = if (numbered) null else EmbeddedCover(track.path),
                    playing = current,
                    dimmed = gone,
                    tag = if (gone) unavailable else null,
                    trailing = if (track.durationMs > 0uL) formatDuration(track.durationMs.toLong()) else null,
                    onClick = {
                        when {
                            gone -> Toast.makeText(context, unavailable, Toast.LENGTH_SHORT).show()
                            current -> app.player.togglePlayPause()
                            else -> {
                                app.player.play(playable, playable.indexOf(track).coerceAtLeast(0), source = source)
                                nav.openPlayer()
                            }
                        }
                    },
                    onLongClick = { rowMenu = track },
                    modifier = (if (index == 0) Modifier.focusRequester(rowsEntry) else Modifier).tvLeftTo(firstFocus),
                )
            }
        }
    }
    if (queueMenu) TvActionMenu(title, null, queueActions { next -> app.player.queueTracks(playable, next) }) { queueMenu = false }
    rowMenu?.let { track -> TvActionMenu(track.title, track.artist, libraryTrackActions(track)) { rowMenu = null } }
}
