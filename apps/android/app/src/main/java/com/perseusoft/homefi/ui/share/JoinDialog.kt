package com.perseusoft.homefi.ui.share

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.share.GuestStatus
import com.perseusoft.homefi.share.JoinRequest
import com.perseusoft.homefi.share.net.HubAddress
import com.perseusoft.homefi.share.net.ShareNetworkMode
import com.perseusoft.homefi.share.net.rememberSharePermissionRequest

private const val DEFAULT_PORT = 47800

/**
 * Joining a hub: only your name (and the code when the link or the banner
 * did not bring it). Without an address it is the manual form: host or a
 * `homefi://join` link, port and code.
 */
@Composable
fun JoinDialog(request: JoinRequest) {
    val app = LocalContext.current.homefi
    val share = app.share
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val guest by share.guest.collectAsStateWithLifecycle()
    val address = request.address

    var name by rememberSaveable { mutableStateOf(share.displayName(settings.share)) }
    var color by rememberSaveable { mutableStateOf(settings.share.color) }
    var code by rememberSaveable { mutableStateOf(address?.code.orEmpty()) }
    var hostText by rememberSaveable { mutableStateOf("") }
    var portText by rememberSaveable { mutableStateOf(DEFAULT_PORT.toString()) }
    var invalid by rememberSaveable { mutableStateOf(false) }
    val connecting = guest.status == GuestStatus.CONNECTING
    var waiting by remember { mutableStateOf<HubAddress?>(null) }
    var denied by rememberSaveable { mutableStateOf(false) }
    fun go(hub: HubAddress) {
        // remember who you are for next time
        app.settings.updateShare { it.copy(name = name.trim(), color = color) }
        share.join(hub, hub.code ?: code, name, color)
    }
    // a Wi-Fi Direct hub needs the nearby devices permission to connect
    val askPermissions = rememberSharePermissionRequest { granted ->
        denied = !granted
        waiting?.takeIf { granted }?.let(::go)
        waiting = null
    }

    // the dialog closes once the guest screen takes over
    LaunchedEffect(guest.status) {
        if (guest.status == GuestStatus.CONNECTED) share.dismissJoin()
    }

    fun target(): HubAddress? {
        if (address != null) return address
        val text = hostText.trim()
        if (text.startsWith("homefi:")) return share.parseLink(text)
        if (text.isEmpty()) return null
        val port = portText.trim().toIntOrNull()?.takeIf { it in 1..65535 } ?: return null
        return HubAddress(sessionName = text, host = text, port = port, mode = ShareNetworkMode.LAN, networkName = null, passphrase = null)
    }

    AlertDialog(
        onDismissRequest = {
            if (!connecting) {
                share.clearGuest()
                share.dismissJoin()
            }
        },
        title = { Text(address?.sessionName?.takeIf(String::isNotBlank) ?: stringResource(R.string.sharing_join_manual)) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                if (address == null) {
                    OutlinedTextField(
                        value = hostText,
                        onValueChange = { hostText = it; invalid = false },
                        label = { Text(stringResource(R.string.sharing_join_host)) },
                        placeholder = { Text("192.168.1.20") },
                        singleLine = true,
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
                        modifier = Modifier.fillMaxWidth(),
                    )
                    if (!hostText.trim().startsWith("homefi:")) {
                        OutlinedTextField(
                            value = portText,
                            onValueChange = { portText = it.filter(Char::isDigit).take(5); invalid = false },
                            label = { Text(stringResource(R.string.sharing_join_port)) },
                            singleLine = true,
                            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                            modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                        )
                    }
                }
                if (address?.code == null) {
                    OutlinedTextField(
                        value = code,
                        onValueChange = { code = it.filter(Char::isDigit).take(8) },
                        label = { Text(stringResource(R.string.sharing_code)) },
                        singleLine = true,
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.NumberPassword),
                        modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                    )
                }
                OutlinedTextField(
                    value = name,
                    onValueChange = { name = it.take(32) },
                    label = { Text(stringResource(R.string.sharing_your_name)) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                )
                Spacer(Modifier.height(12.dp))
                Text(stringResource(R.string.sharing_your_color), style = MaterialTheme.typography.labelLarge)
                Spacer(Modifier.height(6.dp))
                ColorPicker(color, { color = it })

                val error = when {
                    invalid -> stringResource(R.string.sharing_join_invalid)
                    denied -> stringResource(R.string.sharing_permission_denied)
                    else -> guest.error.takeIf { guest.status == GuestStatus.DISCONNECTED }
                }
                if (error != null) {
                    Text(error, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.padding(top = 12.dp))
                }
                if (connecting) {
                    Row(Modifier.padding(top = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                        CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
                        Spacer(Modifier.width(12.dp))
                        Text(stringResource(R.string.sharing_joining))
                    }
                }
            }
        },
        confirmButton = {
            TextButton(
                enabled = !connecting && name.isNotBlank(),
                onClick = {
                    val hub = target()
                    when {
                        hub == null -> invalid = true
                        hub.mode == ShareNetworkMode.WIFI_DIRECT -> {
                            waiting = hub
                            askPermissions()
                        }
                        else -> go(hub)
                    }
                },
            ) { Text(stringResource(R.string.sharing_join)) }
        },
        dismissButton = {
            TextButton(
                enabled = !connecting,
                onClick = {
                    share.clearGuest()
                    share.dismissJoin()
                },
            ) { Text(stringResource(R.string.cancel)) }
        },
    )
}
