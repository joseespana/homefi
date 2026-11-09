package com.perseusoft.homefi.ui.exercise

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material.icons.rounded.FileDownload
import androidx.compose.material.icons.rounded.FileUpload
import androidx.compose.material.icons.rounded.IosShare
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.PushPin
import androidx.compose.material.icons.rounded.Star
import androidx.compose.material.icons.rounded.StarBorder
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.SavedRideSummary
import com.perseusoft.homefi.data.FileExchange
import com.perseusoft.homefi.data.SavedRides
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.ConfirmDeleteDialog
import com.perseusoft.homefi.ui.NameDialog
import com.perseusoft.homefi.ui.haptics
import com.perseusoft.homefi.ui.rememberFileExchange
import com.perseusoft.homefi.ui.theme.Card
import kotlinx.coroutines.launch

/** Rides shown before "Show all". */
private const val FIRST_RIDES = 5

private val MOUNTAIN_WIDTH = 72.dp
private val MOUNTAIN_HEIGHT = 44.dp
private val MIN_TARGET = 48.dp

/** What the rider chose to do with one saved ride from its menu. */
private sealed interface RideDialog {
    val ride: SavedRideSummary

    data class Rename(override val ride: SavedRideSummary) : RideDialog
    data class Delete(override val ride: SavedRideSummary) : RideDialog
}

/**
 * "My rides" on the Train tab: every ride built so far (the app keeps them by itself, the last few dozen
 * unless kept), favorites first. Riding one again needs no new build and no AI. Export and import use the
 * system pickers (local storage, Google Drive) and the share sheet.
 */
@Composable
fun MyRides() {
    val app = LocalContext.current.homefi
    val rides by app.rides.list.collectAsStateWithLifecycle()
    val haptics = haptics()
    val scope = rememberCoroutineScope()
    var all by rememberSaveable { mutableStateOf(false) }
    var dialog by remember { mutableStateOf<RideDialog?>(null) }
    LaunchedEffect(Unit) { app.rides.refresh() }

    val exchange = rememberFileExchange { text -> scope.launch { app.rides.import(text) } }

    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text(stringResource(R.string.rides_title), style = MaterialTheme.typography.titleMedium)
                Text(
                    stringResource(R.string.rides_sub),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            TextButton(onClick = exchange.pick, modifier = Modifier.heightIn(min = MIN_TARGET)) {
                Icon(Icons.Rounded.FileUpload, null, Modifier.size(18.dp))
                Text(stringResource(R.string.rides_import), Modifier.padding(start = 6.dp))
            }
        }
        if (rides.isEmpty()) {
            Text(
                stringResource(R.string.rides_empty),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(vertical = 8.dp),
            )
        }
        val shown = if (all) rides else rides.take(FIRST_RIDES)
        shown.forEach { ride ->
            key(ride.id) {
                RideCard(
                    ride = ride,
                    onRide = { haptics.tick(); app.ride.rideAgain(ride.id) },
                    onFavorite = { haptics.tick(); app.rides.setFavorite(ride.id, !ride.favorite) },
                    onKeep = { scope.launch { app.rides.keep(ride.id) } },
                    onRename = { dialog = RideDialog.Rename(ride) },
                    onDelete = { dialog = RideDialog.Delete(ride) },
                    onSaveAs = {
                        scope.launch {
                            app.rides.export(ride.id)?.let { exchange.saveAs(FileExchange.fileName(ride.name, SavedRides.extension), it) }
                        }
                    },
                    onShare = {
                        scope.launch {
                            app.rides.export(ride.id)?.let { exchange.share(FileExchange.fileName(ride.name, SavedRides.extension), it) }
                        }
                    },
                )
            }
        }
        if (rides.size > FIRST_RIDES) {
            TextButton(onClick = { all = !all }, modifier = Modifier.align(Alignment.CenterHorizontally).heightIn(min = MIN_TARGET)) {
                Text(if (all) stringResource(R.string.rides_less) else stringResource(R.string.rides_all, rides.size))
            }
        }
    }

    when (val d = dialog) {
        is RideDialog.Rename -> NameDialog(
            title = stringResource(R.string.rides_rename),
            initial = d.ride.name,
            confirm = stringResource(R.string.rides_save),
            onConfirm = { name -> dialog = null; scope.launch { app.rides.rename(d.ride.id, name) } },
            onDismiss = { dialog = null },
        )
        is RideDialog.Delete -> ConfirmDeleteDialog(
            title = stringResource(R.string.rides_delete_title),
            text = stringResource(R.string.rides_delete_text, d.ride.name),
            onConfirm = { dialog = null; app.rides.delete(d.ride.id) },
            onDismiss = { dialog = null },
        )
        null -> Unit
    }
}

