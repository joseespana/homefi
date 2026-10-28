package com.perseusoft.homefi.ui.tidal

import com.perseusoft.homefi.ui.haptics
import com.perseusoft.homefi.ui.theme.SkinHeading
import androidx.compose.animation.AnimatedContent
import com.perseusoft.homefi.ui.theme.ListEntrance
import com.perseusoft.homefi.ui.theme.LocalMotion
import com.perseusoft.homefi.ui.theme.itemMotion
import com.perseusoft.homefi.ui.theme.currentTitleColor
import com.perseusoft.homefi.ui.theme.numeric
import com.perseusoft.homefi.ui.theme.skinRowColors
import androidx.compose.runtime.setValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.foundation.combinedClickable
import com.perseusoft.homefi.ui.QueueButton
import com.perseusoft.homefi.ui.QueueMenu
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.Album
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.Download
import androidx.compose.material.icons.rounded.Layers
import androidx.compose.material.icons.rounded.DownloadDone
import androidx.compose.material.icons.rounded.MusicNote
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Shuffle
import androidx.compose.material.icons.rounded.Videocam
import com.perseusoft.homefi.ui.theme.Button
import androidx.compose.material3.CircularProgressIndicator
import com.perseusoft.homefi.ui.theme.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import com.perseusoft.homefi.ui.theme.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.snapshotFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.filter
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.AlbumVersionHit
import com.perseusoft.homefi.core.ItemKind
import com.perseusoft.homefi.core.SearchHit
import com.perseusoft.homefi.core.TasteScope
import com.perseusoft.homefi.player.Sources
import androidx.compose.material.icons.rounded.Tune
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.tidalId
import com.perseusoft.homefi.ui.QualityBadge
import com.perseusoft.homefi.ui.SkeletonDetail
import com.perseusoft.homefi.ui.SkeletonList
import com.perseusoft.homefi.ui.formatDuration
import com.perseusoft.homefi.ui.NowCoverMark
import com.perseusoft.homefi.ui.library.CoverArt

/** What a Tidal row needs to know about the app: is it playing, on the device, queued. */
class RowContext(
    val playingId: String?,
    val local: Set<String>,
    val queued: Set<String>,
    val onPlay: (List<SearchHit>, Int) -> Unit,
    val onOpen: (SearchHit) -> Unit,
    val onDownload: (SearchHit) -> Unit,
    /** Play / open / download a release from the album version picker. */
    val onVersion: (AlbumVersionHit, VersionAction) -> Unit = { _, _ -> },
)

/** The same row context, with taps on tracks playing [key] (the whole list, loaded or not). */
fun RowContext.playing(vm: TidalViewModel, key: ListKey) =
    RowContext(playingId, local, queued, { _, i -> vm.play(key, i) }, onOpen, onDownload, onVersion)

fun localKey(artist: String, title: String) = "${artist.trim().lowercase()}\u0000${title.trim().lowercase()}"

@Composable
fun rememberRowContext(vm: TidalViewModel): RowContext {
    val app = LocalContext.current.homefi
    val library by app.library.state.collectAsStateWithLifecycle()
    val player by app.player.state.collectAsStateWithLifecycle()
    val queued by vm.queued.collectAsStateWithLifecycle()
    // matched by artist + title: library files carry no Tidal id
    val local = remember(library.tracks) { library.tracks.mapTo(HashSet()) { localKey(it.artist, it.title) } }
    return RowContext(
        playingId = player.current?.mediaId?.let(::tidalId),
        local = local,
        queued = queued,
        onPlay = { list, i -> app.player.playTidal(list, i) },
        onOpen = vm::open,
        onDownload = vm::download,
        onVersion = vm::versionAction,
    )
}

