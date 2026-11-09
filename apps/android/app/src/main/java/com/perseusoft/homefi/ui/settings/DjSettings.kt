package com.perseusoft.homefi.ui.settings

import androidx.compose.foundation.layout.Box
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.CrossfaderCurve
import com.perseusoft.homefi.core.DjSettings
import com.perseusoft.homefi.core.HomifitoMode
import com.perseusoft.homefi.core.HomifitoPlace
import com.perseusoft.homefi.core.LoadTarget
import com.perseusoft.homefi.data.setHomifitoLook
import com.perseusoft.homefi.data.setHomifitoMode
import com.perseusoft.homefi.data.setHomifitoPlace
import com.perseusoft.homefi.ui.dj.labelRes
import com.perseusoft.homefi.core.SfxKind
import com.perseusoft.homefi.data.DJ_PADS
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.dj.DjLayoutSheet
import com.perseusoft.homefi.ui.dj.DjSetupsSheet
import com.perseusoft.homefi.ui.dj.label
import kotlin.math.roundToInt

// Ranges mirror the core's `dj::settings` constants; the core clamps again when it stores them.
private const val MIN_TEMPO_RANGE_PCT = 4
private const val MAX_TEMPO_RANGE_PCT = 50
private const val MIN_EQ_BOOST_DB = 0
private const val MAX_EQ_BOOST_DB = 12
private const val MIN_MEMORY_MB = 128
private const val MAX_MEMORY_MB = 1024
private const val MEMORY_STEP_MB = 64
private const val PERCENT = 100
private const val VOLUME_STEP_PCT = 5
private val MIX_BEATS = listOf(8, 16, 32)

/** Settings → DJ summary: "16 beats · Simple view". */
@Composable
internal fun djSummary(): String {
    val app = LocalContext.current.homefi
    val dj by app.settings.dj.collectAsStateWithLifecycle()
    val prefs by app.settings.settings.collectAsStateWithLifecycle()
    return stringResource(
        R.string.dj_set_summary, dj.autoMixBeats.toInt(),
        stringResource(if (prefs.djSimpleView) R.string.dj_set_summary_simple else R.string.dj_set_summary_full),
    )
}

