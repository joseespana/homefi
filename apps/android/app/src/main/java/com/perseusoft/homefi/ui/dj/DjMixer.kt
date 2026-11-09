package com.perseusoft.homefi.ui.dj

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.GraphicEq
import androidx.compose.material.icons.rounded.Stop
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.dp
import com.perseusoft.homefi.R
import com.perseusoft.homefi.player.dj.DJ_EQ_MIN_DB
import com.perseusoft.homefi.player.dj.DJ_DECKS
import com.perseusoft.homefi.player.dj.DjDeckUi
import com.perseusoft.homefi.player.dj.DjEq
import com.perseusoft.homefi.player.dj.DjMixerUi
import com.perseusoft.homefi.player.dj.deckLetter
import com.perseusoft.homefi.ui.SelectChip
import com.perseusoft.homefi.ui.haptics
import com.perseusoft.homefi.ui.settings.Segmented
import kotlin.math.abs
import kotlin.math.roundToInt

/** The mix lengths the core allows, in beats. */
private val MIX_BEATS = listOf(8, 16, 32)

/** The filter snaps to "off" this close to the centre. */
private const val FILTER_SNAP = 0.04f
private const val MIN_TRIM_DB = -12f
private const val MAX_TRIM_DB = 12f
private const val CENTRE_HALF = 0.5f

/** What the mixer asks of the session. */
class MixerActions(
    val mixTo: (Int) -> Unit,
    val stopMix: () -> Unit,
    val setBeats: (Int) -> Unit,
    val crossfader: (Float) -> Unit,
    val openEq: () -> Unit,
)

/**
 * "Mix to B" (or A, away from the side that plays now), its length, and the crossfader. Also
 * the sticky bottom bar on short phones.
 */
@Composable
fun MixBlock(
    mixer: DjMixerUi,
    beats: Int,
    actions: MixerActions,
    modifier: Modifier = Modifier,
    /** Small phone: the Mix button and its length share one row, so the pinned bar leaves room for the decks. */
    compact: Boolean = false,
    /** A wide short screen (a phone on its side): everything on one row in the bottom bar. */
    horizontal: Boolean = false,
    /** The narrow column between two decks: the EQ button goes under the crossfader. */
    column: Boolean = false,
) {
    val haptics = haptics()
    val mixing = mixer.mix
    val to = if (mixer.crossfader < CENTRE_HALF) 1 else 0
    val mixLabel = stringResource(if (to == 1) R.string.dj_mix_to_b else R.string.dj_mix_to_a)
    Column(modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = if (compact) 4.dp else 8.dp), verticalArrangement = Arrangement.spacedBy(if (compact) 4.dp else 8.dp)) {
        val mixButton: @Composable (Modifier) -> Unit = { m ->
            Button(
                onClick = {
                    haptics.confirm()
                    if (mixing != null) actions.stopMix() else actions.mixTo(to)
                },
                // a mix needs both songs; the button explains when it is pressed
                modifier = m.height(DJ_BUTTON_HEIGHT),
            ) {
                if (mixing != null) Icon(Icons.Rounded.Stop, null) else Icon(Icons.Rounded.GraphicEq, null)
                Spacer(Modifier.width(8.dp))
                Text(
                    if (mixing != null) stringResource(R.string.dj_stop_mix) else mixLabel,
                    style = MaterialTheme.typography.titleMedium,
                    maxLines = 1,
                )
            }
        }
        val lengthChips: @Composable () -> Unit = {
            MIX_BEATS.forEach { b ->
                SelectChip(selected = b == beats, onClick = { haptics.tick(); actions.setBeats(b) }, label = { Text("$b") })
            }
        }
        if (horizontal) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                mixButton(Modifier.weight(1f))
                if (mixing == null) {
                    lengthChips()
                } else {
                    Text(stringResource(R.string.dj_mixing, (mixing.progress * 100).roundToInt()), style = MaterialTheme.typography.labelMedium)
                }
                Crossfader(mixer.crossfader, actions, Modifier.weight(1.6f))
            }
            return@Column
        }
        if (compact && mixing == null) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                mixButton(Modifier.weight(1f))
                lengthChips()
            }
        } else {
            mixButton(Modifier.fillMaxWidth())
            if (mixing != null) {
                val percent = (mixing.progress * 100).roundToInt()
                val text = stringResource(R.string.dj_mixing, percent)
                LinearProgressIndicator(progress = { mixing.progress }, modifier = Modifier.fillMaxWidth().semantics { contentDescription = text })
                Text(text, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            } else {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(stringResource(R.string.dj_mix_length), style = MaterialTheme.typography.labelMedium, modifier = Modifier.weight(1f))
                    lengthChips()
                }
            }
        }
        Crossfader(mixer.crossfader, actions, eqBelow = column)
    }
}