fun ItemKind.icon(): ImageVector = when (this) {
    ItemKind.TRACK -> Icons.Rounded.MusicNote
    ItemKind.ALBUM -> Icons.Rounded.Album
    ItemKind.ARTIST -> Icons.Rounded.Person
    ItemKind.PLAYLIST, ItemKind.MIX -> Icons.AutoMirrored.Rounded.QueueMusic
    ItemKind.VIDEO -> Icons.Rounded.Videocam
}

/**
 * One Tidal result. Tracks play (streamed) on tap; albums, artists and
 * playlists open. The trailing button downloads, or shows it's on the device.
 */
@OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)
@Composable
fun TidalRow(hit: SearchHit, list: List<SearchHit>, index: Int, ctx: RowContext, modifier: Modifier = Modifier) {
    val isTrack = hit.kind == ItemKind.TRACK
    val playing = isTrack && hit.id == ctx.playingId
    val onDevice = isTrack && localKey(hit.artist, hit.title) in ctx.local
    val queued = "${hit.kind.name.lowercase()}/${hit.id}" in ctx.queued
    val openable = hit.kind in setOf(ItemKind.ALBUM, ItemKind.ARTIST, ItemKind.PLAYLIST)
    val player = LocalContext.current.homefi.player
    var menu by remember { mutableStateOf(false) }
    var picker by remember { mutableStateOf(false) }
    val hasVersions = hit.versions.size > 1
    val haptics = haptics()
    Box(modifier) {
    if (isTrack) QueueMenu(menu, { menu = false }) { next -> player.queueTidal(listOf(hit), next) }
    if (picker) AlbumVersionsSheet(hit.title, hit.versions, null, { picker = false }, ctx.onVersion)
    ListItem(
        modifier = Modifier.combinedClickable(
            enabled = isTrack || openable,
            onClick = {
                when {
                    playing -> haptics.press().also { player.togglePlayPause() }
                    isTrack -> haptics.press().also { ctx.onPlay(list, index) }
                    else -> ctx.onOpen(hit)
                }
            },
            onLongClick = {
                if (isTrack) {
                    haptics.longPress()
                    menu = true
                } else if (hasVersions) {
                    haptics.longPress()
                    picker = true
                }
            },
        ),
        colors = skinRowColors(index, playing),
        leadingContent = {
            Box(contentAlignment = Alignment.Center) {
                CoverArt(hit.coverUrl, 52.dp, icon = hit.kind.icon(), radius = if (hit.kind == ItemKind.ARTIST) 26.dp else 8.dp)
                if (playing) NowCoverMark(if (hit.kind == ItemKind.ARTIST) 26.dp else 8.dp)
            }
        },
        headlineContent = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    hit.title,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    color = currentTitleColor(playing),
                    fontWeight = if (playing) FontWeight.Bold else null,
                    modifier = Modifier.weight(1f, fill = false),
                )
                if (hit.explicit) {
                    Spacer(Modifier.width(6.dp))
                    QualityBadge("E")
                }
            }
        },
        supportingContent = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                hit.quality?.let {
                    QualityBadge(it)
                    Spacer(Modifier.width(6.dp))
                }
                if (hasVersions) {
                    VersionsChip(hit.versions.size, { picker = true }, onCover = false)
                    Spacer(Modifier.width(6.dp))
                }
                Text(hit.subtitle, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        },
        trailingContent = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                hit.durationMs?.let {
                    Text(formatDuration(it.toLong()), style = MaterialTheme.typography.bodySmall.numeric())
                }
                when {
                    onDevice -> Icon(
                        Icons.Rounded.CheckCircle,
                        stringResource(R.string.tidal_on_device),
                        tint = MaterialTheme.colorScheme.secondary,
                        modifier = Modifier.padding(12.dp),
                    )
                    queued -> Icon(
                        Icons.Rounded.DownloadDone,
                        stringResource(R.string.tidal_queued),
                        tint = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.padding(12.dp),
                    )
                    hit.kind != ItemKind.ARTIST -> IconButton(onClick = { haptics.confirm(); ctx.onDownload(hit) }) {
                        Icon(Icons.Rounded.Download, stringResource(R.string.download), tint = MaterialTheme.colorScheme.primary)
                    }
                }
            }
        },
    )
    }
}