@Composable
private fun RideCard(
    ride: SavedRideSummary,
    onRide: () -> Unit,
    onFavorite: () -> Unit,
    onKeep: () -> Unit,
    onRename: () -> Unit,
    onDelete: () -> Unit,
    onSaveAs: () -> Unit,
    onShare: () -> Unit,
) {
    Card(onClick = onRide, modifier = Modifier.fillMaxWidth()) {
        Row(
            Modifier.padding(start = 12.dp, top = 8.dp, bottom = 8.dp, end = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            MountainProfile(ride.profile, marker = null, modifier = Modifier.size(MOUNTAIN_WIDTH, MOUNTAIN_HEIGHT))
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(ride.name, style = MaterialTheme.typography.titleSmall, maxLines = 2, overflow = TextOverflow.Ellipsis)
                Text(
                    rideLine(ride),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    if (ride.timesRidden > 0u) {
                        Text(
                            pluralStringResource(R.plurals.rides_times, ride.timesRidden.toInt(), ride.timesRidden.toInt()),
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    if (ride.auto) {
                        Text(
                            stringResource(R.string.rides_auto),
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.tertiary,
                        )
                    }
                }
            }
            IconButton(onClick = onFavorite, modifier = Modifier.size(MIN_TARGET)) {
                Icon(
                    if (ride.favorite) Icons.Rounded.Star else Icons.Rounded.StarBorder,
                    stringResource(if (ride.favorite) R.string.rides_unfavorite else R.string.rides_favorite),
                    tint = if (ride.favorite) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        Row(
            Modifier.fillMaxWidth().padding(start = 12.dp, end = 4.dp, bottom = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            FilledTonalButton(onClick = onRide, modifier = Modifier.heightIn(min = MIN_TARGET)) {
                Icon(Icons.Rounded.PlayArrow, null, Modifier.size(18.dp))
                Text(stringResource(R.string.rides_again), Modifier.padding(start = 6.dp))
            }
            Box(Modifier.weight(1f))
            RideMenu(ride, onKeep, onRename, onSaveAs, onShare, onDelete)
        }
    }
}

@Composable
private fun RideMenu(
    ride: SavedRideSummary,
    onKeep: () -> Unit,
    onRename: () -> Unit,
    onSaveAs: () -> Unit,
    onShare: () -> Unit,
    onDelete: () -> Unit,
) {
    var open by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { open = true }, modifier = Modifier.size(MIN_TARGET)) {
            Icon(Icons.Rounded.MoreVert, stringResource(R.string.rides_more))
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            if (ride.auto) MenuEntry(R.string.rides_keep, Icons.Rounded.PushPin) { open = false; onKeep() }
            MenuEntry(R.string.rides_rename, Icons.Rounded.Edit) { open = false; onRename() }
            MenuEntry(R.string.rides_save_file, Icons.Rounded.FileDownload) { open = false; onSaveAs() }
            MenuEntry(R.string.rides_share, Icons.Rounded.IosShare) { open = false; onShare() }
            MenuEntry(R.string.xfer_delete, Icons.Rounded.DeleteOutline) { open = false; onDelete() }
        }
    }
}

@Composable
private fun MenuEntry(label: Int, icon: androidx.compose.ui.graphics.vector.ImageVector, onClick: () -> Unit) {
    DropdownMenuItem(
        text = { Text(stringResource(label)) },
        leadingIcon = { Icon(icon, null) },
        onClick = onClick,
        modifier = Modifier.heightIn(min = MIN_TARGET),
    )
}

/** "30 min · Classic · Rock + Metal". */
@Composable
internal fun rideLine(ride: SavedRideSummary): String {
    val parts = buildList {
        add(stringResource(R.string.ex_minutes_value, ride.minutes.toInt()))
        add(stringResource(ride.style.labelRes()))
        if (ride.genres.isNotEmpty()) {
            add(ride.genres.map { token -> Genre.ofToken(token)?.let { stringResource(it.label) } ?: token.replaceFirstChar(Char::titlecase) }.joinToString(" + "))
        }
    }
    return parts.joinToString(" · ")
}
