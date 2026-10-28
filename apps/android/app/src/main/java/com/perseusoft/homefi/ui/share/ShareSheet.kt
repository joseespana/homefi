package com.perseusoft.homefi.ui.share

import androidx.compose.runtime.remember
import com.perseusoft.homefi.ui.ConfirmDialog
import androidx.compose.material3.Checkbox
import androidx.compose.foundation.clickable
import com.perseusoft.homefi.core.ShareMember
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Block
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.DirectionsCar
import androidx.compose.material.icons.rounded.PersonRemove
import androidx.compose.material.icons.rounded.Speaker
import com.perseusoft.homefi.ui.theme.Button
import androidx.compose.material3.ButtonDefaults
import com.perseusoft.homefi.ui.theme.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import com.perseusoft.homefi.ui.SelectChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import com.perseusoft.homefi.ui.theme.OutlinedButton
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.animateColorAsState
import androidx.compose.ui.graphics.Color
import com.perseusoft.homefi.ui.theme.LocalMotion
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.share.GuestStatus
import com.perseusoft.homefi.share.HubUi
import com.perseusoft.homefi.share.net.ShareNetworkMode
import com.perseusoft.homefi.share.net.rememberSharePermissionRequest
import com.perseusoft.homefi.ui.login.QrCode

/** "Share" in the player: start a hub (car / party), then the QR, guests and approvals. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ShareSheet(onDismiss: () -> Unit) {
    val share = LocalContext.current.homefi.share
    val hub by share.hub.collectAsStateWithLifecycle()
    val guest by share.guest.collectAsStateWithLifecycle()
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(
            Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp)
                .padding(bottom = 24.dp),
        ) {
            // starting a hub turns the sheet into its QR and guests, fading through
            val motion = LocalMotion.current
            val view = when {
                hub.running -> 0
                guest.status == GuestStatus.CONNECTED -> 1
                else -> 2
            }
            AnimatedContent(targetState = view, transitionSpec = { motion.swap() }, label = "share-sheet") { shown ->
                Column {
                    when (shown) {
                        0 -> RunningHub(hub)
                        1 -> Text(
                            stringResource(R.string.sharing_busy_guest, guest.sessionName),
                            style = MaterialTheme.typography.bodyLarge,
                            modifier = Modifier.padding(vertical = 16.dp),
                        )
                        else -> StartHub(hub)
                    }
                }
            }
        }
    }
}

@Composable
private fun StartHub(hub: HubUi) {
    val app = LocalContext.current.homefi
    val motion = LocalMotion.current
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    var party by rememberSaveable { mutableStateOf(settings.share.partyDefault) }
    var mode by rememberSaveable { mutableStateOf(settings.share.network) }
    var denied by rememberSaveable { mutableStateOf(false) }
    val askPermissions = rememberSharePermissionRequest { granted ->
        denied = !granted
        if (granted) app.share.startHub(party, mode)
    }

    Text(stringResource(R.string.sharing_title), style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold)
    Text(
        stringResource(R.string.sharing_as, app.share.sessionName()),
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    Spacer(Modifier.height(16.dp))
    ModeCard(!party, Icons.Rounded.DirectionsCar, R.string.sharing_mode_control, R.string.sharing_mode_control_hint) { party = false }
    Spacer(Modifier.height(8.dp))
    ModeCard(party, Icons.Rounded.Speaker, R.string.sharing_mode_party, R.string.sharing_mode_party_hint) { party = true }

    Spacer(Modifier.height(16.dp))
    Text(stringResource(R.string.sharing_network), style = MaterialTheme.typography.labelLarge)
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        ShareNetworkMode.entries.forEach { m ->
            SelectChip(selected = mode == m, onClick = { mode = m }, label = { Text(stringResource(m.label())) })
        }
    }
    AnimatedContent(targetState = mode, transitionSpec = { motion.swap() }, label = "network-hint") { m ->
        Text(
            stringResource(m.hint()),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }

    val error = hub.error ?: if (denied) stringResource(R.string.sharing_permission_denied) else null
    if (error != null) {
        Spacer(Modifier.height(12.dp))
        Text(error, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium)
    }
    Spacer(Modifier.height(20.dp))
    Button(
        onClick = {
            app.share.clearHubError()
            if (mode == ShareNetworkMode.LAN) app.share.startHub(party, mode) else askPermissions()
        },
        enabled = !hub.starting,
        modifier = Modifier.fillMaxWidth().height(56.dp),
    ) {
        if (hub.starting) {
            CircularProgressIndicator(Modifier.size(22.dp), strokeWidth = 2.dp, color = MaterialTheme.colorScheme.onPrimary)
            Spacer(Modifier.width(12.dp))
            Text(stringResource(R.string.sharing_starting))
        } else {
            Text(stringResource(R.string.sharing_start), style = MaterialTheme.typography.titleMedium)
        }
    }
}

@Composable
private fun ModeCard(selected: Boolean, icon: ImageVector, title: Int, hint: Int, onClick: () -> Unit) {
    // the picked mode lights up (and the other dims) instead of switching at once
    val motion = LocalMotion.current
    val fill by animateColorAsState(
        if (selected) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceContainerHigh,
        motion.color(),
        label = "mode-fill",
    )
    val edge by animateColorAsState(if (selected) MaterialTheme.colorScheme.primary else Color.Transparent, motion.color(), label = "mode-edge")
    Card(
        onClick = onClick,
        colors = CardDefaults.cardColors(containerColor = fill),
        border = BorderStroke(2.dp, edge),
        modifier = Modifier.fillMaxWidth().heightIn(min = 72.dp),
    ) {
        Row(Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(icon, null, Modifier.size(32.dp), tint = MaterialTheme.colorScheme.primary)
            Spacer(Modifier.width(16.dp))
            Column {
                Text(stringResource(title), style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold)
                Text(stringResource(hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

@Composable
private fun RunningHub(hub: HubUi) {
    val share = LocalContext.current.homefi.share
    val info = hub.info ?: return
    Text(info.sessionName, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
    Text(
        stringResource(if (hub.party) R.string.sharing_mode_party else R.string.sharing_mode_control),
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.primary,
    )
    Spacer(Modifier.height(16.dp))
    Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
        hub.qr?.let { QrCode(it, 240.dp) }
        Spacer(Modifier.height(12.dp))
        Text(stringResource(R.string.sharing_code), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Text(
            info.code,
            style = MaterialTheme.typography.displayMedium,
            fontWeight = FontWeight.Bold,
            fontFamily = FontFamily.Monospace,
            letterSpacing = 8.sp,
        )
        hub.network?.networkName?.let { name ->
            // for devices without a camera: join this Wi-Fi from their settings, HomeFi finds the rest
            Text(
                stringResource(R.string.sharing_wifi, name, hub.network.passphrase.orEmpty()),
                style = MaterialTheme.typography.titleSmall,
                fontWeight = FontWeight.Bold,
            )
            Text(
                stringResource(R.string.sharing_wifi_no_camera),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
            )
        }
        Text(
            stringResource(R.string.sharing_address, info.host, info.port.toInt()),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }

    Spacer(Modifier.height(12.dp))
    Row(Modifier.fillMaxWidth().heightIn(min = 56.dp), verticalAlignment = Alignment.CenterVertically) {
        Icon(Icons.Rounded.Speaker, null, tint = MaterialTheme.colorScheme.primary)
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(stringResource(R.string.sharing_mode_party), style = MaterialTheme.typography.titleSmall)
            Text(stringResource(R.string.sharing_party_toggle_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Switch(checked = hub.party, onCheckedChange = share::setParty)
    }

    if (hub.pending.isNotEmpty()) {
        HorizontalDivider(Modifier.padding(vertical = 8.dp))
        Text(stringResource(R.string.sharing_pending, hub.pending.size), style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.Bold)
        hub.pending.forEach { item ->
            Row(Modifier.fillMaxWidth().heightIn(min = 56.dp), verticalAlignment = Alignment.CenterVertically) {
                MemberDot(item.addedBy.color)
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(item.track.title, maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.bodyLarge)
                    Text(
                        stringResource(R.string.sharing_by, item.track.artist, item.addedBy.name),
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                var rejecting by remember { mutableStateOf(false) }
                if (rejecting) {
                    ConfirmDialog(
                        title = stringResource(R.string.sharing_reject_title, item.track.title),
                        confirm = stringResource(R.string.sharing_reject),
                        onConfirm = { share.reject(item.queueId) },
                        onDismiss = { rejecting = false },
                    )
                }
                IconButton(onClick = { rejecting = true }, modifier = Modifier.size(48.dp)) {
                    Icon(Icons.Rounded.Close, stringResource(R.string.sharing_reject), tint = MaterialTheme.colorScheme.error)
                }
                IconButton(onClick = { share.approve(item.queueId) }, modifier = Modifier.size(48.dp)) {
                    Icon(Icons.Rounded.Check, stringResource(R.string.sharing_approve), tint = MaterialTheme.colorScheme.primary)
                }
            }
        }
    }

    HorizontalDivider(Modifier.padding(vertical = 8.dp))
    Text(stringResource(R.string.sharing_guests, hub.guests.size), style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.Bold)
    if (hub.guests.isEmpty()) {
        Text(
            stringResource(R.string.sharing_guests_none),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(vertical = 8.dp),
        )
    }
    var kicking by remember { mutableStateOf<ShareMember?>(null) }
    var stopping by remember { mutableStateOf(false) }
    kicking?.let { g ->
        var block by remember(g.guestId) { mutableStateOf(false) }
        ConfirmDialog(
            title = stringResource(R.string.sharing_kick_title, g.name),
            text = stringResource(R.string.sharing_kick_text),
            confirm = stringResource(R.string.sharing_kick_confirm),
            onConfirm = { share.kick(g.guestId, ban = block) },
            onDismiss = { kicking = null },
            // blocking is a separate, unticked choice: removing someone never blacklists them
            extra = {
                Row(
                    Modifier.fillMaxWidth().padding(top = 12.dp).clickable { block = !block },
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Checkbox(checked = block, onCheckedChange = { block = it })
                    Text(stringResource(R.string.sharing_kick_block), style = MaterialTheme.typography.bodyMedium)
                }
            },
        )
    }
    if (stopping) {
        ConfirmDialog(
            title = stringResource(R.string.sharing_stop_title),
            text = stringResource(R.string.sharing_stop_text),
            confirm = stringResource(R.string.sharing_stop),
            onConfirm = share::stopHub,
            onDismiss = { stopping = false },
        )
    }
    hub.guests.forEach { g ->
        Row(Modifier.fillMaxWidth().heightIn(min = 56.dp), verticalAlignment = Alignment.CenterVertically) {
            MemberDot(g.color, 16.dp)
            Spacer(Modifier.width(12.dp))
            Text(g.name, Modifier.weight(1f), style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
            IconButton(onClick = { kicking = g }, modifier = Modifier.size(48.dp)) {
                Icon(Icons.Rounded.PersonRemove, stringResource(R.string.sharing_kick))
            }
        }
    }

    Spacer(Modifier.height(16.dp))
    OutlinedButton(
        onClick = { stopping = true },
        colors = ButtonDefaults.outlinedButtonColors(contentColor = MaterialTheme.colorScheme.error),
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.error),
        modifier = Modifier.fillMaxWidth().height(56.dp),
    ) { Text(stringResource(R.string.sharing_stop), style = MaterialTheme.typography.titleMedium) }
}

internal fun ShareNetworkMode.label(): Int = when (this) {
    ShareNetworkMode.WIFI_DIRECT -> R.string.sharing_net_direct
    ShareNetworkMode.LAN -> R.string.sharing_net_lan
}

private fun ShareNetworkMode.hint(): Int = when (this) {
    ShareNetworkMode.WIFI_DIRECT -> R.string.sharing_net_direct_hint
    ShareNetworkMode.LAN -> R.string.sharing_net_lan_hint
}