fun LazyListScope.tidalSection(title: Int, hits: List<SearchHit>, ctx: RowContext, keyPrefix: String = "$title") {
    if (hits.isEmpty()) return
    item(key = "header-$keyPrefix") { Box(itemMotion(0)) { SectionTitle(stringResource(title)) } }
    tidalRows(hits, ctx, keyPrefix)
}

/**
 * Rows of a Tidal list. Keys carry the position: a list may hold the same track twice, and
 * pages only ever append, so earlier keys stay put (and an appended page just fades in).
 */
fun LazyListScope.tidalRows(hits: List<SearchHit>, ctx: RowContext, keyPrefix: String) {
    items(hits.size, key = { "$keyPrefix-$it-${hits[it].kind}-${hits[it].id}" }) { i -> TidalRow(hits[i], hits, i, ctx, itemMotion(i + 1)) }
}

/** Under a list still loading: a spinner, or why the next page failed with a retry. */
fun LazyListScope.pagingFooter(list: PagedHits, onRetry: () -> Unit) {
    if (list.complete) return
    item(key = "paging-footer") {
        Box(itemMotion().fillMaxWidth().padding(16.dp), contentAlignment = Alignment.Center) {
            val error = list.moreError
            if (error == null) {
                CircularProgressIndicator(Modifier.size(28.dp), strokeWidth = 3.dp)
            } else {
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    Text(stringResource(R.string.tidal_load_more_failed), color = MaterialTheme.colorScheme.error)
                    Text(error, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, textAlign = TextAlign.Center)
                    TextButton(onClick = onRetry) { Text(stringResource(R.string.retry)) }
                }
            }
        }
    }
}

/** Asks for the next page when the last visible row gets within [ahead] rows of the end. */
@Composable
fun LoadMoreEffect(state: LazyListState, list: PagedHits, onLoadMore: () -> Unit, ahead: Int = 10) {
    val current by rememberUpdatedState(list)
    val load by rememberUpdatedState(onLoadMore)
    LaunchedEffect(state) {
        snapshotFlow {
            val info = state.layoutInfo
            val last = info.visibleItemsInfo.lastOrNull()?.index ?: -1
            val l = current
            !l.complete && !l.loadingMore && l.moreError == null && info.totalItemsCount > 0 && last >= info.totalItemsCount - 1 - ahead
        }
            .distinctUntilChanged()
            .filter { it }
            .collect { load() }
    }
}

@Composable
fun SectionTitle(text: String) {
    SkinHeading(
        text,
        style = MaterialTheme.typography.titleMedium,
        modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 16.dp, bottom = 4.dp),
    )
}

/** Albums / EPs as a horizontal shelf of covers. */
fun LazyListScope.shelf(title: Int, hits: List<SearchHit>, ctx: RowContext) {
    if (hits.isEmpty()) return
    item(key = "shelf-$title") {
        Column(itemMotion(1)) {
            SectionTitle(stringResource(title))
            LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                items(hits, key = { it.id }) { hit -> ShelfCard(hit, ctx, itemMotion()) }
            }
        }
    }
}

/**
 * A cover in a shelf. Albums Tidal lists several times are one card: a tap opens the default
 * release, the "N versions" chip (or a long-press) lists them all.
 */
@OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)
@Composable
private fun ShelfCard(hit: SearchHit, ctx: RowContext, modifier: Modifier = Modifier) {
    var picker by remember { mutableStateOf(false) }
    val hasVersions = hit.versions.size > 1
    val haptics = haptics()
    if (picker) AlbumVersionsSheet(hit.title, hit.versions, null, { picker = false }, ctx.onVersion)
    Column(
        modifier.width(140.dp).combinedClickable(
            onClick = { ctx.onOpen(hit) },
            onLongClick = if (hasVersions) ({ haptics.longPress(); picker = true }) else null,
        ),
    ) {
        Box {
            CoverArt(hit.coverUrl, 140.dp, icon = hit.kind.icon(), radius = 10.dp)
            if (hasVersions) VersionsChip(hit.versions.size, { picker = true }, Modifier.align(Alignment.BottomStart).padding(6.dp))
        }
        Row(verticalAlignment = Alignment.Top, modifier = Modifier.padding(top = 6.dp)) {
            Text(hit.title, maxLines = 2, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.weight(1f, fill = false))
            if (hit.explicit) {
                Spacer(Modifier.width(4.dp))
                QualityBadge("E", Modifier.padding(top = 2.dp))
            }
        }
        Text(hit.subtitle, maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
private fun BackBar(title: String, onBack: () -> Unit, onSound: (() -> Unit)? = null) {
    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth().padding(end = if (onSound == null) 16.dp else 4.dp)) {
        IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.back)) }
        Text(title, style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
        // "Sound for this…": a taste layer for this album, playlist or artist
        if (onSound != null) IconButton(onClick = onSound) { Icon(Icons.Rounded.Tune, stringResource(R.string.sound_for)) }
    }
}

/** Big cover, title, subtitle and Play / Shuffle / Download for a list of tracks. */
@Composable
private fun Hero(
    cover: String?,
    coverSize: Dp,
    round: Boolean,
    title: String,
    subtitle: String,
    hasTracks: Boolean,
    onPlay: (shuffle: Boolean) -> Unit,
    onQueue: (next: Boolean) -> Unit,
    onDownload: (() -> Unit)?,
    onSubtitle: (() -> Unit)? = null,
    explicit: Boolean = false,
    /** Below the subtitle: this release's badges, "Other versions" and a note on Atmos. */
    extra: @Composable () -> Unit = {},
) {
    val explicitLabel = stringResource(R.string.album_explicit)
    Column(Modifier.fillMaxWidth().padding(16.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        CoverArt(cover, coverSize, radius = if (round) coverSize / 2 else 16.dp)
        Spacer(Modifier.size(12.dp))
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(title, style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.Bold, textAlign = TextAlign.Center, modifier = Modifier.weight(1f, fill = false))
            if (explicit) {
                Spacer(Modifier.width(8.dp))
                QualityBadge("E", Modifier.semantics { contentDescription = explicitLabel })
            }
        }
        Text(
            subtitle,
            style = MaterialTheme.typography.bodyLarge,
            color = if (onSubtitle != null) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
            modifier = if (onSubtitle != null) Modifier.clickable(onClick = onSubtitle) else Modifier,
        )
        extra()
        Spacer(Modifier.size(12.dp))
        val haptics = haptics()
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(onClick = { haptics.press(); onPlay(false) }, enabled = hasTracks) {
                Icon(Icons.Rounded.PlayArrow, null)
                Text(stringResource(R.string.library_play_all), Modifier.padding(start = 4.dp))
            }
            FilledTonalButton(onClick = { haptics.press(); onPlay(true) }, enabled = hasTracks) {
                Icon(Icons.Rounded.Shuffle, null)
            }
            QueueButton(onQueue, enabled = hasTracks)
            if (onDownload != null) {
                OutlinedButton(onClick = { haptics.confirm(); onDownload() }) { Icon(Icons.Rounded.Download, stringResource(R.string.download)) }
            }
        }
    }
}

