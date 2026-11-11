package com.perseusoft.homefi.ui.library

import com.perseusoft.homefi.ui.playlists.songSeedAction
import com.perseusoft.homefi.ui.playlists.toSeed
import com.perseusoft.homefi.ui.haptics
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material.icons.rounded.Celebration
import androidx.compose.material.icons.rounded.Chat
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.graphics.Color
import androidx.compose.foundation.layout.heightIn
import com.perseusoft.homefi.core.PlaylistOrigin
import androidx.compose.runtime.LaunchedEffect
import com.perseusoft.homefi.core.HomePlaylist
import com.perseusoft.homefi.ui.playlists.ChatBadge
import com.perseusoft.homefi.ui.playlists.ChatPlaylistScreen
import com.perseusoft.homefi.ui.playlists.ConcertScreen
import com.perseusoft.homefi.ui.playlists.HomePlaylistScreen
import com.perseusoft.homefi.ui.playlists.NotRealSetlistBadge
import com.perseusoft.homefi.ui.playlists.PartyBadge
import com.perseusoft.homefi.ui.playlists.PartyScreen
import com.perseusoft.homefi.ui.playlists.PartyMode
import com.perseusoft.homefi.ui.playlists.ChatRequest
import androidx.compose.runtime.saveable.listSaver
import com.perseusoft.homefi.ui.playlists.concert
import com.perseusoft.homefi.ui.playlists.deleteWithUndo
import com.perseusoft.homefi.core.SetlistSource
import androidx.compose.foundation.combinedClickable
import androidx.compose.animation.AnimatedContent
import androidx.compose.foundation.lazy.grid.itemsIndexed
import com.perseusoft.homefi.ui.theme.ListEntrance
import com.perseusoft.homefi.ui.theme.LocalMotion
import com.perseusoft.homefi.ui.theme.itemMotion
import com.perseusoft.homefi.ui.QueueButton
import com.perseusoft.homefi.ui.QueueMenu
import com.perseusoft.homefi.ui.NowCoverMark
import com.perseusoft.homefi.ui.NowMark
import com.perseusoft.homefi.ui.SkeletonList
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import com.perseusoft.homefi.core.LocalPlaylist
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.Album
import androidx.compose.material.icons.rounded.Clear
import androidx.compose.material.icons.rounded.FolderOpen
import androidx.compose.material.icons.rounded.MusicNote
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material.icons.rounded.Shuffle
import com.perseusoft.homefi.ui.theme.Button
import androidx.compose.material3.ButtonDefaults
import com.perseusoft.homefi.ui.theme.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import com.perseusoft.homefi.ui.theme.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.PrimaryScrollableTabRow
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import coil3.compose.AsyncImage
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.LibraryTrack
import com.perseusoft.homefi.data.Storage
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.library.Album
import com.perseusoft.homefi.library.Artist
import com.perseusoft.homefi.library.EmbeddedCover
import com.perseusoft.homefi.library.albums
import com.perseusoft.homefi.library.artists
import com.perseusoft.homefi.library.filterQuery
import com.perseusoft.homefi.ui.HomefiLogo
import com.perseusoft.homefi.ui.QualityBadge
import com.perseusoft.homefi.ui.formatDuration
import com.perseusoft.homefi.ui.theme.LocalSkin
import com.perseusoft.homefi.ui.theme.currentTitleColor
import com.perseusoft.homefi.ui.theme.numeric
import com.perseusoft.homefi.ui.theme.skinRowColors
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.combinedClickable
import androidx.compose.material.icons.rounded.Tune
import com.perseusoft.homefi.core.SourceContext
import com.perseusoft.homefi.core.TasteScope
import com.perseusoft.homefi.player.Sources
import androidx.compose.material3.LocalContentColor
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.takeOrElse

