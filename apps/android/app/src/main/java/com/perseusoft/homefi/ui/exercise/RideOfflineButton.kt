package com.perseusoft.homefi.ui.exercise

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CloudDone
import androidx.compose.material.icons.rounded.Download
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.RideSongSource
import com.perseusoft.homefi.data.RideOffline
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.ButtonSpinner
import com.perseusoft.homefi.ui.haptics

private val MIN_TARGET = 48.dp
private val ICON_SIZE = 18.dp

/**
 * "Download for offline" of one ride ([key]: its saved id, else its playlist id): queues the TIDAL songs that are not
 * on the phone under the ride's [name], shows "Downloading 3 of 12" with a spinner while they come (it can't be pressed
 * twice), and "Ready offline" once every song plays from a file. Nothing shows for a ride with no song to play.
 */
@Composable
fun RideOfflineButton(key: String, name: String, songs: List<RideSongSource>, modifier: Modifier = Modifier) {
    val app = LocalContext.current.homefi
    val haptics = haptics()
    val flow = remember(key, songs) { app.rideDownloads.state(key, songs) }
    val offline by flow.collectAsStateWithLifecycle(RideOffline.Empty)
    when (val state = offline) {
        RideOffline.Empty -> Unit
        RideOffline.Ready -> {
            val description = stringResource(R.string.rides_offline_ready_desc)
            Row(
                modifier.heightIn(min = MIN_TARGET).semantics { contentDescription = description },
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Icon(Icons.Rounded.CloudDone, null, Modifier.size(ICON_SIZE), tint = MaterialTheme.colorScheme.primary)
                Text(stringResource(R.string.rides_offline_ready), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
            }
        }
        is RideOffline.Missing -> {
            val description = pluralStringResource(R.plurals.rides_offline_missing, state.count, state.count)
            OutlinedButton(
                onClick = { haptics.press(); app.rideDownloads.download(key, name, songs) },
                modifier = modifier.heightIn(min = MIN_TARGET).semantics { contentDescription = description },
            ) {
                Icon(Icons.Rounded.Download, null, Modifier.size(ICON_SIZE))
                Text(stringResource(R.string.rides_offline_get), Modifier.padding(start = 6.dp))
            }
        }
        is RideOffline.Downloading -> OutlinedButton(onClick = {}, enabled = false, modifier = modifier.heightIn(min = MIN_TARGET)) {
            ButtonSpinner(size = ICON_SIZE)
            val label = if (state.started) {
                // the song being downloaded now counts: "Downloading 1 of 12" from the start
                stringResource(R.string.rides_offline_progress, (state.done + 1).coerceAtMost(state.of), state.of)
            } else {
                stringResource(R.string.rides_offline_waiting)
            }
            Text(label, Modifier.padding(start = 6.dp))
        }
    }
}