@Composable
private fun <T> Loaded(
    load: Load<T>?,
    onRetry: () -> Unit,
    skeleton: @Composable () -> Unit = { SkeletonDetail() },
    content: @Composable (T) -> Unit,
) {
    // skeleton → page (or the error) fades through; a new page of rows doesn't count
    val motion = LocalMotion.current
    AnimatedContent(
        targetState = load,
        contentKey = { if (it == null) Load.Loading::class else it::class },
        transitionSpec = { motion.fadeThrough() },
        label = "tidal-load",
    ) { load ->
    when (load) {
        null, Load.Loading -> skeleton()
        is Load.Failed -> Column(Modifier.fillMaxSize().padding(32.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center) {
            Text(load.message, color = MaterialTheme.colorScheme.error, textAlign = TextAlign.Center)
            Spacer(Modifier.size(12.dp))
            FilledTonalButton(onClick = onRetry) { Text(stringResource(R.string.retry)) }
        }
        is Load.Ready -> content(load.value)
    }
    }
}

/**
 * Under an album's title: its quality and labels (Remastered 2023, Box set +209...),
 * "Other versions (N)" when Tidal has more releases, and a note when it's Dolby Atmos.
 */
@Composable
private fun AlbumVersionsLine(
    title: String,
    id: String,
    quality: String?,
    versions: List<AlbumVersionHit>,
    onAction: (AlbumVersionHit, VersionAction) -> Unit,
) {
    var picker by remember { mutableStateOf(false) }
    val mine = versions.firstOrNull { it.id == id }
    if (picker) AlbumVersionsSheet(title, versions, id, { picker = false }, onAction)
    Spacer(Modifier.size(8.dp))
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        quality?.let { QualityBadge(it) }
        if (mine != null) VersionLabels(mine, skip = quality)
    }
    if (versions.size > 1) {
        TextButton(onClick = { picker = true }) {
            Icon(Icons.Rounded.Layers, null, Modifier.size(18.dp))
            Text(stringResource(R.string.album_other_versions, versions.size - 1), Modifier.padding(start = 6.dp))
        }
    }
    if (mine?.atmos == true) {
        Text(
            stringResource(R.string.album_atmos_hint),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(horizontal = 8.dp),
        )
    }
}

