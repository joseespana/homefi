package com.perseusoft.homefi.ui

import androidx.compose.foundation.Image
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.animateColorAsState
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.graphics.graphicsLayer
import com.perseusoft.homefi.ui.theme.LocalMotion
import androidx.compose.ui.res.stringResource
import androidx.compose.runtime.remember
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.getValue
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.SkipNext
import androidx.compose.material.icons.Icons
import com.perseusoft.homefi.ui.theme.FilledTonalButton
import com.perseusoft.homefi.ui.theme.LocalSkin
import com.perseusoft.homefi.ui.theme.bevel
import com.perseusoft.homefi.ui.theme.numeric
import androidx.compose.material3.Icon
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.DropdownMenu
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.perseusoft.homefi.R
import com.perseusoft.homefi.ui.theme.Brand
import androidx.compose.material.icons.automirrored.rounded.VolumeUp
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.homefi

/** The app logo: the mascot (adaptive icon colored layer) on the brand gradient. */
@Composable
fun HomefiLogo(size: Dp, modifier: Modifier = Modifier) {
    Box(
        modifier
            .size(size)
            .clip(RoundedCornerShape(size * 0.227f))
            .background(Brand.gradient),
        contentAlignment = Alignment.Center,
    ) {
        // the layer keeps the adaptive-icon safe margin, so draw it larger and let the box clip it
        Image(
            painterResource(R.drawable.ic_launcher_foreground),
            contentDescription = null,
            modifier = Modifier.requiredSize(size * 1.65f),
        )
    }
}

/** Format / quality tag: a small chip in the skin's colours (an LCD box on the amp, a label on vinyl). */
@Composable
fun QualityBadge(text: String, modifier: Modifier = Modifier) {
    val skin = LocalSkin.current
    val shape = if (skin.roundCovers) RoundedCornerShape(50) else MaterialTheme.shapes.extraSmall
    Text(
        text,
        style = MaterialTheme.typography.labelSmall.numeric(),
        color = MaterialTheme.colorScheme.onSecondaryContainer,
        maxLines = 1,
        modifier = modifier
            .clip(shape)
            .background(MaterialTheme.colorScheme.secondaryContainer)
            .bevel(skin.badgeBevel)
            .padding(horizontal = 6.dp, vertical = 2.dp),
    )
}

fun formatBytes(bytes: Long): String = when {
    bytes >= 1L shl 30 -> "%.1f GB".format(bytes / (1L shl 30).toDouble())
    bytes >= 1L shl 20 -> "%.1f MB".format(bytes / (1L shl 20).toDouble())
    bytes >= 1L shl 10 -> "%.0f KB".format(bytes / (1L shl 10).toDouble())
    else -> "$bytes B"
}

/** `3:07`, `1:02:33`. */
fun formatDuration(ms: Long): String {
    val total = (ms / 1000).coerceAtLeast(0)
    val (h, m, s) = Triple(total / 3600, total % 3600 / 60, total % 60)
    return if (h > 0) "%d:%02d:%02d".format(h, m, s) else "%d:%02d".format(m, s)
}

/** "Play next / Add to queue", opened by a long press on a song or a Queue button. */
@Composable
fun QueueMenu(expanded: Boolean, onDismiss: () -> Unit, onQueue: (next: Boolean) -> Unit) {
    val haptics = haptics()
    DropdownMenu(expanded = expanded, onDismissRequest = onDismiss) {
        DropdownMenuItem(
            text = { Text(stringResource(R.string.queue_play_next)) },
            leadingIcon = { Icon(Icons.Rounded.SkipNext, null) },
            onClick = {
                onDismiss()
                haptics.confirm()
                onQueue(true)
            },
        )
        DropdownMenuItem(
            text = { Text(stringResource(R.string.queue_add)) },
            leadingIcon = { Icon(Icons.AutoMirrored.Rounded.QueueMusic, null) },
            onClick = {
                onDismiss()
                haptics.confirm()
                onQueue(false)
            },
        )
    }
}

