package com.perseusoft.homefi.ui.playlists

import androidx.annotation.StringRes
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.ExpandLess
import androidx.compose.material.icons.rounded.ExpandMore
import androidx.compose.material.icons.rounded.Link
import androidx.compose.material.icons.rounded.PhoneAndroid
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material.icons.rounded.Shuffle
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.ConcertMode
import com.perseusoft.homefi.core.ConcertShow
import com.perseusoft.homefi.core.HomePlaylist
import com.perseusoft.homefi.core.PlaylistEntry
import com.perseusoft.homefi.core.SetlistSource
import com.perseusoft.homefi.core.TasteScope
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.library.EmbeddedCover
import com.perseusoft.homefi.ui.QueueButton
import com.perseusoft.homefi.ui.formatDuration
import com.perseusoft.homefi.ui.haptics
import com.perseusoft.homefi.ui.library.CoverArt
import com.perseusoft.homefi.ui.theme.Button
import com.perseusoft.homefi.ui.theme.Card
import com.perseusoft.homefi.ui.theme.FilledTonalButton
import com.perseusoft.homefi.ui.theme.itemMotion
import com.perseusoft.homefi.ui.theme.numeric
import com.perseusoft.homefi.ui.theme.skinRowColors

private const val COVER_SIZE_DP = 48
private const val DIMMED_ALPHA = 0.6f

/** What the preview can ask for; the screen that owns the playlist decides what each does. */
class PreviewActions(
    /** True for a stored playlist (Search again, Delete); false for a draft (Save, Discard). */
    val saved: Boolean,
    val onSave: () -> Unit = {},
    val onDiscard: () -> Unit = {},
    val onSearchAgain: () -> Unit = {},
    val onDelete: () -> Unit = {},
    /** Another view of the stored shows: one show or the whole tour. */
    val onMode: (ConcertMode) -> Unit = {},
    /** The words on the saved playlist's rebuild button ("Search again", "Change answers"). */
    @StringRes val searchAgainLabel: Int = R.string.pl_search_again,
)

/**
 * A playlist the core built (a draft or a stored one) as rows of a [LazyListScope]: header,
 * buttons, shows, songs and the ones that weren't found. Taps play from that row.
 */
