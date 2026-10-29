package com.perseusoft.homefi.ui.sound

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
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
import com.perseusoft.homefi.core.EqBand
import com.perseusoft.homefi.core.EqBandKind
import com.perseusoft.homefi.core.EqProfile
import com.perseusoft.homefi.core.EqSource
import com.perseusoft.homefi.core.TasteBand
import com.perseusoft.homefi.core.TasteCreator
import com.perseusoft.homefi.core.TasteRule
import com.perseusoft.homefi.core.TasteScope
import com.perseusoft.homefi.core.Tilt
import com.perseusoft.homefi.core.tasteClampCurve
import com.perseusoft.homefi.core.tasteCombinedProfile
import com.perseusoft.homefi.core.tasteCurveProfile
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.AiIcon
import com.perseusoft.homefi.ui.SelectChip
import com.perseusoft.homefi.ui.eq.BandEditor
import com.perseusoft.homefi.ui.eq.BandRow
import com.perseusoft.homefi.ui.eq.EqCurve
import com.perseusoft.homefi.ui.haptics
import kotlin.math.abs
import kotlin.math.log2
import kotlin.math.roundToInt

/** Bands a taste curve holds, and each band's range (homefi-core's taste caps). */
const val MAX_TASTE_BANDS = 8
const val MAX_TASTE_BAND_DB = 9f
private val TASTE_Q = 0.3f..8f

/** The rule changes nothing: a flat tilt and no audible band. */
fun ruleNeutral(tilt: Tilt, bands: List<TasteBand>): Boolean =
    abs(tilt.bassDb) < 0.05f && abs(tilt.trebleDb) < 0.05f && abs(tilt.clarityDb ?: 0f) < 0.05f &&
        bands.none { it.band.enabled && (!it.band.kind.gainKind() || abs(it.band.gainDb) >= 0.05f) }

private fun EqBandKind.gainKind() = this == EqBandKind.PEAK || this == EqBandKind.LOW_SHELF || this == EqBandKind.HIGH_SHELF

/** A new band an octave away from the others when there is room. */
private fun newBand(bands: List<TasteBand>): TasteBand {
    val spots = listOf(1000f, 125f, 4000f, 250f, 8000f, 60f, 500f, 2000f)
    val f = spots.firstOrNull { s -> bands.all { abs(log2(it.band.freqHz / s)) > 0.7f } } ?: 1000f
    return TasteBand(EqBand(EqBandKind.PEAK, f, 0f, 1f, true), "")
}

/**
 * A taste rule's sound, two ways: "Simple" (bass / treble / clarity) or "Advanced" (the
 * parametric curve: drag a point, the band list with an editor for the selected one).
 * The layer is drawn over the output's own profile (faint) with what plays, both
 * together, dashed; the assistant's note for each band shows under it. Settings → Sound
 * picks the tab it opens in.
 */
