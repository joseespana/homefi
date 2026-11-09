package com.perseusoft.homefi.ui.dj

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material.icons.rounded.Sync
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.perseusoft.homefi.R
import com.perseusoft.homefi.library.EmbeddedCover
import com.perseusoft.homefi.ui.library.CoverArt
import com.perseusoft.homefi.player.dj.DJ_HOT_CUES
import com.perseusoft.homefi.player.dj.DJ_LOOP_BEATS
import com.perseusoft.homefi.player.dj.DjDeckUi
import com.perseusoft.homefi.player.dj.deckLetter
import com.perseusoft.homefi.player.dj.formatClock
import com.perseusoft.homefi.ui.SelectChip
import com.perseusoft.homefi.ui.haptics
import com.perseusoft.homefi.ui.theme.Card
import com.perseusoft.homefi.ui.theme.numeric
import java.util.Locale

/** Hit target of the main deck buttons. */
internal val DJ_BUTTON_HEIGHT = 56.dp
private val WAVEFORM_HEIGHT = 56.dp
private val COVER_SIZE = 56.dp
private val SMALL_BUTTON_HEIGHT = 48.dp
private val PAD_GAP = 8.dp
private val SYNC_ICON = 16.dp
private val DECK_BORDER = 2.dp

/** What a deck card asks of the session. */
class DeckActions(
    val choose: () -> Unit,
    val togglePlay: () -> Unit,
    val cueDown: () -> Unit,
    val cueUp: () -> Unit,
    val sync: () -> Unit,
    val seek: (Float) -> Unit,
    val tempo: (Float) -> Unit,
    val resetTempo: () -> Unit,
    val hotCue: (Int) -> Unit,
    val clearHotCue: (Int) -> Unit,
    val loop: (Int) -> Unit,
    val keyLock: (Boolean) -> Unit,
)

/** The colour that tells deck A from deck B (the skin's own tokens). */
@Composable
internal fun deckColor(deck: Int): Color {
    val look = LocalDjLook.current
    return when {
        look != null -> if (deck == 0) look.deckA else look.deckB
        deck == 0 -> MaterialTheme.colorScheme.primary
        else -> MaterialTheme.colorScheme.tertiary
    }
}