/** The page on top of the browser stack. */
@Composable
fun TidalRoutePage(route: TidalRoute, vm: TidalViewModel) {
    val pages by vm.pages.collectAsStateWithLifecycle()
    val versions by vm.versions.collectAsStateWithLifecycle()
    val ctx = rememberRowContext(vm)
    val player = LocalContext.current.homefi.player
    val sound = LocalContext.current.homefi.sound
    val load = pages[route]
    val key = ListKey.Page(route)
    Column(Modifier.fillMaxSize()) {
        when (route) {
            is TidalRoute.Artist -> Loaded(load?.artist(), { vm.load(route) }, { SkeletonDetail(round = true) }) { artist ->
                ListEntrance {
                LazyColumn(Modifier.fillMaxSize()) {
                    item(key = "bar") { BackBar(artist.name, vm::pop) { sound.soundFor(TasteScope.Artist(id = artist.id, name = artist.name)) } }
                    item(key = "hero") {
                        Hero(
                            artist.pictureUrl, 120.dp, true, artist.name, stringResource(R.string.tidal_top_tracks),
                            hasTracks = artist.topTracks.isNotEmpty(),
                            onPlay = { shuffle -> player.playTidal(artist.topTracks, shuffle = shuffle, source = Sources.artist(artist.id, artist.name)) },
                            onQueue = { next -> player.queueTidal(artist.topTracks, next) },
                            onDownload = null,
                        )
                    }
                    tidalSection(R.string.tidal_top_tracks, artist.topTracks, ctx, "top")
                    shelf(R.string.search_albums, artist.albums, ctx)
                    shelf(R.string.tidal_eps_singles, artist.epsAndSingles, ctx)
                    item(key = "end") { Spacer(Modifier.size(16.dp)) }
                }
                }
            }
            is TidalRoute.Album -> Loaded(load?.album(), { vm.load(route) }) { page ->
                val album = page.info
                val tracks = page.tracks
                val state = rememberLazyListState()
                LoadMoreEffect(state, tracks, { vm.loadMore(key) })
                ListEntrance {
                LazyColumn(Modifier.fillMaxSize(), state = state) {
                    item(key = "bar") { BackBar(album.title, vm::pop) { sound.soundFor(TasteScope.Album(id = album.id, name = album.title)) } }
                    item(key = "hero") {
                        Hero(
                            album.coverUrl, 180.dp, false, album.title,
                            listOfNotNull(album.artist, album.year?.toString()).joinToString(" · "),
                            hasTracks = tracks.items.isNotEmpty(),
                            onPlay = { shuffle -> vm.play(key, shuffle = shuffle) },
                            onQueue = { next -> vm.queue(key, next) },
                            onDownload = { vm.download(album.title, "album/${album.id}") },
                            onSubtitle = album.artistId?.let { id -> { vm.push(TidalRoute.Artist(id)) } },
                            explicit = album.explicit,
                        ) { AlbumVersionsLine(album.title, album.id, album.quality, versions[album.id].orEmpty(), vm::versionAction) }
                    }
                    if (tracks.items.isNotEmpty()) {
                        item(key = "header-album") { SectionTitle(stringResource(R.string.search_tracks)) }
                        tidalRows(tracks.items, ctx.playing(vm, key), "album")
                    }
                    pagingFooter(tracks) { vm.loadMore(key) }
                    if (tracks.complete) {
                        album.copyright?.let { item(key = "copyright") { Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = itemMotion().padding(16.dp)) } }
                    }
                }
                }
            }
            is TidalRoute.Playlist -> Loaded(load?.playlist(), { vm.load(route) }) { page ->
                val playlist = page.info
                val tracks = page.tracks
                val state = rememberLazyListState()
                LoadMoreEffect(state, tracks, { vm.loadMore(key) })
                ListEntrance {
                LazyColumn(Modifier.fillMaxSize(), state = state) {
                    item(key = "bar") { BackBar(playlist.title, vm::pop) { sound.soundFor(TasteScope.Playlist(id = playlist.uuid, name = playlist.title)) } }
                    item(key = "hero") {
                        Hero(
                            playlist.coverUrl, 180.dp, false, playlist.title,
                            stringResource(R.string.library_count, tracks.count),
                            hasTracks = tracks.items.isNotEmpty(),
                            onPlay = { shuffle -> vm.play(key, shuffle = shuffle) },
                            onQueue = { next -> vm.queue(key, next) },
                            onDownload = { vm.download(playlist.title, "playlist/${playlist.uuid}") },
                        )
                    }
                    if (tracks.items.isNotEmpty()) {
                        item(key = "header-playlist") { SectionTitle(stringResource(R.string.search_tracks)) }
                        tidalRows(tracks.items, ctx.playing(vm, key), "playlist")
                    }
                    pagingFooter(tracks) { vm.loadMore(key) }
                }
                }
            }
        }
    }
}

/** The user's Tidal collection, shown before searching. */
@Composable
fun CollectionView(which: Collection, vm: TidalViewModel) {
    val collection by vm.collection.collectAsStateWithLifecycle()
    val ctx = rememberRowContext(vm)
    val key = ListKey.Of(which)
    Loaded(collection[which], { vm.loadCollection(which, force = true) }, { SkeletonList(round = which == Collection.ARTISTS) }) { list ->
        if (list.items.isEmpty() && list.complete) {
            Text(stringResource(R.string.tidal_collection_empty), color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(32.dp))
        } else {
            // one scroll position per collection
            androidx.compose.runtime.key(which) {
                val state = rememberLazyListState()
                LoadMoreEffect(state, list, { vm.loadMore(key) })
                ListEntrance {
                LazyColumn(Modifier.fillMaxSize(), state = state) {
                    item(key = "total") {
                        Text(
                            stringResource(R.string.tidal_items_total, list.count),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
                        )
                    }
                    tidalRows(list.items, ctx.playing(vm, key), "c-$which")
                    pagingFooter(list) { vm.loadMore(key) }
                }
                }
            }
        }
    }
}