private enum class LibraryTab(val label: Int) {
    SONGS(R.string.library_songs),
    ALBUMS(R.string.library_albums),
    ARTISTS(R.string.library_artists),
    PLAYLISTS(R.string.library_playlists),
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LibraryScreen() {
    val app = LocalContext.current.homefi
    val state by app.library.state.collectAsStateWithLifecycle()
    val playing by app.player.state.collectAsStateWithLifecycle()
    val currentId = playing.current?.mediaId

    var query by rememberSaveable { mutableStateOf("") }
    var tab by rememberSaveable { mutableStateOf(LibraryTab.SONGS) }
    var albumKey by rememberSaveable { mutableStateOf<String?>(null) }
    var artistName by rememberSaveable { mutableStateOf<String?>(null) }
    var playlistPath by rememberSaveable { mutableStateOf<String?>(null) }
    var concertOpen by rememberSaveable { mutableStateOf(false) }
    // the AI chat on screen (party answers or free chat), by its id in app.conversations; closed when null.
    // Leaving the screen does not close the chat: it goes on in the background (the chats menu lists it).
    var aiChat by rememberSaveable(stateSaver = AiChatSaver) { mutableStateOf<AiChat?>(null) }
    val openChats by app.conversations.list.collectAsStateWithLifecycle()
    val leaveChat = {
        aiChat?.let { app.conversations.leave(it.id) }
        aiChat = null
    }
    var homeListId by rememberSaveable { mutableStateOf<String?>(null) }
    val homeLists by app.playlists.list.collectAsStateWithLifecycle()
    val snackbar = remember { SnackbarHostState() }

    val permission = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { app.library.refresh() }

    // a song sent here from a menu opens a chat with it; a chat picked in the chats menu or a notice comes back
    LaunchedEffect(Unit) {
        app.chatRequests.request.collect { request ->
            if (request == null) return@collect
            app.chatRequests.consume()
            concertOpen = false
            when (request) {
                is ChatRequest.LikeSong -> {
                    leaveChat()
                    app.conversations.open(PartyMode.CHAT, seed = request.seed)?.let { aiChat = AiChat(it.id, PartyMode.CHAT) }
                }
                is ChatRequest.Resume -> app.conversations.get(request.id)?.let {
                    if (aiChat?.id != it.id) leaveChat()
                    aiChat = AiChat(it.id, it.mode)
                }
            }
        }
    }

    val tracks = remember(state.tracks, query) { state.tracks.filterQuery(query) }
    val albums = remember(tracks) { tracks.albums() }
    val artists = remember(tracks) { tracks.artists() }

    // an album, playlist or artist opens over the tabs: forward slides in, back slides out
    val motion = LocalMotion.current
    val page = when {
        concertOpen -> LibraryPage.Concert
        // after the process died the chat is gone: back to the list
        aiChat != null && openChats.any { it.id == aiChat!!.id } -> LibraryPage.Chat(aiChat!!)
        homeListId != null && homeLists.any { it.id == homeListId } -> LibraryPage.HomeList(homeListId!!)
        albumKey != null && albums.any { it.key == albumKey } -> LibraryPage.AlbumPage(albumKey!!)
        playlistPath != null && state.playlists.any { it.path == playlistPath } -> LibraryPage.PlaylistPage(playlistPath!!)
        artistName != null && artists.any { it.name == artistName } -> LibraryPage.ArtistPage(artistName!!)
        else -> LibraryPage.Root
    }
    BackHandler(enabled = page != LibraryPage.Root) {
        when (page) {
            LibraryPage.Concert -> concertOpen = false
            is LibraryPage.Chat -> leaveChat()
            is LibraryPage.HomeList -> homeListId = null
            is LibraryPage.AlbumPage -> albumKey = null
            is LibraryPage.PlaylistPage -> playlistPath = null
            is LibraryPage.ArtistPage -> artistName = null
            LibraryPage.Root -> Unit
        }
    }
    Box(Modifier.fillMaxSize()) {
    AnimatedContent(
        targetState = page,
        transitionSpec = { motion.screens(forward = targetState != LibraryPage.Root) },
        label = "library-page",
    ) { shown ->
        when (shown) {
            LibraryPage.Concert -> ConcertScreen(
                onBack = { concertOpen = false },
                onDelete = { deleteWithUndo(app, snackbar, it) },
            )
            is LibraryPage.Chat -> if (shown.chat.mode == PartyMode.PARTY) {
                PartyScreen(
                    conversationId = shown.chat.id,
                    onBack = leaveChat,
                    onDelete = { leaveChat(); deleteWithUndo(app, snackbar, it) },
                )
            } else {
                ChatPlaylistScreen(
                    conversationId = shown.chat.id,
                    onBack = leaveChat,
                    onDelete = { leaveChat(); deleteWithUndo(app, snackbar, it) },
                )
            }
            is LibraryPage.HomeList -> HomePlaylistScreen(
                id = shown.id,
                onBack = { homeListId = null },
                onDelete = { homeListId = null; deleteWithUndo(app, snackbar, it) },
                // a saved playlist goes back to the screen that made it
                onChangeAnswers = {
                    val mode = if (it.origin is PlaylistOrigin.Chat) PartyMode.CHAT else PartyMode.PARTY
                    app.conversations.open(mode, replace = it)?.let { chat -> aiChat = AiChat(chat.id, mode) }
                },
            )
            is LibraryPage.AlbumPage -> albums.firstOrNull { it.key == shown.key }?.let { album ->
                AlbumDetail(album, currentId, onBack = { albumKey = null })
            }
            is LibraryPage.PlaylistPage -> state.playlists.firstOrNull { it.path == shown.path }?.let { openPlaylist ->
                // entries resolve to scanned files; ones that aren't on the device are counted, not shown
                val found = remember(openPlaylist, state.tracks) { openPlaylist.tracks.mapNotNull(app.library::find) }
                TrackList(
                    tracks = found,
                    currentId = currentId,
                    source = Sources.playlist("", openPlaylist.name),
                    header = {
                        ScreenHeader(
                            openPlaylist.name,
                            onBack = { playlistPath = null },
                            onSound = { app.sound.openMenu(TasteScope.Playlist(id = "", name = openPlaylist.name)) },
                        )
                        val missing = openPlaylist.tracks.size - found.size
                        PlayHeader(
                            stringResource(R.string.library_count, found.size) +
                                if (missing > 0) " · " + stringResource(R.string.library_playlist_missing, missing) else "",
                            found,
                            Sources.playlist("", openPlaylist.name),
                        )
                    },
                )
            }
            is LibraryPage.ArtistPage -> artists.firstOrNull { it.name == shown.name }?.let { openArtist ->
                TrackList(
                    tracks = openArtist.tracks,
                    currentId = currentId,
                    source = Sources.artist("", openArtist.name),
                    header = {
                        ScreenHeader(
                            openArtist.name,
                            onBack = { artistName = null },
                            onSound = openArtist.name.takeIf { it.isNotBlank() }?.let { n -> { app.sound.openMenu(TasteScope.Artist(id = "", name = n)) } },
                        )
                    },
                )
            }
            LibraryPage.Root -> LibraryTabs(
                query = query,
                onQuery = { query = it },
                tab = tab,
                onTab = { tab = it },
                tracks = tracks,
                albums = albums,
                artists = artists,
                currentId = currentId,
                onGrant = { permission.launch(Storage.audioPermission) },
                onAlbum = { albumKey = it.key },
                onArtist = { artistName = it.name },
                onPlaylist = { playlistPath = it.path },
                onSound = app.sound::openMenu,
                onChat = { app.conversations.open(PartyMode.CHAT)?.let { aiChat = AiChat(it.id, PartyMode.CHAT) } },
                onConcert = { concertOpen = true },
                onParty = { app.conversations.open(PartyMode.PARTY)?.let { aiChat = AiChat(it.id, PartyMode.PARTY) } },
                onHomeList = { homeListId = it.id },
            )
        }
    }
    SnackbarHost(snackbar, Modifier.align(Alignment.BottomCenter).padding(bottom = 8.dp))
    }
}

/** What the library shows: its tabs, or an album / playlist / artist opened from them. */
/** The AI chat on screen: its id in `app.conversations` and which screen shows it. */
private data class AiChat(val id: String, val mode: PartyMode)

private val AiChatSaver = listSaver<AiChat?, String>(
    save = { chat -> listOfNotNull(chat?.id, chat?.mode?.name) },
    restore = { saved -> if (saved.size == 2) AiChat(saved[0], PartyMode.valueOf(saved[1])) else null },
)

private sealed interface LibraryPage {
    data object Root : LibraryPage
    data object Concert : LibraryPage
    data class Chat(val chat: AiChat) : LibraryPage
    data class HomeList(val id: String) : LibraryPage
    data class AlbumPage(val key: String) : LibraryPage
    data class PlaylistPage(val path: String) : LibraryPage
    data class ArtistPage(val name: String) : LibraryPage
}

/** What a tab's area holds; switching tabs slides along their order. */
private sealed interface TabBody {
    data object Loading : TabBody
    data object Empty : TabBody
    data class Showing(val tab: LibraryTab) : TabBody
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun LibraryTabs(
    query: String,
    onQuery: (String) -> Unit,
    tab: LibraryTab,
    onTab: (LibraryTab) -> Unit,
    tracks: List<LibraryTrack>,
    albums: List<Album>,
    artists: List<Artist>,
    currentId: String?,
    onGrant: () -> Unit,
    onAlbum: (Album) -> Unit,
    onArtist: (Artist) -> Unit,
    onPlaylist: (LocalPlaylist) -> Unit,
    onSound: (TasteScope) -> Unit,
    onChat: () -> Unit,
    onConcert: () -> Unit,
    onParty: () -> Unit,
    onHomeList: (HomePlaylist) -> Unit,
) {
    val app = LocalContext.current.homefi
    val state by app.library.state.collectAsStateWithLifecycle()
    val motion = LocalMotion.current
    // the playlists HomeFi made are read again whenever their tab shows
    LaunchedEffect(tab) { if (tab == LibraryTab.PLAYLISTS) app.playlists.refresh() }
    Column(Modifier.fillMaxSize()) {
        OutlinedTextField(
            value = query,
            onValueChange = onQuery,
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 8.dp),
            placeholder = { Text(stringResource(R.string.library_filter)) },
            leadingIcon = { Icon(Icons.Rounded.Search, null) },
            trailingIcon = {
                if (query.isNotEmpty()) {
                    IconButton(onClick = { onQuery("") }) { Icon(Icons.Rounded.Clear, stringResource(R.string.clear)) }
                }
            },
            singleLine = true,
            shape = RoundedCornerShape(28.dp),
        )
        PrimaryScrollableTabRow(selectedTabIndex = tab.ordinal, edgePadding = 8.dp) {
            LibraryTab.entries.forEach { t ->
                Tab(selected = tab == t, onClick = { onTab(t) }, text = { Text(stringResource(t.label)) })
            }
        }

        PullToRefreshBox(
            isRefreshing = state.scanning,
            onRefresh = app.library::refresh,
            modifier = Modifier.fillMaxSize(),
        ) {
            val body = when {
                // the concert playlist needs no local files
                tab == LibraryTab.PLAYLISTS -> TabBody.Showing(tab)
                state.tracks.isEmpty() && !state.scanned -> TabBody.Loading
                state.tracks.isEmpty() -> TabBody.Empty
                else -> TabBody.Showing(tab)
            }
            AnimatedContent(
                targetState = body,
                transitionSpec = {
                    val from = initialState
                    val to = targetState
                    if (from is TabBody.Showing && to is TabBody.Showing) {
                        motion.screens(forward = to.tab.ordinal > from.tab.ordinal)
                    } else {
                        motion.fadeThrough() // skeleton → songs
                    }
                },
                label = "library-tab",
            ) { shown ->
                when (shown) {
                    TabBody.Loading -> SkeletonList(10, round = tab == LibraryTab.ARTISTS)
                    TabBody.Empty -> EmptyLibrary(
                        hasAccess = state.hasAccess,
                        onGrant = onGrant,
                        onRescan = app.library::refresh,
                    )
                    is TabBody.Showing -> when (shown.tab) {
                        LibraryTab.SONGS -> TrackList(tracks, currentId, source = Sources.library(), header = {
                            PlayHeader(stringResource(R.string.library_count, tracks.size), tracks, Sources.library())
                            if (!state.hasAccess) AccessCard(onGrant)
                        })
                        LibraryTab.ALBUMS -> AlbumGrid(albums, onAlbum, onSound)
                        LibraryTab.ARTISTS -> ArtistList(artists, onArtist, onSound)
                        LibraryTab.PLAYLISTS -> PlaylistList(
                            state.playlists.filter { query.isBlank() || it.name.contains(query, ignoreCase = true) },
                            onPlaylist, onSound, onChat, onConcert, onParty, onHomeList,
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun EmptyLibrary(hasAccess: Boolean, onGrant: () -> Unit, onRescan: () -> Unit) {
    Column(
        Modifier
            .fillMaxSize()
            .padding(32.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        HomefiLogo(96.dp)
        Spacer(Modifier.height(16.dp))
        Text(stringResource(R.string.library_empty), color = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.height(24.dp))
        if (!hasAccess) {
            Button(onClick = onGrant) { Text(stringResource(R.string.library_access_grant)) }
            Spacer(Modifier.height(8.dp))
        }
        FilledTonalButton(onClick = onRescan) { Text(stringResource(R.string.library_rescan)) }
    }
}

@Composable
private fun AccessCard(onGrant: () -> Unit) {
    Card(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 8.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.primaryContainer),
    ) {
        Row(Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Rounded.FolderOpen, null)
            Spacer(Modifier.width(12.dp))
            Text(stringResource(R.string.library_access_title), Modifier.weight(1f))
            FilledTonalButton(onClick = onGrant) { Text(stringResource(R.string.library_access_grant)) }
        }
    }
}

@Composable
fun PlayHeader(subtitle: String, tracks: List<LibraryTrack>, source: SourceContext? = null) {
    val player = LocalContext.current.homefi.player
    val haptics = haptics()
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            subtitle,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.weight(1f),
        )
        // icons only next to Play: the row has to fit a DAP's narrow screen
        QueueButton({ next -> player.queueTracks(tracks, next) }, enabled = tracks.isNotEmpty())
        Spacer(Modifier.width(8.dp))
        FilledTonalButton(onClick = { haptics.press(); player.play(tracks, shuffle = true, source = source) }, enabled = tracks.isNotEmpty()) {
            Icon(Icons.Rounded.Shuffle, stringResource(R.string.library_shuffle))
        }
        Spacer(Modifier.width(8.dp))
        Button(onClick = { haptics.press(); player.play(tracks, source = source) }, enabled = tracks.isNotEmpty()) {
            Icon(Icons.Rounded.PlayArrow, null, Modifier.size(ButtonDefaults.IconSize))
            Spacer(Modifier.size(ButtonDefaults.IconSpacing))
            Text(stringResource(R.string.library_play_all))
        }
    }
}

@Composable
internal fun ScreenHeader(title: String, onBack: () -> Unit, onSound: (() -> Unit)? = null) {
    Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.back)) }
        Text(
            title,
            style = MaterialTheme.typography.titleLarge,
            fontWeight = FontWeight.Bold,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        // "Sound for this…": a taste layer for this album, artist or playlist
        if (onSound != null) IconButton(onClick = onSound) { Icon(Icons.Rounded.Tune, stringResource(R.string.sound_for)) }
    }
}

@Composable
private fun TrackList(tracks: List<LibraryTrack>, currentId: String?, source: SourceContext? = null, header: @Composable () -> Unit) {
    val player = LocalContext.current.homefi.player
    ListEntrance {
        LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 16.dp)) {
            item(key = "header") { Column(itemMotion(0)) { header() } }
            trackItems(tracks, currentId) { index -> player.play(tracks, index, source = source) }
        }
    }
}

