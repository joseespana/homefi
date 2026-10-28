package com.perseusoft.homefi.ui.share

import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import com.perseusoft.homefi.ui.theme.LocalMotion
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Groups
import androidx.compose.material.icons.rounded.Wifi
import com.perseusoft.homefi.ui.theme.Button
import com.perseusoft.homefi.ui.theme.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.toMutableStateList
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material3.IconButton
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.listSaver
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
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.share.GuestStatus
import com.perseusoft.homefi.share.net.SharePermissions
import com.perseusoft.homefi.share.net.rememberSharePermissionRequest

/**
 * Home's way into someone else's music: "<session> is sharing music · Join"
 * for hubs found nearby, the session you are in, and "Join with a code".
 */
@Composable
fun ShareEntry() {
    val context = LocalContext.current
    val app = context.homefi
    val share = app.share
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val guest by share.guest.collectAsStateWithLifecycle()
    val hub by share.hub.collectAsStateWithLifecycle()
    val found by share.finder.hubs.collectAsStateWithLifecycle()
    val joining by share.joinRequest.collectAsStateWithLifecycle()
    var permissionTick by remember { mutableIntStateOf(0) }
    val askPermissions = rememberSharePermissionRequest { granted -> if (granted) permissionTick++ }

    if (hub.running || hub.starting) return

    when (guest.status) {
        GuestStatus.CONNECTED -> {
            EntryCard(Icons.Rounded.Groups, stringResource(R.string.sharing_in_session, guest.sessionName), stringResource(R.string.sharing_open)) {
                share.openGuest(true)
            }
            return
        }
        GuestStatus.CONNECTING -> {
            Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
                Spacer(Modifier.width(12.dp))
                Text(stringResource(R.string.sharing_joining), style = MaterialTheme.typography.bodyMedium)
            }
            return
        }
        GuestStatus.DISCONNECTED -> if (joining == null) {
            // kicked, the hub stopped, or the connection dropped
            guest.error?.let { error ->
                Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 8.dp, top = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(error, Modifier.weight(1f), color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium)
                    TextButton(onClick = share::clearGuest) { Text(stringResource(R.string.sharing_ok)) }
                }
            }
        }
        GuestStatus.IDLE -> Unit
    }

    // NearbySessions runs the search; the home Wi-Fi needs no permission, Wi-Fi Direct does
    val granted = remember(permissionTick) { SharePermissions.granted(context) }
    if (settings.share.discover) {
        found.take(MAX_BANNERS).forEach { address ->
            EntryCard(Icons.Rounded.Wifi, stringResource(R.string.sharing_found, address.sessionName), stringResource(R.string.sharing_join)) {
                share.requestJoin(address)
            }
        }
    }
    Row(Modifier.padding(horizontal = 8.dp), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        TextButton(onClick = { share.requestJoin(null) }) { Text(stringResource(R.string.sharing_join_manual)) }
        if (settings.share.discover && !granted) {
            TextButton(onClick = askPermissions) { Text(stringResource(R.string.sharing_find_nearby)) }
        }
    }
}

@Composable
private fun EntryCard(icon: ImageVector, text: String, action: String, onClick: () -> Unit) {
    Card(
        onClick = onClick,
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.primaryContainer),
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 6.dp),
    ) {
        Row(Modifier.padding(horizontal = 16.dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(icon, null, tint = MaterialTheme.colorScheme.onPrimaryContainer)
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Text(
                    text,
                    style = MaterialTheme.typography.titleSmall,
                    fontWeight = FontWeight.Bold,
                    color = MaterialTheme.colorScheme.onPrimaryContainer,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Spacer(Modifier.width(8.dp))
            Button(onClick = onClick) { Text(action) }
        }
    }
}

/**
 * Looks for sessions nearby while the app is on screen (and this phone is neither
 * hub nor guest), and, when [offer] and the setting allow it, offers the newest one
 * over the current screen. Home lists them itself, so it passes `offer = false`.
 */
@Composable
fun NearbySessions(offer: Boolean, modifier: Modifier = Modifier) {
    val app = LocalContext.current.homefi
    val share = app.share
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val guest by share.guest.collectAsStateWithLifecycle()
    val hub by share.hub.collectAsStateWithLifecycle()
    val found by share.finder.hubs.collectAsStateWithLifecycle()
    val joining by share.joinRequest.collectAsStateWithLifecycle()
    val search = settings.share.discover && !hub.running && !hub.starting && guest.status == GuestStatus.IDLE

    LifecycleResumeEffect(search) {
        if (search) share.finder.start()
        onPauseOrDispose { if (search) share.finder.stop() }
    }

    // "Not now" hides a session until the app is reopened
    val dismissed = rememberSaveable(saver = listSaver(save = { it.toList() }, restore = { it.toMutableStateList() })) {
        mutableListOf<String>().toMutableStateList()
    }
    val next = found.firstOrNull { it.mode.name + ":" + it.sessionName !in dismissed }
    val visible = offer && search && settings.share.discoverEverywhere && joining == null && next != null
    var shown by remember { mutableStateOf(next) }
    if (next != null) shown = next

    val motion = LocalMotion.current
    AnimatedVisibility(
        visible,
        modifier.statusBarsPadding(),
        enter = if (motion.moves) slideInVertically(motion.move()) { -it } + fadeIn(motion.tween(200)) else fadeIn(motion.tween(200)),
        exit = if (motion.moves) slideOutVertically(motion.move()) { -it } + fadeOut(motion.tween(150)) else fadeOut(motion.tween(150)),
    ) {
        val address = shown ?: return@AnimatedVisibility
        Card(
            colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.primaryContainer),
            elevation = CardDefaults.cardElevation(defaultElevation = 6.dp),
            modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp),
        ) {
            Row(Modifier.padding(start = 16.dp, end = 4.dp, top = 8.dp, bottom = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                Icon(Icons.Rounded.Wifi, null, tint = MaterialTheme.colorScheme.onPrimaryContainer)
                Spacer(Modifier.width(12.dp))
                Text(
                    stringResource(R.string.sharing_found_banner, address.sessionName),
                    Modifier.weight(1f),
                    style = MaterialTheme.typography.titleSmall,
                    fontWeight = FontWeight.Bold,
                    color = MaterialTheme.colorScheme.onPrimaryContainer,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
                Spacer(Modifier.width(8.dp))
                Button(onClick = { share.requestJoin(address) }) { Text(stringResource(R.string.sharing_join)) }
                IconButton(onClick = { dismissed.add(address.mode.name + ":" + address.sessionName) }) {
                    Icon(Icons.Rounded.Close, stringResource(R.string.sharing_not_now), tint = MaterialTheme.colorScheme.onPrimaryContainer)
                }
            }
        }
    }
}

private const val MAX_BANNERS = 3