@Composable
fun DjDeckCard(
    index: Int,
    deck: DjDeckUi,
    actions: DeckActions,
    simple: Boolean,
    tempoRangePct: Float,
    keyLock: Boolean,
    waveformColors: Boolean,
    showBeats: Boolean,
    modifier: Modifier = Modifier,
) {
    val accent = deckColor(index)
    Card(
        modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerLow),
        border = BorderStroke(DECK_BORDER, accent.copy(alpha = if (deck.playing) 1f else 0.4f)),
    ) {
        Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            DeckHeader(index, deck, accent, actions.choose)
            if (!deck.loaded && !deck.loading) {
                EmptyDeck(index, actions.choose)
                return@Column
            }
            DjWaveform(
                deck, accent, waveformColors, showBeats, actions.seek,
                Modifier.fillMaxWidth().height(WAVEFORM_HEIGHT).clip(MaterialTheme.shapes.small).background(LocalDjLook.current?.waveBackground ?: MaterialTheme.colorScheme.surfaceContainerHighest),
            )
            if (deck.analyzing || deck.loading) {
                Text(stringResource(R.string.dj_finding_beat), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                LinearProgressIndicator(Modifier.fillMaxWidth())
            }
            deck.note?.let { Text(stringResource(it), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
            Transport(deck, accent, actions)
            if (!simple) {
                TempoRow(deck, tempoRangePct, keyLock, actions)
                HotCues(deck, accent, actions)
                Loops(deck, actions)
            }
        }
    }
}

@Composable
private fun DeckHeader(index: Int, deck: DjDeckUi, accent: Color, choose: () -> Unit) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Box(
            Modifier.size(COVER_SIZE).clickable(onClickLabel = stringResource(R.string.dj_choose), onClick = choose),
            contentAlignment = Alignment.Center,
        ) {
            CoverArt(deck.path?.let(::EmbeddedCover), COVER_SIZE)
            Text(
                deckLetter(index).toString(),
                style = MaterialTheme.typography.labelLarge,
                fontWeight = FontWeight.Bold,
                color = MaterialTheme.colorScheme.surface,
                modifier = Modifier.align(Alignment.TopStart).background(accent, RoundedCornerShape(bottomEnd = 8.dp)).padding(horizontal = 6.dp),
            )
        }
        Column(Modifier.weight(1f)) {
            Text(
                deck.title.ifBlank { stringResource(R.string.dj_deck, deckLetter(index).toString()) },
                style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis,
            )
            if (deck.artist.isNotBlank()) {
                Text(deck.artist, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        if (deck.loaded) {
            Column(horizontalAlignment = Alignment.End) {
                Text(
                    if (deck.analysis?.bpm?.let { it > 0 } == true) stringResource(R.string.dj_bpm, String.format(Locale.getDefault(), "%.1f", deck.bpm)) else stringResource(R.string.dj_no_bpm),
                    style = MaterialTheme.typography.titleSmall.numeric(),
                )
                if (deck.synced) Text(stringResource(R.string.dj_synced), style = MaterialTheme.typography.labelMedium, color = accent)
            }
        }
    }
}

@Composable
private fun EmptyDeck(index: Int, choose: () -> Unit) {
    val line = MaterialTheme.colorScheme.outline
    Box(
        Modifier
            .fillMaxWidth()
            .heightIn(min = DJ_BUTTON_HEIGHT)
            .clip(MaterialTheme.shapes.medium)
            .border(BorderStroke(1.dp, line), MaterialTheme.shapes.medium)
            .clickable(role = Role.Button, onClickLabel = stringResource(R.string.dj_choose_for, deckLetter(index).toString()), onClick = choose),
        contentAlignment = Alignment.Center,
    ) {
        Text(stringResource(R.string.dj_choose), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(16.dp))
    }
}

@Composable
private fun Transport(deck: DjDeckUi, accent: Color, actions: DeckActions) {
    val haptics = haptics()
    val ready = deck.loaded
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(PAD_GAP)) {
        HoldButton(
            stringResource(R.string.dj_cue), enabled = ready,
            onDown = { haptics.tick(); actions.cueDown() }, onUp = actions.cueUp,
            modifier = Modifier.weight(1f),
        )
        androidx.compose.material3.Button(
            onClick = { haptics.press(); actions.togglePlay() },
            enabled = ready,
            modifier = Modifier.weight(1.4f).height(DJ_BUTTON_HEIGHT),
            colors = androidx.compose.material3.ButtonDefaults.buttonColors(containerColor = accent, contentColor = MaterialTheme.colorScheme.surface),
        ) {
            Icon(if (deck.playing) Icons.Rounded.Pause else Icons.Rounded.PlayArrow, null)
            Spacer(Modifier.width(6.dp))
            Text(stringResource(if (deck.playing) R.string.dj_pause else R.string.dj_play))
        }
        val syncLabel = stringResource(if (deck.synced) R.string.dj_synced else R.string.dj_sync)
        val syncHint = stringResource(R.string.dj_sync_hint)
        androidx.compose.material3.FilledTonalButton(
            onClick = { haptics.toggle(!deck.synced); actions.sync() },
            enabled = ready && !deck.analyzing,
            modifier = Modifier.weight(1f).height(DJ_BUTTON_HEIGHT).semantics { stateDescription = syncHint },
            contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 6.dp),
        ) {
            Icon(Icons.Rounded.Sync, null, Modifier.size(SYNC_ICON), tint = if (deck.synced) accent else MaterialTheme.colorScheme.onSecondaryContainer)
            Spacer(Modifier.width(2.dp))
            Text(syncLabel, maxLines = 1, softWrap = false)
        }
    }
}