@OptIn(ExperimentalFoundationApi::class)
fun LazyListScope.playlistPreview(
    playlist: HomePlaylist,
    fallbackNote: String?,
    aiName: String?,
    actions: PreviewActions,
    showsOpen: Boolean,
    onToggleShows: () -> Unit,
) {
    item(key = "head") {
        PreviewHeader(playlist, fallbackNote, aiName, actions)
    }
    val shows = playlist.concert?.shows.orEmpty()
    if (shows.isNotEmpty()) {
        item(key = "shows-head") {
            val whole = playlist.concert?.mode is ConcertMode.Show
            ListItem(
                modifier = Modifier.heightIn(min = MIN_TOUCH_DP.dp).clickable(onClick = onToggleShows),
                headlineContent = { Text(stringResource(R.string.pl_shows_title) + " (${shows.size})", fontWeight = FontWeight.Bold) },
                trailingContent = {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        if (whole) TextButton(onClick = { actions.onMode(ConcertMode.Tour) }) { Text(stringResource(R.string.pl_whole_tour)) }
                        Icon(if (showsOpen) Icons.Rounded.ExpandLess else Icons.Rounded.ExpandMore, null)
                    }
                },
            )
        }
        if (showsOpen) {
            items(shows.size, key = { "show-${shows[it].id}" }) { index ->
                ShowRow(shows[index]) { actions.onMode(ConcertMode.Show(shows[index].id)) }
            }
        }
    }
    items(playlist.entries.size, key = { index -> "e$index-" + playlist.entries[index].let { it.tidalId ?: it.localPath ?: it.title } }) { index ->
        EntryRow(playlist, index, itemMotion(index))
    }
    if (playlist.unmatched.isNotEmpty()) {
        item(key = "unmatched-head") {
            Text(
                stringResource(R.string.pl_unmatched_title, playlist.unmatched.size),
                style = MaterialTheme.typography.titleSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 20.dp, bottom = 4.dp),
            )
        }
        items(playlist.unmatched.size, key = { "u$it-" + playlist.unmatched[it].title }) { index ->
            val song = playlist.unmatched[index]
            ListItem(
                modifier = Modifier.alpha(DIMMED_ALPHA),
                headlineContent = { Text(song.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                supportingContent = {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        NotFoundChip()
                        Spacer(Modifier.width(6.dp))
                        Text(listOf(song.artist, song.note).filter(String::isNotBlank).joinToString(" · "), maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                },
            )
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun PreviewHeader(playlist: HomePlaylist, fallbackNote: String?, aiName: String?, actions: PreviewActions) {
    val app = LocalContext.current.homefi
    val player = app.player
    val haptics = haptics()
    val settings by app.settings.playlists.collectAsStateWithLifecycle()
    val library by app.library.state.collectAsStateWithLifecycle()
    val find = app.library::find
    var sourcesOpen by rememberSaveable { mutableStateOf(false) }
    var confirmDelete by rememberSaveable { mutableStateOf(false) }

    val origin = playlist.concert
    val popular = origin?.source is SetlistSource.TidalPopular
    val playable = remember(playlist, library.tracks) { playlist.mediaItems(find).isNotEmpty() }
    val totalMs = remember(playlist) { playlist.entries.sumOf { it.durationMs.toLong() } }
    val widget = playlist.searchWidgetHtml
    val hasSources = (settings.showSources && playlist.sources.isNotEmpty()) || widget != null

    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
        if (popular && origin != null) {
            Text(stringResource(R.string.pl_popular_header, origin.artist), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.size(4.dp))
            NotRealSetlistBadge()
            Spacer(Modifier.size(8.dp))
        }
        Text(playlist.name, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold)
        if (playlist.description.isNotBlank()) {
            Text(playlist.description, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Text(
            stringResource(R.string.pl_count, playlist.entries.size, formatDuration(totalMs)),
            style = MaterialTheme.typography.bodyMedium.numeric(),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        if (fallbackNote != null) {
            Spacer(Modifier.size(8.dp))
            Card(Modifier.fillMaxWidth(), colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.secondaryContainer)) {
                Text(fallbackNote, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.padding(12.dp))
            }
        }
        if (widget != null) WidgetSection(widget, Modifier.padding(horizontal = 0.dp))
        Spacer(Modifier.size(8.dp))
        // icons next to Play: the row has to fit a DAP's narrow screen
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(
                onClick = { haptics.press(); player.playPlaylist(playlist, find) },
                enabled = playable,
                modifier = Modifier.weight(1f),
            ) {
                Icon(Icons.Rounded.PlayArrow, null, Modifier.size(ButtonDefaults.IconSize))
                Spacer(Modifier.size(ButtonDefaults.IconSpacing))
                Text(stringResource(R.string.library_play_all), maxLines = 1)
            }
            FilledTonalButton(onClick = { haptics.press(); player.playPlaylist(playlist, find, shuffle = true) }, enabled = playable) {
                Icon(Icons.Rounded.Shuffle, stringResource(R.string.library_shuffle))
            }
            QueueButton({ next -> player.enqueuePlaylist(playlist, find, next) }, enabled = playable)
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = { app.sound.openMenu(TasteScope.Playlist(playlist.id, playlist.name)) }, modifier = Modifier.size(MIN_TOUCH_DP.dp)) {
                Icon(Icons.Rounded.Tune, stringResource(R.string.sound_for))
            }
            if (hasSources) {
                TextButton(onClick = { sourcesOpen = true }) {
                    Icon(Icons.Rounded.Link, null, Modifier.size(ButtonDefaults.IconSize))
                    Spacer(Modifier.size(ButtonDefaults.IconSpacing))
                    Text(stringResource(R.string.pl_sources))
                }
            }
        }
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            if (actions.saved) {
                OutlinedButton(onClick = actions.onSearchAgain) {
                    Icon(Icons.Rounded.Refresh, null, Modifier.size(ButtonDefaults.IconSize))
                    Spacer(Modifier.size(ButtonDefaults.IconSpacing))
                    Text(stringResource(actions.searchAgainLabel))
                }
                TextButton(onClick = { confirmDelete = true }) {
                    Icon(Icons.Rounded.Delete, null, Modifier.size(ButtonDefaults.IconSize))
                    Spacer(Modifier.size(ButtonDefaults.IconSpacing))
                    Text(stringResource(R.string.pl_delete))
                }
            } else {
                Button(onClick = { haptics.confirm(); actions.onSave() }) { Text(stringResource(R.string.pl_save)) }
                TextButton(onClick = actions.onDiscard) { Text(stringResource(R.string.pl_discard)) }
            }
        }
        if (actions.saved) {
            Text(
                stringResource(R.string.pl_only_in_homefi),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp),
            )
        }
    }
    if (sourcesOpen) SourcesSheet(playlist, aiName, settings.showSources) { sourcesOpen = false }
    if (confirmDelete) {
        AlertDialog(
            onDismissRequest = { confirmDelete = false },
            title = { Text(stringResource(R.string.pl_delete)) },
            text = { Text(stringResource(R.string.pl_delete_confirm, playlist.name)) },
            confirmButton = { TextButton(onClick = { confirmDelete = false; actions.onDelete() }) { Text(stringResource(R.string.pl_delete)) } },
            dismissButton = { TextButton(onClick = { confirmDelete = false }) { Text(stringResource(R.string.pl_cancel)) } },
        )
    }
}

@Composable
private fun ShowRow(show: ConcertShow, onlyThis: () -> Unit) {
    ListItem(
        headlineContent = {
            Text(
                listOf(show.date, listOf(show.venue, show.city).filter(String::isNotBlank).joinToString(", ")).filter(String::isNotBlank).joinToString(" · "),
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        },
        trailingContent = { TextButton(onClick = onlyThis) { Text(stringResource(R.string.pl_only_this_show)) } },
    )
}

@Composable
private fun EntryRow(playlist: HomePlaylist, index: Int, modifier: Modifier) {
    val app = LocalContext.current.homefi
    val haptics = haptics()
    val entry: PlaylistEntry = playlist.entries[index]
    val localTrack = entry.localPath?.let(app.library::find)
    val playable = localTrack != null || entry.tidalId != null
    ListItem(
        modifier = modifier
            .alpha(if (playable) 1f else DIMMED_ALPHA)
            .clickable(enabled = playable) {
                haptics.press()
                app.player.playPlaylist(playlist, app.library::find, entryIndex = index)
            },
        colors = skinRowColors(index, current = false),
        leadingContent = {
            CoverArt(if (localTrack != null) EmbeddedCover(localTrack.path) else entry.coverUrl, COVER_SIZE_DP.dp)
        },
        headlineContent = { Text(entry.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        supportingContent = {
            Column {
                Text(entry.artist, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Row(verticalAlignment = Alignment.CenterVertically) {
                    if (localTrack != null) {
                        Icon(Icons.Rounded.PhoneAndroid, stringResource(R.string.pl_local), Modifier.size(16.dp))
                        Spacer(Modifier.width(4.dp))
                    }
                    TagChip(entry.tag)
                    if (entry.tag != com.perseusoft.homefi.core.EntryTag.PLAIN) Spacer(Modifier.width(6.dp))
                    Text(entry.note, style = MaterialTheme.typography.bodySmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
        },
        trailingContent = if (entry.durationMs > 0u) {
            { Text(formatDuration(entry.durationMs.toLong()), style = MaterialTheme.typography.bodySmall.numeric()) }
        } else {
            null
        },
    )
}
