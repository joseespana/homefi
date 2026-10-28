package com.perseusoft.homefi.ui.downloads

import androidx.compose.runtime.mutableStateOf
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.lazy.itemsIndexed
import com.perseusoft.homefi.ui.theme.ListEntrance
import com.perseusoft.homefi.ui.theme.LocalMotion
import com.perseusoft.homefi.ui.theme.itemMotion
import androidx.compose.runtime.setValue
import com.perseusoft.homefi.ui.ConfirmDialog
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.DoNotDisturbOn
import androidx.compose.material.icons.rounded.ErrorOutline
import androidx.compose.material.icons.rounded.LibraryMusic
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.TaskAlt
import com.perseusoft.homefi.ui.theme.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import com.perseusoft.homefi.ui.library.TrackRow
import com.perseusoft.homefi.ui.library.PlayHeader
import com.perseusoft.homefi.data.Storage
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.data.ActiveTask
import com.perseusoft.homefi.data.LogEntry
import com.perseusoft.homefi.data.LogKind
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.formatBytes

@Composable
fun DownloadsScreen() {
    val context = LocalContext.current
    val app = context.homefi
    val repo = app.downloads
    val state by repo.state.collectAsStateWithLifecycle()
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val library by app.library.state.collectAsStateWithLifecycle()
    val playing by app.player.state.collectAsStateWithLifecycle()

    // everything in the download folder, whoever put it there (tiddl, a computer, HomeFi)
    val folder = remember(settings.downloadDir) { Storage.downloadDir(context, settings.downloadDir).absolutePath }
    val inFolder = remember(library.tracks, folder) { library.tracks.filter { it.path.startsWith("$folder/") } }

    // the empty state and the list fade through each other
    val motion = LocalMotion.current
    val empty = !state.isBusy && state.log.isEmpty() && inFolder.isEmpty()
    AnimatedContent(targetState = empty, transitionSpec = { motion.fadeThrough() }, label = "downloads") { isEmpty ->
    if (isEmpty) {
        Column(
            Modifier
                .fillMaxSize()
                .padding(32.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center,
        ) {
            Icon(Icons.Rounded.LibraryMusic, null, Modifier.size(64.dp), tint = MaterialTheme.colorScheme.primary)
            Spacer(Modifier.size(16.dp))
            Text(stringResource(R.string.downloads_empty), color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    } else {
    ListEntrance {
    LazyColumn(Modifier.fillMaxSize()) {
        if (state.isBusy || state.log.isNotEmpty()) item(key = "summary") {
            Card(
                itemMotion(0)
                    .fillMaxWidth()
                    .padding(16.dp),
                colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh),
            ) {
                Column(Modifier.padding(16.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(
                            if (state.isBusy) stringResource(R.string.downloads_running) else stringResource(R.string.downloads_idle),
                            style = MaterialTheme.typography.titleMedium,
                            fontWeight = FontWeight.Bold,
                            modifier = Modifier.weight(1f),
                        )
                        if (state.isBusy) {
                            TextButton(onClick = repo::cancelAll) { Text(stringResource(R.string.cancel)) }
                        } else {
                            var clearing by remember { mutableStateOf(false) }
                            if (clearing) {
                                ConfirmDialog(
                                    title = stringResource(R.string.downloads_clear_title),
                                    text = stringResource(R.string.downloads_clear_text),
                                    confirm = stringResource(R.string.clear),
                                    onConfirm = repo::clearLog,
                                    onDismiss = { clearing = false },
                                )
                            }
                            TextButton(onClick = { clearing = true }) { Text(stringResource(R.string.clear)) }
                        }
                    }
                    Text(
                        stringResource(R.string.downloads_counter, state.finished, state.queued) +
                            if (state.pending.isNotEmpty()) " · " + stringResource(R.string.downloads_pending, state.pending.size) else "",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    if (state.isBusy) {
                        Spacer(Modifier.size(12.dp))
                        if (state.queued == 0) {
                            LinearProgressIndicator(Modifier.fillMaxWidth())
                        } else {
                            // glides to each new count instead of jumping
                            val done by animateFloatAsState(state.finished.toFloat() / state.queued, motion.tween(400), label = "downloads-done")
                            LinearProgressIndicator(
                                progress = { done },
                                modifier = Modifier.fillMaxWidth(),
                            )
                        }
                    }
                }
            }
        }

        // a task finishing slides into the log; new ones fade in
        items(state.active.values.toList(), key = { "task-${it.task}" }) { ActiveRow(it, itemMotion(1)) }
        itemsIndexed(state.log, key = { _, it -> "log-${it.id}" }) { i, entry -> LogRow(entry, itemMotion(i + 2)) }

        if (inFolder.isNotEmpty()) {
            item(key = "folder") {
                Column(itemMotion().padding(top = 8.dp)) {
                    Text(
                        stringResource(R.string.downloads_folder),
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.Bold,
                        modifier = Modifier.padding(horizontal = 16.dp),
                    )
                    Text(
                        folder,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.StartEllipsis,
                        modifier = Modifier.padding(horizontal = 16.dp),
                    )
                    PlayHeader(stringResource(R.string.library_count, inFolder.size), inFolder)
                }
            }
            items(inFolder.size, key = { "file-${inFolder[it].path}" }) { i ->
                TrackRow(inFolder[i], current = inFolder[i].path == playing.current?.mediaId, index = i, modifier = itemMotion()) { app.player.play(inFolder, i) }
            }
        }
    }
    }
    }
    }
}

@Composable
private fun ActiveRow(task: ActiveTask, modifier: Modifier = Modifier) {
    val motion = LocalMotion.current
    ListItem(
        modifier = modifier,
        colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surface),
        headlineContent = { Text(task.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        supportingContent = {
            Column {
                val total = task.total
                Text(
                    "${task.quality} · ${formatBytes(task.downloaded)}" + (total?.let { " / ${formatBytes(it)}" } ?: ""),
                    style = MaterialTheme.typography.bodySmall,
                )
                Spacer(Modifier.size(6.dp))
                if (total != null && total > 0) {
                    val got by animateFloatAsState(task.downloaded.toFloat() / total, motion.tween(250), label = "task-progress")
                    LinearProgressIndicator(progress = { got }, modifier = Modifier.fillMaxWidth())
                } else {
                    LinearProgressIndicator(Modifier.fillMaxWidth())
                }
            }
        },
    )
}

private fun LogKind.icon(): ImageVector = when (this) {
    LogKind.DONE -> Icons.Rounded.CheckCircle
    LogKind.EXISTS -> Icons.Rounded.TaskAlt
    LogKind.SKIPPED -> Icons.Rounded.DoNotDisturbOn
    LogKind.ERROR -> Icons.Rounded.ErrorOutline
}

@Composable
private fun LogRow(entry: LogEntry, modifier: Modifier = Modifier) {
    val tint = when (entry.kind) {
        LogKind.DONE -> MaterialTheme.colorScheme.primary
        LogKind.EXISTS -> MaterialTheme.colorScheme.secondary
        LogKind.SKIPPED -> MaterialTheme.colorScheme.onSurfaceVariant
        LogKind.ERROR -> MaterialTheme.colorScheme.error
    }
    val player = LocalContext.current.homefi.player
    ListItem(
        modifier = if (entry.path != null) modifier.clickable { player.playFile(entry.path) } else modifier,
        colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surface),
        leadingContent = { Icon(entry.kind.icon(), null, tint = tint) },
        headlineContent = { Text(entry.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        supportingContent = {
            Text(entry.detail, maxLines = 2, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.bodySmall)
        },
        trailingContent = entry.path?.let {
            { Icon(Icons.Rounded.PlayArrow, stringResource(R.string.player_play), tint = MaterialTheme.colorScheme.primary) }
        },
    )
}
