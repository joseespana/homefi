package com.perseusoft.homefi.tv

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.Album
import androidx.compose.material.icons.rounded.LibraryMusic
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.library.EmbeddedCover
import com.perseusoft.homefi.player.NowPlaying
import com.perseusoft.homefi.player.Recent
import com.perseusoft.homefi.player.tidalId
import com.perseusoft.homefi.ui.home.cover
import com.perseusoft.homefi.ui.home.greetingFor
import com.perseusoft.homefi.ui.home.homePlaylists
import com.perseusoft.homefi.ui.home.playPlaylist
import com.perseusoft.homefi.ui.home.playRecentAlbum
import com.perseusoft.homefi.ui.home.recentAlbums
import com.perseusoft.homefi.ui.home.recentSongs
import com.perseusoft.homefi.ui.library.CoverArt
import com.perseusoft.homefi.ui.player.coverModel
import kotlinx.coroutines.launch
import java.util.Calendar
import kotlin.math.roundToInt

/** Where the focused row sits in the list: this share of its height from the top (docs/TV_UI.md 6.1). */
private const val ROW_PIN = 0.4f

/** Hero card height and its cover. */
private val HERO_HEIGHT = 118.dp
private val HERO_COVER = 88.dp

/**
 * Home: the greeting, the continue-listening card and the shelves of what was heard and
 * what HomeFi built, with a key hint below them (never over them). State comes from the
 * same recents / library the phone's home reads ([com.perseusoft.homefi.ui.home] rows).
 */
@Composable
fun TvHome(entryKey: Int, onOpenPlayer: () -> Unit, onOpenLibrary: () -> Unit) {
    val app = LocalContext.current.homefi
    val player by app.player.state.collectAsStateWithLifecycle()
    val recents by app.player.recents.items.collectAsStateWithLifecycle()
    val library by app.library.state.collectAsStateWithLifecycle()
    val albums = remember(recents) { recentAlbums(recents) }
    val playlists = remember(library.playlists) { homePlaylists(library.playlists) }
    val firstFocus = remember { FocusRequester() }
    val list = rememberLazyListState()
    val scope = rememberCoroutineScope()
    var viewportPx by remember { mutableIntStateOf(0) }
    // the focused shelf rises to 40 % of the height, the one before it still peeking above
    val pin: (Int) -> Unit = { index ->
        scope.launch { list.animateScrollToItem(index, -(viewportPx * ROW_PIN).roundToInt()) }
    }
    val playNow: (suspend () -> Unit) -> Unit = { start -> scope.launch { start(); onOpenPlayer() } }

    val safe = tvSafePadding()
    Column(Modifier.fillMaxSize().padding(top = safe.calculateTopPadding(), end = safe.calculateRightPadding(androidx.compose.ui.unit.LayoutDirection.Ltr))) {
        LazyColumn(
            state = list,
            modifier = Modifier.weight(1f).fillMaxWidth().onSizeChanged { viewportPx = it.height },
            contentPadding = PaddingValues(start = 12.dp, bottom = 40.dp),
            verticalArrangement = Arrangement.spacedBy(18.dp),
        ) {
            item(key = "header") { HomeHeader(player.current) }
            if (recents.isEmpty() && player.current == null) {
                item(key = "empty") { EmptyHome(entryKey, onOpenLibrary) }
            } else {
                item(key = "hero") {
                    Hero(
                        playing = player.current,
                        isPlaying = player.isPlaying,
                        positionMs = player.positionMs,
                        durationMs = player.durationMs,
                        last = recents.firstOrNull(),
                        entryKey = entryKey,
                        firstFocus = firstFocus,
                        onOpen = onOpenPlayer,
                        onToggle = app.player::togglePlayPause,
                        onResume = { playNow { app.player.playRecents(recents, 0) } },
                        onFocused = { if (it) pin(1) },
                    )
                }
                if (recents.isNotEmpty()) {
                    item(key = "recent-songs") {
                        Shelf(stringResource(R.string.home_recent_songs), stringResource(R.string.tv_home_rows_sub)) {
                            items(recentSongs(recents), key = { it.mediaId }) { r ->
                                TvCoverCard(
                                    cover = r.cover(),
                                    title = r.title,
                                    subtitle = r.artist,
                                    onFocused = { if (it) pin(2) },
                                    onClick = { playNow { app.player.playRecents(recents, recents.indexOf(r)) } },
                                )
                            }
                        }
                    }
                }
                if (albums.isNotEmpty()) {
                    item(key = "recent-albums") {
                        Shelf(stringResource(R.string.home_recent_albums)) {
                            items(albums, key = { "album-${it.mediaId}" }) { r ->
                                TvCoverCard(
                                    cover = r.cover(),
                                    title = r.album,
                                    subtitle = r.artist,
                                    icon = Icons.Rounded.Album,
                                    onFocused = { if (it) pin(3) },
                                    onClick = { playNow { app.playRecentAlbum(r, recents, library) } },
                                )
                            }
                        }
                    }
                }
            }
            if (playlists.isNotEmpty()) {
                item(key = "playlists") {
                    Shelf(stringResource(R.string.library_playlists)) {
                        items(playlists, key = { it.path }) { p ->
                            TvCoverCard(
                                cover = p.tracks.firstOrNull()?.let(::EmbeddedCover),
                                title = p.name,
                                subtitle = stringResource(R.string.library_count, p.tracks.size),
                                icon = Icons.AutoMirrored.Rounded.QueueMusic,
                                onFocused = { if (it) pin(4) },
                                onClick = { playNow { app.playPlaylist(p) } },
                            )
                        }
                    }
                }
            }
        }
        TvKeyHints(
            listOf("OK" to stringResource(R.string.tv_hint_ok_open), "◀" to stringResource(R.string.tv_hint_back_drawer)),
            Modifier.fillMaxWidth().padding(start = 12.dp, top = 6.dp, bottom = safe.calculateBottomPadding().coerceAtLeast(8.dp)),
        )
    }
}