@Composable
fun TasteCurveEditor(
    tilt: Tilt,
    bands: List<TasteBand>,
    onTilt: (Tilt) -> Unit,
    onBands: (List<TasteBand>) -> Unit,
    scope: TasteScope?,
    onRefine: (() -> Unit)?,
) {
    val app = LocalContext.current.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val haptics = haptics()
    val pref = settings.tasteEditor
    var advanced by rememberSaveable { mutableStateOf(pref == "advanced" || (pref == "auto" && bands.isNotEmpty())) }
    var selected by rememberSaveable { mutableStateOf<Int?>(null) }

    Row(Modifier.horizontalScroll(rememberScrollState()), verticalAlignment = Alignment.CenterVertically) {
        SelectChip(selected = !advanced, onClick = { haptics.tick(); advanced = false }, label = { Text(stringResource(R.string.taste_mode_simple)) })
        Spacer(Modifier.width(8.dp))
        SelectChip(
            selected = advanced,
            onClick = { haptics.tick(); advanced = true },
            label = { Text(if (bands.isEmpty()) stringResource(R.string.taste_mode_advanced) else stringResource(R.string.taste_mode_advanced_count, bands.size)) },
        )
    }
    if (onRefine != null) {
        TextButton(onClick = { haptics.press(); onRefine() }) {
            Icon(AiIcon, null, Modifier.size(18.dp))
            Spacer(Modifier.width(6.dp))
            Text(stringResource(R.string.taste_refine))
        }
    }

    if (!advanced) {
        TiltSliders(tilt, inset = 0.dp, onChange = onTilt)
        if (bands.isNotEmpty()) Hint(stringResource(R.string.taste_simple_plus_bands, bands.size))
        return
    }

    // the core draws it: the layer, the output's profile and both together
    val small = remember { app.sound.smallSpeaker() }
    val device = remember { app.sound.deviceProfile() }
    val rule = TasteRule(
        id = "preview",
        scope = scope ?: TasteScope.Genre(""),
        tilt = tilt,
        label = "",
        createdBy = TasteCreator.MANUAL,
        enabled = true,
        bands = bands,
        explanation = "",
    )
    val curve = remember(tilt, bands) { runCatching { tasteCurveProfile(rule) }.getOrNull() }
    val combined = remember(tilt, bands, device) { runCatching { tasteCombinedProfile(device, rule, small) }.getOrNull() }
    val kept = remember(tilt, bands, small) { runCatching { tasteClampCurve(bands, tilt, small).notes }.getOrDefault(emptyList()) }
    val points = EqProfile(name = "", preampDb = 0f, bands = bands.map { it.band }, source = EqSource.Manual)

    fun update(i: Int, change: (EqBand) -> EqBand) {
        onBands(
            bands.mapIndexed { k, b ->
                if (k != i) b else {
                    val n = change(b.band)
                    b.copy(band = n.copy(freqHz = n.freqHz.coerceIn(20f, 20_000f), gainDb = n.gainDb.coerceIn(-MAX_TASTE_BAND_DB, MAX_TASTE_BAND_DB), q = n.q.coerceIn(TASTE_Q)))
                }
            },
        )
    }

    EqCurve(
        profile = points,
        curve = curve,
        background = device,
        compare = combined,
        maxRange = 12f,
        selected = selected,
        onSelect = { selected = it },
        onDrag = { i, f, g -> update(i) { it.copy(freqHz = f.roundToInt().toFloat(), gainDb = if (it.kind.gainKind()) Math.round(g * 10) / 10f else it.gainDb) } },
        onDragEnd = { haptics.tick() },
        modifier = Modifier.fillMaxWidth().height(170.dp).padding(top = 4.dp),
    )
    Text(
        listOfNotNull(
            stringResource(R.string.taste_legend_layer),
            device?.name?.ifBlank { null },
            stringResource(R.string.taste_legend_sum),
        ).joinToString("  ·  "),
        style = MaterialTheme.typography.labelSmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(top = 4.dp),
    )
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Text(stringResource(R.string.taste_bands_heading, bands.size, MAX_TASTE_BANDS), style = MaterialTheme.typography.titleSmall, modifier = Modifier.weight(1f))
        TextButton(onClick = { haptics.tick(); onBands(bands + newBand(bands)); selected = bands.size }, enabled = bands.size < MAX_TASTE_BANDS) {
            Icon(Icons.Rounded.Add, null, Modifier.size(18.dp))
            Spacer(Modifier.width(4.dp))
            Text(stringResource(R.string.taste_band_add))
        }
    }
    if (bands.isEmpty()) Hint(stringResource(R.string.taste_bands_empty))
    bands.forEachIndexed { i, b ->
        key(i) {
            BandRow(b.band, selected = selected == i, onClick = { haptics.tick(); selected = if (selected == i) null else i }, note = b.note)
            if (selected == i) {
                BandEditor(
                    b.band,
                    onChange = { t -> update(i, t) },
                    onKind = { k -> update(i) { it.copy(kind = k) } },
                    onRemove = { onBands(bands.filterIndexed { k, _ -> k != i }); selected = null },
                    gainRange = -MAX_TASTE_BAND_DB..MAX_TASTE_BAND_DB,
                    qRange = TASTE_Q,
                )
            }
        }
    }
    val said = tiltWords(tilt)
    if (said != stringResource(R.string.sound_tilt_none)) Hint(stringResource(R.string.taste_also_tilt, said))
    if (kept.isNotEmpty()) {
        Text(
            stringResource(R.string.taste_kept_safe, kept.joinToString(" · ")),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.tertiary,
            modifier = Modifier.padding(top = 4.dp),
        )
    }
}

@Composable
private fun Hint(text: String) {
    Text(text, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(vertical = 4.dp))
}

/**
 * Edits a taste rule (or makes one for [scope]): its name, and its sound in the Simple /
 * Advanced editor. [onDelete] adds a delete button; [onRefine] (with AI set up) saves what
 * is there and opens the assistant on it.
 */
@Composable
fun TasteRuleDialog(
    title: String,
    subtitle: String?,
    scope: TasteScope?,
    initial: TasteRule?,
    onDismiss: () -> Unit,
    onSave: (Tilt, List<TasteBand>, String?) -> Unit,
    onDelete: (() -> Unit)? = null,
    onRefine: ((Tilt, List<TasteBand>, String?) -> Unit)? = null,
) {
    var tilt by remember { mutableStateOf(initial?.tilt ?: Tilt(0f, 0f, null)) }
    var bands by remember { mutableStateOf(initial?.bands.orEmpty()) }
    var label by rememberSaveable { mutableStateOf(initial?.label.orEmpty()) }
    val haptics = haptics()
    val neutral = ruleNeutral(tilt, bands)
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                subtitle?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
                initial?.explanation?.ifBlank { null }?.let { Text(it, style = MaterialTheme.typography.bodySmall, modifier = Modifier.padding(top = 4.dp)) }
                OutlinedTextField(
                    value = label,
                    onValueChange = { label = it.take(60) },
                    label = { Text(stringResource(R.string.sound_rule_name)) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
                )
                TasteCurveEditor(
                    tilt = tilt,
                    bands = bands,
                    onTilt = { tilt = it },
                    onBands = { bands = it },
                    scope = scope ?: initial?.scope,
                    onRefine = onRefine?.takeIf { !neutral }?.let { r -> { r(tilt, bands, label) } },
                )
                if (onDelete != null) {
                    TextButton(onClick = { haptics.tick(); onDelete() }) {
                        Text(stringResource(R.string.sound_rule_delete), color = MaterialTheme.colorScheme.error)
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = { haptics.confirm(); onSave(tilt, bands, label) }, enabled = !neutral) { Text(stringResource(R.string.sound_save)) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}
