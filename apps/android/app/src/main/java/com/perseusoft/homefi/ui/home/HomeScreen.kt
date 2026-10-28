package com.perseusoft.homefi.ui.home

import com.perseusoft.homefi.ui.theme.SkinHeading
import com.perseusoft.homefi.ui.theme.ListEntrance
import com.perseusoft.homefi.ui.theme.itemMotion
import androidx.compose.foundation.clickable
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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.Album
import androidx.compose.material.icons.rounded.Cloud
import androidx.compose.material.icons.rounded.PlayArrow
import com.perseusoft.homefi.ui.theme.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.LocalPlaylist
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.library.EmbeddedCover
import com.perseusoft.homefi.player.Recent
import com.perseusoft.homefi.player.isFilePath
import com.perseusoft.homefi.player.tidalId
import com.perseusoft.homefi.ui.HomefiLogo
import com.perseusoft.homefi.ui.library.CoverArt
import com.perseusoft.homefi.ui.share.ShareEntry
import java.util.Calendar

private fun Recent.cover(): Any? = if (isFilePath(mediaId)) EmbeddedCover(mediaId) else artworkUri

/** The main view: pick up where you left off. */
@Composable
fun HomeScreen() {
    val app = LocalContext.current.homefi
    val recents by app.player.recents.items.collectAsStateWithLifecycle()
    val library by app.library.state.collectAsStateWithLifecycle()

    // one card per album, in the order they were last heard
    val albums = remember(recents) {
        recents.filter { it.album.isNotBlank() }.distinctBy { "${it.artist}\u0000${it.album}".lowercase() }.take(12)
    }

    // the sections arrive one after another; shelves slide when the recents change
    ListEntrance {
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
        item(key = "greeting") {
            Text(
                stringResource(greeting()),
                style = MaterialTheme.typography.headlineSmall,
                fontWeight = FontWeight.Bold,
                modifier = itemMotion(0).padding(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 4.dp),
            )
        }
        // someone nearby sharing music, or the session you are in
        item(key = "share") { Box(itemMotion(1)) { ShareEntry() } }
        if (recents.isEmpty()) {
            item(key = "empty") { Box(itemMotion(2)) { EmptyHome() } }
        } else {
            item(key = "continue") { Box(itemMotion(2)) { ContinueCard(recents.first()) { app.player.playRecents(recents, 0) } } }
            item(key = "recent-songs") {
                Shelf(stringResource(R.string.home_recent_songs), itemMotion(3)) {
                    items(recents.take(20), key = { it.mediaId }) { r ->
                        Tile(r.cover(), r.title, r.artist, cloud = tidalId(r.mediaId) != null, modifier = itemMotion()) {
                            app.player.playRecents(recents, recents.indexOf(r))
                        }
                    }
                }
            }
            if (albums.isNotEmpty()) {
                item(key = "recent-albums") {
                    Shelf(stringResource(R.string.home_recent_albums), itemMotion(4)) {
                        items(albums, key = { "album-${it.mediaId}" }) { r ->
                            Tile(r.cover(), r.album, r.artist, cloud = tidalId(r.mediaId) != null, icon = Icons.Rounded.Album, modifier = itemMotion()) {
                                // the album's songs we've heard, local ones as a whole album
                                val local = library.tracks.filter { it.album.equals(r.album, true) && it.artist.equals(r.artist, true) }
                                if (local.isNotEmpty()) {
                                    app.player.play(local.sortedWith(compareBy({ it.discNumber }, { it.trackNumber })))
                                } else {
                                    val heard = recents.filter { it.album == r.album && it.artist == r.artist }
                                    app.player.playRecents(heard, 0)
                                }
                            }
                        }
                    }
                }
            }
        }
        if (library.playlists.isNotEmpty()) {
            item(key = "playlists") {
                Shelf(stringResource(R.string.library_playlists), itemMotion(5)) {
                    items(library.playlists.take(20), key = { it.path }) { p -> PlaylistTile(p, itemMotion()) }
                }
            }
        }
    }
    }
}

private fun greeting(): Int = when (Calendar.getInstance().get(Calendar.HOUR_OF_DAY)) {
    in 5..11 -> R.string.home_morning
    in 12..18 -> R.string.home_afternoon
    else -> R.string.home_evening
}

@Composable
private fun ContinueCard(r: Recent, onPlay: () -> Unit) {
    Card(
        Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp).clickable(onClick = onPlay),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh),
    ) {
        Row(Modifier.padding(12.dp), verticalAlignment = Alignment.CenterVertically) {
            CoverArt(r.cover(), 72.dp, radius = 10.dp)
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Text(stringResource(R.string.home_continue), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.primary)
                Text(r.title, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(
                    listOf(r.artist, r.album).filter(String::isNotBlank).joinToString(" · "),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            FilledIconButton(onClick = onPlay, modifier = Modifier.size(48.dp)) {
                Icon(Icons.Rounded.PlayArrow, stringResource(R.string.player_play))
            }
        }
    }
}

@Composable
private fun Shelf(title: String, modifier: Modifier = Modifier, content: androidx.compose.foundation.lazy.LazyListScope.() -> Unit) {
    Column(modifier.padding(top = 16.dp)) {
        SkinHeading(
            title,
            style = MaterialTheme.typography.titleMedium,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
        )
        LazyRow(
            contentPadding = PaddingValues(horizontal = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            content = content,
        )
    }
}

@Composable
private fun Tile(
    cover: Any?,
    title: String,
    subtitle: String,
    cloud: Boolean,
    icon: androidx.compose.ui.graphics.vector.ImageVector = Icons.Rounded.PlayArrow,
    modifier: Modifier = Modifier,
    onClick: () -> Unit,
) {
    Column(modifier.width(128.dp).clickable(onClick = onClick)) {
        Box {
            CoverArt(cover, 128.dp, radius = 10.dp, icon = icon)
            // streamed from Tidal, not on the device
            if (cloud) {
                Icon(
                    Icons.Rounded.Cloud,
                    stringResource(R.string.player_source_tidal),
                    tint = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.align(Alignment.TopEnd).padding(6.dp).size(16.dp),
                )
            }
        }
        Text(title, style = MaterialTheme.typography.bodyMedium, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 6.dp))
        Text(subtitle, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

@Composable
private fun PlaylistTile(p: LocalPlaylist, modifier: Modifier = Modifier) {
    val app = LocalContext.current.homefi
    val first = p.tracks.firstOrNull()
    Tile(first?.let(::EmbeddedCover), p.name, stringResource(R.string.library_count, p.tracks.size), cloud = false, icon = Icons.AutoMirrored.Rounded.QueueMusic, modifier = modifier) {
        val tracks = p.tracks.mapNotNull(app.library::find)
        if (tracks.isNotEmpty()) app.player.play(tracks)
    }
}

@Composable
private fun EmptyHome() {
    Column(
        Modifier.fillMaxWidth().padding(32.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        HomefiLogo(72.dp)
        Spacer(Modifier.height(16.dp))
        Text(
            stringResource(R.string.home_empty),
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
    }
}