@Composable
private fun HomeHeader(playing: NowPlaying?) {
    val look = LocalTvLook.current
    val output = outputLabel()
    Row(Modifier.fillMaxWidth().padding(top = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            val hello = stringResource(greetingFor(Calendar.getInstance().get(Calendar.HOUR_OF_DAY)))
            Text(
                hello.uppercase(),
                style = TvType.caps,
                color = look.ink2,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(stringResource(R.string.home_continue), style = TvType.title, color = look.ink, maxLines = 1)
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            playing?.quality?.let { TvTag(it, tone = look.ok) }
            output?.let { TvTag(it) }
            TvClock(Modifier.padding(start = 6.dp))
        }
    }
}

@Composable
private fun Hero(
    playing: NowPlaying?,
    isPlaying: Boolean,
    positionMs: Long,
    durationMs: Long,
    last: Recent?,
    entryKey: Int,
    firstFocus: FocusRequester,
    onOpen: () -> Unit,
    onToggle: () -> Unit,
    onResume: () -> Unit,
    onFocused: (Boolean) -> Unit,
) {
    val look = LocalTvLook.current
    TvGlass(Modifier.fillMaxWidth().height(HERO_HEIGHT)) {
        Row(Modifier.fillMaxSize().padding(horizontal = 15.dp), verticalAlignment = Alignment.CenterVertically) {
            val cover = playing?.coverModel() ?: last?.cover()
            CoverArt(cover, HERO_COVER, radius = 11.dp)
            Spacer(Modifier.width(20.dp))
            Column(Modifier.weight(1f)) {
                val tag = when {
                    playing == null -> stringResource(R.string.tv_home_last_played)
                    tidalId(playing.mediaId) != null -> stringResource(R.string.tv_home_playing_now) + " · " + stringResource(R.string.player_source_tidal)
                    else -> stringResource(R.string.tv_home_playing_now) + " · " + stringResource(R.string.player_source_local)
                }
                Text(tag.uppercase(), style = TvType.caps, color = look.accent2, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(playing?.title ?: last?.title.orEmpty(), style = TvType.section.copy(fontSize = 23.sp), color = look.ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
                val sub = if (playing != null) listOf(playing.artist, playing.album) else listOf(last?.artist.orEmpty(), last?.album.orEmpty())
                Text(sub.filter(String::isNotBlank).joinToString(" · "), style = TvType.body.copy(fontWeight = androidx.compose.ui.text.font.FontWeight.Normal), color = look.ink2, maxLines = 1, overflow = TextOverflow.Ellipsis)
                if (playing != null) {
                    Spacer(Modifier.height(8.dp))
                    TvProgress(if (durationMs > 0) positionMs.toFloat() / durationMs else 0f, Modifier.width(380.dp))
                    Text("${formatClock(positionMs)} / ${formatClock(durationMs)}", style = TvType.secondary, color = look.ink2)
                }
            }
            Column(verticalArrangement = Arrangement.spacedBy(9.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                if (playing != null) {
                    TvPill(
                        stringResource(R.string.tv_np_open),
                        onOpen,
                        Modifier.tvInitialFocus(firstFocus, entryKey).tvLeftToDrawer().width(170.dp),
                        icon = Icons.Rounded.PlayArrow,
                        primary = true,
                        onFocused = onFocused,
                    )
                    TvPill(
                        stringResource(if (isPlaying) R.string.player_pause else R.string.player_play),
                        onToggle,
                        Modifier.tvLeftToDrawer().width(170.dp),
                        icon = if (isPlaying) Icons.Rounded.Pause else Icons.Rounded.PlayArrow,
                        onFocused = onFocused,
                    )
                } else {
                    TvPill(
                        stringResource(R.string.player_play),
                        onResume,
                        Modifier.tvInitialFocus(firstFocus, entryKey).tvLeftToDrawer().width(170.dp),
                        icon = Icons.Rounded.PlayArrow,
                        primary = true,
                        onFocused = onFocused,
                    )
                }
            }
        }
    }
}

@Composable
private fun Shelf(title: String, subtitle: String? = null, content: androidx.compose.foundation.lazy.LazyListScope.() -> Unit) {
    val look = LocalTvLook.current
    Column {
        Row(Modifier.padding(bottom = 10.dp), verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Text(title, style = TvType.section, color = look.ink)
            if (subtitle != null) Text(subtitle, style = TvType.secondary, color = look.ink2, modifier = Modifier.padding(bottom = 2.dp))
        }
        // room around the row for the focused cover's growth, ring and glow (a lazy row clips)
        LazyRow(
            contentPadding = PaddingValues(start = 6.dp, end = 24.dp, top = 12.dp, bottom = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(TvDimens.gap),
            content = content,
        )
    }
}

@Composable
private fun EmptyHome(entryKey: Int, onOpenLibrary: () -> Unit) {
    TvEmptyState(
        title = stringResource(R.string.tv_empty_title),
        body = stringResource(R.string.tv_empty_body),
        cta = stringResource(R.string.tv_empty_cta),
        ctaIcon = Icons.Rounded.LibraryMusic,
        icon = Icons.Rounded.LibraryMusic,
        entryKey = entryKey,
        onCta = onOpenLibrary,
    )
}
