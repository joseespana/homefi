package com.perseusoft.homefi.ui.player

import com.perseusoft.homefi.ui.haptics
import com.perseusoft.homefi.share.GuestStatus
import androidx.compose.ui.semantics.Role
import androidx.compose.material3.Switch
import androidx.compose.material.icons.rounded.Groups
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Bluetooth
import androidx.compose.material.icons.rounded.Cable
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Headphones
import androidx.compose.material.icons.rounded.Speaker
import androidx.compose.material.icons.rounded.SpeakerGroup
import androidx.compose.material.icons.rounded.Tv
import androidx.compose.material.icons.rounded.Usb
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.runtime.getValue
import com.perseusoft.homefi.ui.theme.LocalMotion
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.AudioOutput
import com.perseusoft.homefi.player.OutputKind

fun OutputKind.icon(): ImageVector = when (this) {
    OutputKind.USB -> Icons.Rounded.Usb
    OutputKind.WIRED -> Icons.Rounded.Headphones
    OutputKind.BLUETOOTH -> Icons.Rounded.Bluetooth
    OutputKind.SPEAKER -> Icons.Rounded.Speaker
    OutputKind.HDMI -> Icons.Rounded.Tv
    OutputKind.LINE -> Icons.Rounded.Cable
    OutputKind.OTHER -> Icons.Rounded.SpeakerGroup
}

fun OutputKind.label(): Int = when (this) {
    OutputKind.USB -> R.string.output_usb
    OutputKind.WIRED -> R.string.output_wired
    OutputKind.BLUETOOTH -> R.string.output_bluetooth
    OutputKind.SPEAKER -> R.string.output_speaker
    OutputKind.HDMI -> R.string.output_hdmi
    OutputKind.LINE -> R.string.output_line
    OutputKind.OTHER -> R.string.output_other
}

/** `up to 32-bit / 384 kHz · 2 ch`, empty when the device does not report limits. */
fun AudioOutput.capabilities(): String {
    val parts = mutableListOf<String>()
    val bits = bitDepths.maxOrNull()
    val rate = sampleRates.maxOrNull()
    if (bits != null || rate != null) {
        parts += listOfNotNull(bits?.let { "$it-bit" }, rate?.let { r -> if (r % 1000 == 0) "${r / 1000} kHz" else "%.1f kHz".format(r / 1000.0) })
            .joinToString(" / ")
    }
    maxChannels?.takeIf { it > 0 }?.let { parts += "$it ch" }
    return parts.joinToString(" · ")
}

/** Icon of the output in use: the preferred one if connected, else the system route. */
@Composable
fun currentOutputKind(): OutputKind {
    val outputs = LocalContext.current.homefi.outputs
    val preferred by outputs.preferredKey.collectAsStateWithLifecycle()
    val list by outputs.outputs.collectAsStateWithLifecycle()
    return list.firstOrNull { it.key == preferred }?.kind ?: remember(list) { outputs.systemRoute()?.kind } ?: OutputKind.SPEAKER
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun OutputPicker(onDismiss: () -> Unit) {
    val outputs = LocalContext.current.homefi.outputs
    val list by outputs.outputs.collectAsStateWithLifecycle()
    val preferred by outputs.preferredKey.collectAsStateWithLifecycle()
    val system = remember(list) { outputs.systemRoute() }

    val motion = LocalMotion.current
    ModalBottomSheet(onDismissRequest = onDismiss) {
        // outputs connecting or leaving grow and shrink the sheet instead of jumping
        Column(Modifier.animateContentSize(motion.move()).padding(bottom = 24.dp)) {
            Text(
                stringResource(R.string.output_title),
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Bold,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            )
            PartyHere()
            OutputRow(
                icon = system?.kind?.icon() ?: Icons.Rounded.SpeakerGroup,
                title = stringResource(R.string.output_auto),
                subtitle = system?.let { s -> stringResource(R.string.output_auto_now, s.name.ifBlank { stringResource(s.kind.label()) }) }
                    ?: stringResource(R.string.output_auto_hint),
                selected = preferred == null,
            ) { outputs.select(null) }

            list.forEach { output ->
                OutputRow(
                    icon = output.kind.icon(),
                    title = output.name.ifBlank { stringResource(output.kind.label()) },
                    subtitle = listOf(stringResource(output.kind.label()), output.capabilities())
                        .filter(String::isNotBlank).distinct().joinToString(" · "),
                    selected = preferred == output.key,
                ) { outputs.select(output.key) }
            }

            if (preferred != null && list.none { it.key == preferred }) {
                Text(
                    stringResource(R.string.output_missing),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                )
            }
            Text(
                stringResource(R.string.output_note),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            )
        }
    }
}

/** In a party: whether it sounds on this phone's speaker too (hub) or at all (guest). */
@Composable
private fun PartyHere() {
    val share = LocalContext.current.homefi.share
    val hub by share.hub.collectAsStateWithLifecycle()
    val guest by share.guest.collectAsStateWithLifecycle()
    val (checked, onChange) = when {
        hub.running && hub.party -> hub.playHere to share::setHubPlayHere
        guest.status == GuestStatus.CONNECTED && guest.state?.audio != null -> guest.playAlong to share::setPlayAlong
        else -> return
    }
    ListItem(
        modifier = Modifier.toggleable(checked, role = Role.Switch, onValueChange = onChange),
        colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceContainerLow),
        leadingContent = { Icon(Icons.Rounded.Groups, null, tint = MaterialTheme.colorScheme.primary) },
        headlineContent = { Text(stringResource(R.string.output_party_here)) },
        supportingContent = {
            Text(stringResource(if (hub.running) R.string.output_party_here_hub else R.string.output_party_here_guest))
        },
        trailingContent = { Switch(checked = checked, onCheckedChange = null) },
    )
}

@Composable
private fun OutputRow(icon: ImageVector, title: String, subtitle: String, selected: Boolean, onClick: () -> Unit) {
    val motion = LocalMotion.current
    val tint by animateColorAsState(
        if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
        motion.color(),
        label = "output-tint",
    )
    val haptics = haptics()
    ListItem(
        modifier = Modifier.clickable {
            if (!selected) haptics.confirm()
            onClick()
        },
        colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceContainerLow),
        leadingContent = { Icon(icon, null, tint = tint) },
        headlineContent = { Text(title, fontWeight = if (selected) FontWeight.Bold else null) },
        supportingContent = { Text(subtitle) },
        // the check pops in on the picked output and out of the previous one
        trailingContent = {
            AnimatedVisibility(
                selected,
                enter = if (motion.moves) scaleIn(motion.move(), initialScale = 0.4f) + fadeIn(motion.tween(150)) else fadeIn(motion.tween(150)),
                exit = if (motion.moves) scaleOut(motion.tween(120), targetScale = 0.4f) + fadeOut(motion.tween(120)) else fadeOut(motion.tween(120)),
            ) {
                Icon(Icons.Rounded.Check, stringResource(R.string.output_selected), tint = MaterialTheme.colorScheme.primary)
            }
        },
    )
}