/** A button that reports the press and the release (Cue: hold to listen from the cue point). */
@Composable
private fun HoldButton(label: String, enabled: Boolean, onDown: () -> Unit, onUp: () -> Unit, modifier: Modifier = Modifier) {
    val down by rememberUpdatedState(onDown)
    val up by rememberUpdatedState(onUp)
    val hint = stringResource(R.string.dj_cue_hint)
    Surface(
        modifier
            .height(DJ_BUTTON_HEIGHT)
            .semantics(mergeDescendants = true) {
                role = Role.Button
                contentDescription = "$label. $hint"
                if (enabled) onClick(label) { down(); up(); true }
            }
            .pointerInput(enabled) {
                if (enabled) {
                    detectTapGestures(onPress = {
                        down()
                        try {
                            tryAwaitRelease()
                        } finally {
                            up()
                        }
                    })
                }
            },
        shape = MaterialTheme.shapes.large,
        color = if (enabled) MaterialTheme.colorScheme.secondaryContainer else MaterialTheme.colorScheme.onSurface.copy(alpha = 0.12f),
        contentColor = if (enabled) MaterialTheme.colorScheme.onSecondaryContainer else MaterialTheme.colorScheme.onSurface.copy(alpha = 0.38f),
    ) {
        Box(contentAlignment = Alignment.Center) { Text(label, style = MaterialTheme.typography.labelLarge) }
    }
}

@Composable
private fun TempoRow(deck: DjDeckUi, rangePct: Float, keyLock: Boolean, actions: DeckActions) {
    val pct = deck.tempoPct
    val readout = String.format(Locale.getDefault(), "%+.1f", pct)
    val tempoText = stringResource(R.string.dj_tempo_value, readout)
    Column {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(tempoText, style = MaterialTheme.typography.labelLarge.numeric(), modifier = Modifier.width(96.dp))
            Slider(
                value = pct.coerceIn(-rangePct, rangePct),
                onValueChange = { actions.tempo(it) },
                valueRange = -rangePct..rangePct,
                enabled = deck.loaded,
                modifier = Modifier.weight(1f).semantics { contentDescription = tempoText },
            )
            IconButton(onClick = actions.resetTempo, enabled = deck.loaded) {
                Icon(Icons.Rounded.Refresh, stringResource(R.string.dj_tempo_reset))
            }
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(stringResource(R.string.dj_key_lock), style = MaterialTheme.typography.bodyMedium, modifier = Modifier.weight(1f))
            Switch(checked = keyLock, onCheckedChange = actions.keyLock)
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun HotCues(deck: DjDeckUi, accent: Color, actions: DeckActions) {
    val haptics = haptics()
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(PAD_GAP)) {
        repeat(DJ_HOT_CUES) { i ->
            val saved = deck.hotCues.getOrNull(i)
            val description = stringResource(if (saved != null) R.string.dj_hot_cue_set else R.string.dj_hot_cue_empty, i + 1)
            Surface(
                Modifier
                    .weight(1f)
                    .heightIn(min = SMALL_BUTTON_HEIGHT)
                    .semantics { contentDescription = description; role = Role.Button }
                    .combinedClickable(
                        enabled = deck.loaded,
                        onClick = { haptics.tick(); actions.hotCue(i) },
                        onLongClick = { if (saved != null) { haptics.longPress(); actions.clearHotCue(i) } },
                    ),
                shape = MaterialTheme.shapes.medium,
                color = if (saved != null) accent else MaterialTheme.colorScheme.surfaceContainerHighest,
                contentColor = if (saved != null) MaterialTheme.colorScheme.surface else MaterialTheme.colorScheme.onSurfaceVariant,
            ) {
                Column(Modifier.padding(vertical = 4.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center) {
                    Text("${i + 1}", style = MaterialTheme.typography.labelLarge)
                    // numbers and text, never colour alone
                    Text(saved?.let(::formatClock) ?: "-", style = MaterialTheme.typography.labelSmall.numeric())
                }
            }
        }
    }
}

@Composable
private fun Loops(deck: DjDeckUi, actions: DeckActions) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(PAD_GAP)) {
        Text(stringResource(R.string.dj_loop), style = MaterialTheme.typography.labelLarge, modifier = Modifier.width(64.dp))
        DJ_LOOP_BEATS.forEach { beats ->
            val on = deck.loopBeats == beats
            val description = stringResource(R.string.dj_loop_n, beats)
            SelectChip(
                selected = on,
                onClick = { actions.loop(beats) },
                // the check mark says "on" without colour
                label = { Text(if (on) "$beats ✓" else "$beats") },
                modifier = Modifier.semantics { contentDescription = description },
                enabled = deck.loaded && deck.hasGrid,
            )
        }
    }
}