private fun LazyListScope.trackItems(tracks: List<LibraryTrack>, currentId: String?, onPlay: (Int) -> Unit) {
    items(tracks.size, key = { tracks[it].path }) { index ->
        TrackRow(tracks[index], current = tracks[index].path == currentId, index = index, modifier = itemMotion(index + 1)) { onPlay(index) }
    }
}

/** Cover with a placeholder icon underneath (shown while loading or when there is none). */
@Composable
fun CoverArt(
    cover: Any?,
    size: Dp?,
    modifier: Modifier = Modifier,
    icon: ImageVector = Icons.Rounded.MusicNote,
    radius: Dp = 8.dp,
) {
    val skin = LocalSkin.current
    // vinyl: list thumbnails are round record labels with a spindle hole
    val label = skin.roundCovers && size != null && size <= 72.dp && radius * 2 < size
    val r = skin.coverRadius(radius)
    val shape = if (label || (size != null && r * 2 >= size)) CircleShape else RoundedCornerShape(r)
    val hole = skin.base
    Box(
        (if (size != null) modifier.size(size) else modifier)
            .clip(shape)
            .background(MaterialTheme.colorScheme.surfaceContainerHigh)
            .then(if (label) Modifier.drawWithContent { drawContent(); drawCircle(hole, this.size.minDimension * 0.07f) } else Modifier),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.fillMaxSize(0.45f))
        if (cover != null) {
            AsyncImage(model = cover, contentDescription = null, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize())
        }
    }
}

@OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)
@Composable
fun TrackRow(track: LibraryTrack, current: Boolean, index: Int = 0, modifier: Modifier = Modifier, onClick: () -> Unit) {
    val titleColor = currentTitleColor(current)
    val player = LocalContext.current.homefi.player
    var menu by remember { mutableStateOf(false) }
    val haptics = haptics()
    Box(modifier) {
    val sound = LocalContext.current.homefi.sound
    QueueMenu(
        menu, { menu = false },
        onSound = { sound.openMenu(TasteScope.Track(id = track.path, name = track.title)) },
        onPlaylistLikeThis = songSeedAction { track.toSeed() },
    ) { next -> player.queueTracks(listOf(track), next) }
    ListItem(
        modifier = Modifier.combinedClickable(
            onClick = {
                haptics.press()
                if (current) player.togglePlayPause() else onClick()
            },
            onLongClick = {
                haptics.longPress()
                menu = true
            },
        ),
        colors = skinRowColors(index, current),
        leadingContent = {
            Box(contentAlignment = Alignment.Center) {
                CoverArt(EmbeddedCover(track.path), 48.dp)
                if (current) NowCoverMark(8.dp)
            }
        },
        headlineContent = {
            Text(track.title, color = titleColor, maxLines = 1, overflow = TextOverflow.Ellipsis, fontWeight = if (current) FontWeight.Bold else null)
        },
        supportingContent = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                QualityBadge(track.format)
                Spacer(Modifier.width(6.dp))
                Text(
                    listOf(track.artist, track.album).filter(String::isNotBlank).joinToString(" · ")
                        .ifBlank { stringResource(R.string.unknown_artist) },
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        },
        trailingContent = if (track.durationMs > 0u) {
            { Text(formatDuration(track.durationMs.toLong()), style = MaterialTheme.typography.bodySmall.numeric()) }
        } else {
            null
        },
    )
    }
}

