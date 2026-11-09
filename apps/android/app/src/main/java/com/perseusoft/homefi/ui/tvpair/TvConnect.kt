package com.perseusoft.homefi.ui.tvpair

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.QrCodeScanner
import androidx.compose.material.icons.rounded.Tv
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.pair.TvSendState

private val MIN_TARGET = 48.dp

/** "Connect to TV": opens the scanner; a scanned link pairs the TV (and sends the running ride, if any). */
@Composable
fun ConnectToTvButton(modifier: Modifier = Modifier, label: String = stringResource(R.string.tv_connect)) {
    val app = LocalContext.current.homefi
    var scanning by rememberSaveable { mutableStateOf(false) }
    FilledTonalButton(onClick = { scanning = true }, modifier = modifier.heightIn(min = MIN_TARGET)) {
        Icon(Icons.Rounded.QrCodeScanner, null, Modifier.size(18.dp))
        Text(label, Modifier.padding(start = 8.dp))
    }
    if (scanning) {
        TvScanDialog(onDismiss = { scanning = false }) { link ->
            scanning = false
            app.tvPairings.connect(link)
        }
    }
}

/** "Put HomeFi on your TV": opens the install helper. */
@Composable
fun PutOnTvButton(modifier: Modifier = Modifier) {
    var open by rememberSaveable { mutableStateOf(false) }
    TextButton(onClick = { open = true }, modifier = modifier.heightIn(min = MIN_TARGET)) {
        Icon(Icons.Rounded.Tv, null, Modifier.size(18.dp))
        Text(stringResource(R.string.tv_install_title), Modifier.padding(start = 8.dp))
    }
    if (open) TvInstallDialog { open = false }
}

/** The paired TV, in one line, with how the last send went. Nothing when no TV is paired. */
@Composable
fun PairedTvLine() {
    val app = LocalContext.current.homefi
    val paired by app.tvPairings.paired.collectAsStateWithLifecycle()
    val send by app.tvPairings.sendState.collectAsStateWithLifecycle()
    val tv = paired ?: return
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        Icon(Icons.Rounded.Tv, null, Modifier.size(18.dp), tint = MaterialTheme.colorScheme.primary)
        Text(
            when (send) {
                TvSendState.UNREACHABLE -> stringResource(R.string.tv_paired_unreachable, tv.name)
                TvSendState.SENDING -> stringResource(R.string.tv_paired_sending, tv.name)
                else -> stringResource(R.string.tv_paired, tv.name)
            },
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

/** The Train setup's TV card body: paired TV, connect, forget, and the install helper. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun TvPairingBlock() {
    val app = LocalContext.current.homefi
    val paired by app.tvPairings.paired.collectAsStateWithLifecycle()
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        PairedTvLine()
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            ConnectToTvButton(label = stringResource(if (paired == null) R.string.tv_connect else R.string.tv_connect_other))
            if (paired != null) TextButton(onClick = app.tvPairings::forget, modifier = Modifier.heightIn(min = MIN_TARGET)) { Text(stringResource(R.string.tv_forget)) }
            PutOnTvButton()
        }
    }
}