@Composable
private fun Crossfader(position: Float, actions: MixerActions, modifier: Modifier = Modifier, eqBelow: Boolean = false) {
    val label = stringResource(R.string.dj_crossfader)
    val hint = stringResource(R.string.dj_crossfader_hint)
    Row(modifier, verticalAlignment = Alignment.CenterVertically) {
        Text(deckLetter(0).toString(), color = deckColor(0), style = MaterialTheme.typography.titleMedium)
        Slider(
            value = position,
            onValueChange = actions.crossfader,
            modifier = Modifier.weight(1f).padding(horizontal = 8.dp).semantics {
                contentDescription = label
                stateDescription = hint
            },
        )
        Text(deckLetter(1).toString(), color = deckColor(1), style = MaterialTheme.typography.titleMedium)
        if (!eqBelow) EqButton(actions)
    }
    if (eqBelow) EqButton(actions, Modifier.fillMaxWidth())
}

@Composable
private fun EqButton(actions: MixerActions, modifier: Modifier = Modifier) {
    TextButton(onClick = actions.openEq, modifier = modifier.heightIn(min = 48.dp)) {
        Icon(Icons.Rounded.Tune, null)
        Spacer(Modifier.width(4.dp))
        Text(stringResource(R.string.dj_eq), maxLines = 1)
    }
}

/** EQ, filter, volume (and trim in the full view) of either deck, and the master volume. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun EqSheet(
    decks: List<DjDeckUi>,
    mixer: DjMixerUi,
    maxBoostDb: Float,
    simple: Boolean,
    onEq: (Int, (DjEq) -> DjEq) -> Unit,
    onVolume: (Int, Float) -> Unit,
    onTrim: (Int, Float) -> Unit,
    onMaster: (Float) -> Unit,
    onDismiss: () -> Unit,
) {
    var deck by remember { mutableIntStateOf(0) }
    val d = decks[deck]
    val letter = deckLetter(deck).toString()
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 16.dp).padding(bottom = 24.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(stringResource(R.string.dj_eq), style = MaterialTheme.typography.titleLarge)
            Segmented(
                options = List(DJ_DECKS) { it },
                selected = deck,
                label = { stringResource(R.string.dj_deck, deckLetter(it).toString()) },
                onSelect = { deck = it },
                description = stringResource(R.string.dj_eq),
                modifier = Modifier.padding(vertical = 8.dp),
            )
            BandRow(R.string.dj_high, d.eq.highDb, d.eq.killHigh, maxBoostDb) { db, kill -> onEq(deck) { it.copy(highDb = db, killHigh = kill) } }
            BandRow(R.string.dj_mid, d.eq.midDb, d.eq.killMid, maxBoostDb) { db, kill -> onEq(deck) { it.copy(midDb = db, killMid = kill) } }
            BandRow(R.string.dj_low, d.eq.lowDb, d.eq.killLow, maxBoostDb) { db, kill -> onEq(deck) { it.copy(lowDb = db, killLow = kill) } }
            LabeledSlider(
                stringResource(R.string.dj_filter), d.eq.filter, -1f..1f,
                onChange = { v -> onEq(deck) { it.copy(filter = if (abs(v) < FILTER_SNAP) 0f else v) } },
                hint = stringResource(R.string.dj_filter_hint),
            )
            LabeledSlider(stringResource(R.string.dj_volume_deck, letter), d.volume, 0f..1f, onChange = { onVolume(deck, it) })
            if (!simple) {
                LabeledSlider(
                    stringResource(R.string.dj_trim, letter), d.trimDb, MIN_TRIM_DB..MAX_TRIM_DB,
                    onChange = { onTrim(deck, it) },
                    readout = stringResource(R.string.dj_db, d.trimDb.roundToInt()),
                )
            }
            LabeledSlider(stringResource(R.string.dj_master), mixer.master, 0f..1f, onChange = onMaster)
        }
    }
}

/** One EQ band: a slider from [DJ_EQ_MIN_DB] up to the largest boost, and its mute switch. */
@Composable
private fun BandRow(label: Int, db: Float, killed: Boolean, maxBoostDb: Float, onChange: (Float, Boolean) -> Unit) {
    val name = stringResource(label)
    val haptics = haptics()
    Row(verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text("$name  ${stringResource(R.string.dj_db, db.roundToInt())}", style = MaterialTheme.typography.labelLarge)
            Slider(
                value = db.coerceIn(DJ_EQ_MIN_DB, maxBoostDb),
                onValueChange = { onChange(it, killed) },
                valueRange = DJ_EQ_MIN_DB..maxBoostDb.coerceAtLeast(0.1f),
                enabled = !killed,
                modifier = Modifier.semantics { contentDescription = name },
            )
        }
        val kill = stringResource(R.string.dj_kill)
        SelectChip(
            selected = killed,
            onClick = { haptics.toggle(!killed); onChange(db, !killed) },
            label = { Text(if (killed) "$kill ✓" else kill, maxLines = 1) },
            modifier = Modifier.padding(start = 8.dp).semantics { contentDescription = "$kill: $name" },
        )
    }
}

@Composable
private fun LabeledSlider(
    label: String,
    value: Float,
    range: ClosedFloatingPointRange<Float>,
    onChange: (Float) -> Unit,
    hint: String? = null,
    readout: String? = null,
) {
    Column {
        Text(readout?.let { "$label  $it" } ?: label, style = MaterialTheme.typography.labelLarge)
        Slider(
            value = value.coerceIn(range.start, range.endInclusive),
            onValueChange = onChange,
            valueRange = range,
            modifier = Modifier.semantics { contentDescription = label },
        )
        if (hint != null) Text(hint, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}
