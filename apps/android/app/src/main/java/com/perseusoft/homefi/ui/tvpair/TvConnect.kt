package com.perseusoft.homefi.ui.tvpair

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Tv
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
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

/** The Train setup's TV card body: the paired TV, then "Connect the TV" (the guide) and forget. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun TvPairingBlock() {
    val app = LocalContext.current.homefi
    val paired by app.tvPairings.paired.collectAsStateWithLifecycle()
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        PairedTvLine()
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            ConnectTvButton(label = stringResource(if (paired == null) R.string.tv_connect else R.string.tv_connect_other))
            if (paired != null) TextButton(onClick = app.tvPairings::forget, modifier = Modifier.heightIn(min = MIN_TARGET)) { Text(stringResource(R.string.tv_forget)) }
        }
    }
}