@Composable
private fun AlbumGrid(albums: List<Album>, onOpen: (Album) -> Unit, onSound: (TasteScope) -> Unit) {
    val haptics = haptics()
    ListEntrance {
    LazyVerticalGrid(
        columns = GridCells.Adaptive(156.dp),
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(16.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        itemsIndexed(albums, key = { _, it -> it.key }) { i, album ->
            Column(
                itemMotion(i)
                    .clip(RoundedCornerShape(12.dp))
                    // long press: "Sound for this…"
                    .combinedClickable(
                        onClick = { onOpen(album) },
                        onLongClick = { haptics.longPress(); onSound(TasteScope.Album(id = "", name = album.title)) },
                    ),
            ) {
                CoverArt(
                    album.cover,
                    size = null,
                    icon = Icons.Rounded.Album,
                    radius = 12.dp,
                    modifier = Modifier
                        .fillMaxWidth()
                        .aspectRatio(1f),
                )
                Spacer(Modifier.height(6.dp))
                Text(
                    album.title.ifBlank { stringResource(R.string.unknown_album) },
                    style = MaterialTheme.typography.titleSmall,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    album.artist.ifBlank { stringResource(R.string.unknown_artist) },
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
    }
}

@Composable
private fun PlaylistList(
    playlists: List<LocalPlaylist>,
    onOpen: (LocalPlaylist) -> Unit,
    onSound: (TasteScope) -> Unit,
    onChat: () -> Unit,
    onConcert: () -> Unit,
    onParty: () -> Unit,
    onHomeList: (HomePlaylist) -> Unit,
) {
    val haptics = haptics()
    val app = LocalContext.current.homefi
    val made by app.playlists.list.collectAsStateWithLifecycle()
    val settings by app.settings.playlists.collectAsStateWithLifecycle()
    val homeMade = if (settings.homeSection) made else emptyList()
    ListEntrance {
    LazyColumn(Modifier.fillMaxSize()) {
        // the AI chat is the general way in (primary); concert and party are the specific ones
        item(key = "chat") {
            MakeCard(Icons.Rounded.Chat, R.string.pl_chat_button, MaterialTheme.colorScheme.primaryContainer, MaterialTheme.colorScheme.onPrimaryContainer, onChat, itemMotion(0))
        }
        item(key = "concert") {
            MakeCard(Icons.Rounded.AutoAwesome, R.string.pl_concert_button, MaterialTheme.colorScheme.tertiaryContainer, MaterialTheme.colorScheme.onTertiaryContainer, onConcert, itemMotion(1))
        }
        item(key = "party") {
            MakeCard(Icons.Rounded.Celebration, R.string.pl_party_button, MaterialTheme.colorScheme.secondaryContainer, MaterialTheme.colorScheme.onSecondaryContainer, onParty, itemMotion(2))
        }
        if (homeMade.isNotEmpty()) {
            item(key = "made-head") {
                Text(
                    stringResource(R.string.pl_made_by_homefi),
                    style = MaterialTheme.typography.titleSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 4.dp),
                )
            }
            itemsIndexed(homeMade, key = { _, it -> "made-" + it.id }) { i, playlist ->
                val popular = playlist.concert?.source is SetlistSource.TidalPopular
                val party = playlist.origin is PlaylistOrigin.Party
                val chat = playlist.origin is PlaylistOrigin.Chat
                ListItem(
                    modifier = itemMotion(i + 3).combinedClickable(
                        onClick = { onHomeList(playlist) },
                        onLongClick = { haptics.longPress(); onSound(TasteScope.Playlist(id = playlist.id, name = playlist.name)) },
                    ),
                    colors = skinRowColors(i, current = false),
                    leadingContent = {
                        CoverArt(playlist.entries.firstNotNullOfOrNull { it.coverUrl }, 52.dp, icon = if (party) Icons.Rounded.Celebration else if (chat) Icons.Rounded.Chat else Icons.Rounded.AutoAwesome)
                    },
                    headlineContent = { Text(playlist.name, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    supportingContent = {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Text(
                                stringResource(R.string.pl_count, playlist.entries.size, formatDuration(playlist.entries.sumOf { it.durationMs.toLong() })),
                                maxLines = 1,
                            )
                            if (popular) {
                                Spacer(Modifier.width(6.dp))
                                NotRealSetlistBadge()
                            }
                            if (party) {
                                Spacer(Modifier.width(6.dp))
                                PartyBadge()
                            }
                            if (chat) {
                                Spacer(Modifier.width(6.dp))
                                ChatBadge()
                            }
                        }
                    },
                )
            }
        }
        if (playlists.isEmpty() && homeMade.isEmpty()) {
            item(key = "empty") {
                Text(
                    stringResource(R.string.library_playlists_empty),
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(32.dp),
                )
            }
        }
        itemsIndexed(playlists, key = { _, it -> it.path }) { i, playlist ->
            ListItem(
                modifier = itemMotion(i + homeMade.size + 2).combinedClickable(
                    onClick = { onOpen(playlist) },
                    onLongClick = { haptics.longPress(); onSound(TasteScope.Playlist(id = "", name = playlist.name)) },
                ),
                colors = skinRowColors(i, current = false),
                leadingContent = {
                    CoverArt(playlist.tracks.firstOrNull()?.let(::EmbeddedCover), 52.dp, icon = Icons.AutoMirrored.Rounded.QueueMusic)
                },
                headlineContent = { Text(playlist.name, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                supportingContent = { Text(stringResource(R.string.library_count, playlist.tracks.size)) },
            )
        }
    }
    }
}

/** One way into making a playlist (chat, concert or party): an icon and a label on a tinted card. */
@Composable
private fun MakeCard(
    icon: ImageVector,
    label: Int,
    container: Color,
    content: Color,
    onClick: () -> Unit,
    modifier: Modifier,
) {
    val haptics = haptics()
    Card(
        modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 8.dp)
            .clickable(role = Role.Button) { haptics.press(); onClick() },
        colors = CardDefaults.cardColors(containerColor = container, contentColor = content),
    ) {
        Row(Modifier.heightIn(min = 56.dp).padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(icon, null)
            Spacer(Modifier.width(12.dp))
            Text(stringResource(label), Modifier.weight(1f), fontWeight = FontWeight.Bold)
        }
    }
}

@Composable
private fun ArtistList(artists: List<Artist>, onOpen: (Artist) -> Unit, onSound: (TasteScope) -> Unit) {
    val haptics = haptics()
    ListEntrance {
    LazyColumn(Modifier.fillMaxSize()) {
        itemsIndexed(artists, key = { _, it -> it.name }) { i, artist ->
            ListItem(
                modifier = itemMotion(i).combinedClickable(
                    onClick = { onOpen(artist) },
                    onLongClick = { if (artist.name.isNotBlank()) { haptics.longPress(); onSound(TasteScope.Artist(id = "", name = artist.name)) } },
                ),
                colors = skinRowColors(i, current = false),
                leadingContent = { CoverArt(EmbeddedCover(artist.tracks.first().path), 48.dp, icon = Icons.Rounded.Person, radius = 24.dp) },
                headlineContent = { Text(artist.name.ifBlank { stringResource(R.string.unknown_artist) }, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                supportingContent = { Text(stringResource(R.string.library_artist_summary, artist.albums, artist.tracks.size)) },
            )
        }
    }
    }
}

@Composable
private fun AlbumDetail(album: Album, currentId: String?, onBack: () -> Unit) {
    val app = LocalContext.current.homefi
    val player = app.player
    val haptics = haptics()
    val source = remember(album.key) { Sources.album("", album.title) }
    ListEntrance {
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 16.dp)) {
        item(key = "header") {
            Column(itemMotion(0)) {
            ScreenHeader(
                album.title.ifBlank { stringResource(R.string.unknown_album) },
                onBack,
                onSound = album.title.takeIf { it.isNotBlank() }?.let { t -> { app.sound.openMenu(TasteScope.Album(id = "", name = t)) } },
            )
            Row(Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
                CoverArt(album.cover, 128.dp, icon = Icons.Rounded.Album, radius = 12.dp)
                Spacer(Modifier.width(16.dp))
                Column {
                    Text(album.artist.ifBlank { stringResource(R.string.unknown_artist) }, style = MaterialTheme.typography.titleMedium)
                    Text(
                        listOfNotNull(album.year?.toString(), stringResource(R.string.library_count, album.tracks.size)).joinToString(" · "),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Spacer(Modifier.height(6.dp))
                    QualityBadge(album.tracks.first().quality)
                }
            }
            PlayHeader("", album.tracks, source)
            }
        }
        items(album.tracks.size, key = { album.tracks[it].path }) { index ->
            val track = album.tracks[index]
            val current = track.path == currentId
            ListItem(
                modifier = itemMotion(index + 1).clickable {
                    haptics.press()
                    if (current) player.togglePlayPause() else player.play(album.tracks, index, source = source)
                },
                colors = skinRowColors(index, current),
                leadingContent = {
                    if (current) {
                        NowMark(currentTitleColor(true).takeOrElse { LocalContentColor.current }, Modifier.width(28.dp))
                    } else {
                        Text(
                            if (track.trackNumber > 0u) track.trackNumber.toString() else "–",
                            style = MaterialTheme.typography.bodyMedium.numeric(),
                            color = currentTitleColor(current).takeOrElse { if (current) LocalContentColor.current else MaterialTheme.colorScheme.onSurfaceVariant },
                            modifier = Modifier.width(28.dp),
                        )
                    }
                },
                headlineContent = {
                    Text(
                        track.title,
                        color = currentTitleColor(current),
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                },
                supportingContent = if (track.artist != album.artist && track.artist.isNotBlank()) {
                    { Text(track.artist, maxLines = 1, overflow = TextOverflow.Ellipsis) }
                } else {
                    null
                },
                trailingContent = { Text(formatDuration(track.durationMs.toLong()), style = MaterialTheme.typography.bodySmall.numeric()) },
            )
        }
    }
    }
}