/** Settings → DJ: every option of the simple DJ (the core's JSON plus the phone's own switches). */
@Composable
internal fun DjSettingsPage() {
    val app = LocalContext.current.homefi
    val store = app.settings
    val dj by store.dj.collectAsStateWithLifecycle()
    val prefs by store.settings.collectAsStateWithLifecycle()
    val set: (DjSettings) -> Unit = store::setDj
    var layoutOpen by rememberSaveable { mutableStateOf(false) }
    var setupsOpen by rememberSaveable { mutableStateOf(false) }

    SettingsGroup(title = stringResource(R.string.dj_set_group_screen)) {
        ActionRow(
            stringResource(R.string.dj_title),
            onClick = app.dj::open,
            trailing = { TextButton(onClick = app.dj::open) { Text(stringResource(R.string.dj_play)) } },
        )
        RowDivider()
        ActionRow(
            stringResource(R.string.djs_edit_layout), hint = stringResource(R.string.djs_set_layout_hint),
            onClick = { layoutOpen = true },
            trailing = { TextButton(onClick = { layoutOpen = true }) { Text(stringResource(R.string.djs_edit)) } },
        )
        RowDivider()
        ActionRow(
            stringResource(R.string.djs_setups), hint = stringResource(R.string.djs_set_setups_hint),
            onClick = { setupsOpen = true },
            trailing = { TextButton(onClick = { setupsOpen = true }) { Text(stringResource(R.string.djs_open)) } },
        )
        RowDivider()
        SwitchRow(
            stringResource(R.string.dj_set_entry), prefs.djEntry,
            hint = stringResource(R.string.dj_set_entry_hint),
        ) { v -> store.update { it.copy(djEntry = v) } }
        RowDivider()
        SwitchRow(
            stringResource(R.string.dj_set_simple), prefs.djSimpleView,
            hint = stringResource(R.string.dj_set_simple_hint),
        ) { v -> store.update { it.copy(djSimpleView = v) } }
        RowDivider()
        SwitchRow(stringResource(R.string.dj_set_tips), prefs.djShowTips) { v -> store.update { it.copy(djShowTips = v) } }
        RowDivider()
        SwitchRow(
            stringResource(R.string.dj_set_waveform_colors), dj.waveformColors,
            hint = stringResource(R.string.dj_set_waveform_colors_hint),
        ) { set(dj.copy(waveformColors = it)) }
        RowDivider()
        SwitchRow(stringResource(R.string.dj_set_beat_grid), dj.showBeatGrid) { set(dj.copy(showBeatGrid = it)) }
    }

    SettingsGroup(title = stringResource(R.string.dj_homifito_group)) {
        SegmentedRow(
            stringResource(R.string.dj_homifito_size), HomifitoMode.entries, dj.homifitoMode,
            label = { stringResource(it.labelRes()) },
            onSelect = store::setHomifitoMode,
            hint = stringResource(R.string.dj_homifito_hint),
        )
        RowDivider()
        SegmentedRow(
            stringResource(R.string.dj_homifito_place), HomifitoPlace.entries, dj.homifitoPlace,
            label = { stringResource(it.labelRes()) },
            onSelect = store::setHomifitoPlace,
        )
        RowDivider()
        SwitchRow(
            stringResource(R.string.dj_homifito_look), dj.homifitoLook,
            hint = stringResource(R.string.dj_homifito_look_hint),
        ) { store.setHomifitoLook(it) }
    }

    SettingsGroup(title = stringResource(R.string.dj_set_group_mix)) {
        SwitchRow(
            stringResource(R.string.dj_set_sync_load), dj.syncOnLoad,
            hint = stringResource(R.string.dj_set_sync_load_hint),
        ) { set(dj.copy(syncOnLoad = it)) }
        RowDivider()
        SegmentedRow(
            stringResource(R.string.dj_set_mix_length), MIX_BEATS, dj.autoMixBeats.toInt(),
            label = { stringResource(R.string.dj_set_beats, it) },
            onSelect = { set(dj.copy(autoMixBeats = it.toUInt())) },
        )
        RowDivider()
        SwitchRow(
            stringResource(R.string.dj_set_bass_swap), prefs.djBassSwap,
            hint = stringResource(R.string.dj_set_bass_swap_hint),
        ) { v -> store.update { it.copy(djBassSwap = v) } }
        RowDivider()
        SwitchRow(
            stringResource(R.string.dj_set_quantize), dj.quantize,
            hint = stringResource(R.string.dj_set_quantize_hint),
        ) { set(dj.copy(quantize = it)) }
        RowDivider()
        CountSlider(
            stringResource(R.string.dj_set_tempo_range), dj.tempoRangePct.roundToInt(), MIN_TEMPO_RANGE_PCT..MAX_TEMPO_RANGE_PCT, step = 1,
            readout = { stringResource(R.string.dj_set_tempo_range_value, it) },
        ) { set(dj.copy(tempoRangePct = it.toFloat())) }
        RowDivider()
        SwitchRow(
            stringResource(R.string.dj_key_lock), prefs.djKeyLock,
            hint = stringResource(R.string.dj_set_key_lock_hint),
        ) { v -> store.update { it.copy(djKeyLock = v) } }
        RowDivider()
        SegmentedRow(
            stringResource(R.string.dj_set_curve), CrossfaderCurve.entries, dj.crossfaderCurve,
            label = { stringResource(it.labelRes()) },
            onSelect = { set(dj.copy(crossfaderCurve = it)) },
            hint = stringResource(R.string.dj_set_curve_hint),
        )
        RowDivider()
        SegmentedRow(
            stringResource(R.string.dj_set_load_target), listOf(LoadTarget.FREE_DECK, LoadTarget.ASK), dj.loadTarget.takeIf { it != LoadTarget.CRATE } ?: LoadTarget.ASK,
            label = { stringResource(if (it == LoadTarget.FREE_DECK) R.string.dj_load_free else R.string.dj_load_ask) },
            onSelect = { set(dj.copy(loadTarget = it)) },
        )
    }

    SettingsGroup(title = stringResource(R.string.dj_set_group_sound)) {
        SwitchRow(
            stringResource(R.string.dj_set_auto_gain), dj.autoGain,
            hint = stringResource(R.string.dj_set_auto_gain_hint),
        ) { set(dj.copy(autoGain = it)) }
        RowDivider()
        CountSlider(
            stringResource(R.string.dj_set_eq_boost), dj.eqMaxBoostDb.roundToInt(), MIN_EQ_BOOST_DB..MAX_EQ_BOOST_DB, step = 1,
            readout = { stringResource(R.string.dj_db, it) },
        ) { set(dj.copy(eqMaxBoostDb = it.toFloat())) }
    }

    SettingsGroup(title = stringResource(R.string.dj_set_group_sampler), description = stringResource(R.string.dj_set_pads_hint)) {
        CountSlider(
            stringResource(R.string.dj_set_sampler_volume), (dj.samplerVolume * PERCENT).roundToInt(), 0..PERCENT, step = VOLUME_STEP_PCT,
            readout = { "$it%" },
        ) { set(dj.copy(samplerVolume = it / PERCENT.toFloat())) }
        repeat(DJ_PADS) { pad ->
            RowDivider()
            ActionRow(
                stringResource(R.string.dj_set_pad_n, pad + 1),
                trailing = {
                    PadPicker(prefs.djPads.getOrElse(pad) { SfxKind.AIR_HORN }) { kind ->
                        store.update { s -> s.copy(djPads = s.djPads.toMutableList().also { it[pad] = kind }) }
                    }
                },
            )
        }
        RowDivider()
        CountSlider(
            stringResource(R.string.dj_set_memory), dj.memoryMb.toInt().coerceIn(MIN_MEMORY_MB, MAX_MEMORY_MB), MIN_MEMORY_MB..MAX_MEMORY_MB, step = MEMORY_STEP_MB,
            readout = { stringResource(R.string.dj_set_memory_value, it) },
            hint = stringResource(R.string.dj_set_memory_hint),
        ) { set(dj.copy(memoryMb = it.toUInt())) }
    }

    if (layoutOpen) DjLayoutSheet(onDismiss = { layoutOpen = false })
    if (setupsOpen) DjSetupsSheet(onDismiss = { setupsOpen = false })
}

private fun CrossfaderCurve.labelRes(): Int = when (this) {
    CrossfaderCurve.SMOOTH -> R.string.dj_curve_smooth
    CrossfaderCurve.LINEAR -> R.string.dj_curve_linear
    CrossfaderCurve.CUT -> R.string.dj_curve_cut
}

/** The sound of one pad, picked from the core's eight. */
@Composable
private fun PadPicker(current: SfxKind, onPick: (SfxKind) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box {
        TextButton(onClick = { open = true }) { Text(stringResource(current.label())) }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            SfxKind.entries.forEach { kind ->
                DropdownMenuItem(
                    text = { Text(stringResource(kind.label())) },
                    onClick = { open = false; onPick(kind) },
                )
            }
        }
    }
}