/** Tonal "Queue" button with the menu above, for album / artist / playlist headers. */
@Composable
fun QueueButton(onQueue: (next: Boolean) -> Unit, enabled: Boolean = true) {
    var open by remember { mutableStateOf(false) }
    Box {
        FilledTonalButton(onClick = { open = true }, enabled = enabled) {
            Icon(Icons.AutoMirrored.Rounded.QueueMusic, stringResource(R.string.queue_add))
        }
        QueueMenu(open, { open = false }, onQueue)
    }
}

/**
 * Asks before anything that removes, deletes or ends something, so a stray tap never does
 * it. [extra] goes under the text (an option like "also block"); Cancel is the safe default.
 */
@Composable
fun ConfirmDialog(
    title: String,
    confirm: String,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
    text: String? = null,
    extra: (@Composable () -> Unit)? = null,
) {
    androidx.compose.material3.AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = if (text == null && extra == null) null else {
            {
                androidx.compose.foundation.layout.Column {
                    if (text != null) Text(text, style = MaterialTheme.typography.bodyMedium)
                    extra?.invoke()
                }
            }
        },
        confirmButton = {
            androidx.compose.material3.TextButton(onClick = { onDismiss(); onConfirm() }) {
                Text(confirm, color = MaterialTheme.colorScheme.error)
            }
        },
        dismissButton = {
            androidx.compose.material3.TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) }
        },
    )
}

/**
 * Marks the row of the song being heard: a speaker while it plays, play while it's paused.
 * Tapping that row pauses or resumes it instead of starting it over.
 */
@Composable
fun NowMark(tint: Color, modifier: Modifier = Modifier) {
    val player by LocalContext.current.homefi.player.state.collectAsStateWithLifecycle()
    Icon(
        if (player.isPlaying) Icons.AutoMirrored.Rounded.VolumeUp else Icons.Rounded.PlayArrow,
        stringResource(if (player.isPlaying) R.string.now_playing_mark else R.string.paused_mark),
        tint = tint,
        modifier = modifier,
    )
}

/** The cover of the song being heard, dimmed under its [NowMark]. */
@Composable
fun BoxScope.NowCoverMark(radius: Dp) {
    Box(Modifier.matchParentSize().clip(RoundedCornerShape(radius)).background(Color.Black.copy(alpha = 0.5f)), contentAlignment = Alignment.Center) {
        NowMark(Color.White)
    }
}

/**
 * A FilterChip whose selection eases in: fill, ink and border fade between the two states
 * and a chip gives a small pop when it gets picked (all off with the motion setting).
 */
@Composable
fun SelectChip(
    selected: Boolean,
    onClick: () -> Unit,
    label: @Composable () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
) {
    val motion = LocalMotion.current
    val scheme = MaterialTheme.colorScheme
    val container by animateColorAsState(if (selected) scheme.secondaryContainer else Color.Transparent, motion.color(), label = "chip")
    val ink by animateColorAsState(if (selected) scheme.onSecondaryContainer else scheme.onSurfaceVariant, motion.color(), label = "chip-ink")
    val edge by animateColorAsState(if (selected) Color.Transparent else scheme.outlineVariant, motion.color(), label = "chip-edge")
    val pop = remember { Animatable(1f) }
    val was = remember { mutableStateOf(selected) }
    LaunchedEffect(selected) {
        // only when picked, not when the page opens with it already selected
        if (selected && !was.value && motion.moves) {
            pop.snapTo(0.9f)
            pop.animateTo(1f, motion.move())
        }
        was.value = selected
    }
    val haptics = haptics()
    FilterChip(
        selected = selected,
        onClick = {
            haptics.tick()
            onClick()
        },
        label = label,
        enabled = enabled,
        colors = FilterChipDefaults.filterChipColors(
            containerColor = container,
            selectedContainerColor = container,
            labelColor = ink,
            selectedLabelColor = ink,
        ),
        border = FilterChipDefaults.filterChipBorder(
            enabled = enabled,
            selected = selected,
            borderColor = edge,
            selectedBorderColor = edge,
            selectedBorderWidth = 1.dp,
        ),
        modifier = modifier.graphicsLayer {
            scaleX = pop.value
            scaleY = pop.value
        },
    )
}
