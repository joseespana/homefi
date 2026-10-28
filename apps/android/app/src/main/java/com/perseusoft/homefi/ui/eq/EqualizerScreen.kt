package com.perseusoft.homefi.ui.eq

import com.perseusoft.homefi.ui.AiIcon
import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.Headphones
import androidx.compose.material.icons.rounded.HearingDisabled
import androidx.compose.material.icons.rounded.Info
import androidx.compose.material.icons.rounded.Save
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material.icons.automirrored.rounded.VolumeUp
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.EqBand
import com.perseusoft.homefi.core.EqBandKind
import com.perseusoft.homefi.core.eqAutoPreamp
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.EqTab
import com.perseusoft.homefi.player.Equalizer
import com.perseusoft.homefi.ui.sound.TasteRulesTab
import androidx.compose.material3.PrimaryTabRow
import androidx.compose.material3.Tab
import com.perseusoft.homefi.ui.ConfirmDialog
import com.perseusoft.homefi.ui.SelectChip
import com.perseusoft.homefi.ui.haptics
import com.perseusoft.homefi.ui.theme.Card
import com.perseusoft.homefi.ui.theme.FilledTonalButton
import com.perseusoft.homefi.ui.theme.LocalMotion
import com.perseusoft.homefi.ui.theme.LocalSkin
import com.perseusoft.homefi.ui.theme.SkinHeading
import com.perseusoft.homefi.ui.theme.numeric

/**
 * The equalizer: on/off, which output's profile to edit, the response curve with draggable
 * bands, the band list, preamp, taste, presets, AutoEq search and the way to the assistant.
 * Everything is heard as it changes.
 */
@OptIn(ExperimentalFoundationApi::class, androidx.compose.material3.ExperimentalMaterial3Api::class)
@Composable
fun EqualizerScreen(onClose: () -> Unit, onAssistant: () -> Unit, vm: EqViewModel = viewModel()) {
    BackHandler(onBack = onClose)
    LaunchedEffect(Unit) { vm.reopen() }
    val app = LocalContext.current.homefi
    val eq = app.equalizer
    val settings by eq.settings.collectAsStateWithLifecycle()
    val presets by eq.presets.collectAsStateWithLifecycle()
    val playingOn by eq.outputId.collectAsStateWithLifecycle()
    val active by eq.active.collectAsStateWithLifecycle()
    val held by eq.held.collectAsStateWithLifecycle()
    val connected by app.outputs.outputs.collectAsStateWithLifecycle()
    val target by vm.target.collectAsStateWithLifecycle()
    val selected by vm.selected.collectAsStateWithLifecycle()
    val taste by vm.taste.collectAsStateWithLifecycle()
    val notice by vm.notice.collectAsStateWithLifecycle()
    val (profile, own) = eq.profileOf(target, settings)
    val haptics = haptics()
    val motion = LocalMotion.current

    Column(
        Modifier
            .fillMaxSize()
            .background(LocalSkin.current.base)
            .systemBarsPadding()
            .imePadding(),
    ) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = onClose) { Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.back)) }
            Text(
                stringResource(R.string.eq_title),
                style = MaterialTheme.typography.titleLarge,
                fontWeight = FontWeight.Bold,
                modifier = Modifier.weight(1f),
            )
            Switch(
                checked = settings.enabled,
                onCheckedChange = { v -> haptics.toggle(v); vm.setEnabled(v) },
                modifier = Modifier.padding(end = 12.dp),
            )
        }

        val eqTab by eq.tab.collectAsStateWithLifecycle()
        PrimaryTabRow(selectedTabIndex = eqTab.ordinal) {
            EqTab.entries.forEach { t ->
                Tab(
                    selected = eqTab == t,
                    onClick = { if (eqTab != t) haptics.tick(); eq.selectTab(t) },
                    text = { Text(stringResource(if (t == EqTab.EQUALIZER) R.string.eq_tab_equalizer else R.string.sound_rules), maxLines = 1) },
                )
            }
        }

        if (eqTab == EqTab.TASTE) Box(Modifier.weight(1f)) { TasteRulesTab() } else
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(bottom = 24.dp)) {
            // what the switch means for the signal
            Note(
                if (active) R.string.eq_not_bitperfect_note else R.string.eq_off_note,
                if (active) Icons.Rounded.Info else Icons.AutoMirrored.Rounded.VolumeUp,
            )

            // ---- output
            Heading(R.string.eq_output)
            val ids = remember(connected, settings.perOutput, playingOn) {
                (listOf(playingOn) + connected.map { it.eqId } + settings.perOutput.keys).distinct()
            }
            Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
                SelectChip(
                    selected = target == null,
                    onClick = { vm.selectTarget(null) },
                    label = { Text(stringResource(R.string.eq_output_default)) },
                    modifier = Modifier.padding(end = 8.dp),
                )
                ids.forEach { id ->
                    key(id) {
                        SelectChip(
                            selected = target == id,
                            onClick = { vm.selectTarget(id) },
                            label = {
                                Row(verticalAlignment = Alignment.CenterVertically) {
                                    if (id == playingOn) {
                                        Icon(Icons.AutoMirrored.Rounded.VolumeUp, stringResource(R.string.eq_output_now), Modifier.size(16.dp))
                                        Spacer(Modifier.width(4.dp))
                                    }
                                    Text(outputLabel(id, connected))
                                }
                            },
                            modifier = Modifier.padding(end = 8.dp),
                        )
                    }
                }
            }
            if (target != null) {
                Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        stringResource(if (own) R.string.eq_own_profile else R.string.eq_uses_default),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.weight(1f),
                    )
                    if (own) TextButton(onClick = { haptics.tick(); vm.useDefault() }) { Text(stringResource(R.string.eq_use_default)) }
                }
            }

            // ---- curve
            Card(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
                Column(Modifier.padding(12.dp)) {
                    Text(
                        profile?.name?.ifBlank { null } ?: stringResource(if (profile?.bands.isNullOrEmpty()) R.string.eq_flat else R.string.eq_custom),
                        style = MaterialTheme.typography.titleSmall,
                        fontWeight = FontWeight.Bold,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    profile?.sourceLabel()?.let {
                        Text(
                            it,
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                    Spacer(Modifier.height(8.dp))
                    EqCurve(
                        profile = profile,
                        selected = selected,
                        dimmed = !settings.enabled || held,
                        onSelect = vm::select,
                        onDrag = vm::drag,
                        onDragEnd = { haptics.tick() },
                        modifier = Modifier.fillMaxWidth().height(200.dp),
                    )
                    Text(
                        stringResource(if (profile?.bands.isNullOrEmpty()) R.string.eq_empty else R.string.eq_curve_hint),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(top = 6.dp),
                    )
                    Spacer(Modifier.height(8.dp))
                    HoldButton(held = held, enabled = active || held, onHold = vm::hold)
                }
            }

            // ---- the selected band, opened under the curve
            val band = selected?.let { profile?.bands?.getOrNull(it) }
            AnimatedVisibility(band != null, enter = motion.reveal(), exit = motion.conceal()) {
                val i = selected
                if (band != null && i != null) BandEditor(band, onChange = { t -> vm.updateBand(i, t) }, onKind = { vm.setKind(i, it) }, onRemove = { vm.removeBand(i) })
            }

            // ---- bands
            val bands = profile?.bands.orEmpty()
            Row(Modifier.fillMaxWidth().padding(end = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.weight(1f)) { Heading(R.string.eq_bands, bands.size, Equalizer.MAX_BANDS) }
                TextButton(onClick = { haptics.tick(); vm.addBand() }, enabled = bands.size < Equalizer.MAX_BANDS) {
                    Icon(Icons.Rounded.Add, null, Modifier.size(18.dp))
                    Spacer(Modifier.width(4.dp))
                    Text(stringResource(R.string.eq_band_add))
                }
            }
            Column(Modifier.animateContentSize(motion.move())) {
                bands.forEachIndexed { i, b ->
                    key(i) { BandRow(b, selected = selected == i, onClick = { haptics.tick(); vm.select(if (selected == i) null else i) }) }
                }
            }

            HorizontalDivider(Modifier.padding(vertical = 8.dp))

            // ---- preamp
            Heading(R.string.eq_preamp)
            val auto = profile?.let { remember(it) { runCatching { eqAutoPreamp(it) }.getOrDefault(0f) } } ?: 0f
            ListItem(
                colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surface),
                headlineContent = { Text(stringResource(R.string.eq_preamp_auto)) },
                supportingContent = { Text(stringResource(R.string.eq_preamp_auto_hint, stringResource(R.string.eq_db, formatDb(auto)))) },
                trailingContent = {
                    Switch(checked = settings.autoPreamp, onCheckedChange = { v -> haptics.toggle(v); vm.setAutoPreamp(v) })
                },
            )
            AnimatedVisibility(!settings.autoPreamp, enter = motion.reveal(), exit = motion.conceal()) {
                LabeledSlider(
                    label = stringResource(R.string.eq_preamp),
                    value = stringResource(R.string.eq_db, formatDb(profile?.preampDb ?: 0f)),
                    position = profile?.preampDb ?: 0f,
                    range = -24f..6f,
                    onChange = vm::setPreamp,
                )
            }

            HorizontalDivider(Modifier.padding(vertical = 8.dp))

            // ---- taste
            Heading(R.string.eq_taste)
            LabeledSlider(stringResource(R.string.eq_bass), stringResource(R.string.eq_db, formatDb(taste.bass)), taste.bass, -6f..6f) {
                vm.setTaste(taste.copy(bass = Math.round(it * 2) / 2f))
            }
            LabeledSlider(stringResource(R.string.eq_treble), stringResource(R.string.eq_db, formatDb(taste.treble)), taste.treble, -6f..6f) {
                vm.setTaste(taste.copy(treble = Math.round(it * 2) / 2f))
            }
            LabeledSlider(stringResource(R.string.eq_clarity), stringResource(R.string.eq_db, formatDb(taste.clarity)), taste.clarity, -6f..6f) {
                vm.setTaste(taste.copy(clarity = Math.round(it * 2) / 2f))
            }
            Hint(R.string.eq_taste_hint)

            HorizontalDivider(Modifier.padding(vertical = 8.dp))

            // ---- presets
            Heading(R.string.eq_presets)
            var naming by rememberSaveable { mutableStateOf(false) }
            var deleting by rememberSaveable { mutableStateOf<String?>(null) }
            Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                SelectChip(
                    selected = profile?.bands.isNullOrEmpty(),
                    onClick = { haptics.confirm(); vm.flat() },
                    label = { Text(stringResource(R.string.eq_flat)) },
                    modifier = Modifier.padding(end = 8.dp),
                )
                presets.keys.sorted().forEach { name ->
                    key(name) {
                        val p = presets.getValue(name)
                        PresetChip(
                            name = name,
                            selected = profile != null && profile.bands == p.bands && profile.name == name,
                            onClick = { haptics.confirm(); vm.applyPreset(p) },
                            onLongClick = { haptics.longPress(); deleting = name },
                        )
                    }
                }
                TextButton(onClick = { naming = true }) {
                    Icon(Icons.Rounded.Save, null, Modifier.size(18.dp))
                    Spacer(Modifier.width(4.dp))
                    Text(stringResource(R.string.eq_save_preset))
                }
            }
            if (presets.isNotEmpty()) Hint(R.string.eq_preset_hint)
            if (naming) {
                PresetNameDialog(initial = profile?.name.orEmpty(), onDismiss = { naming = false }) { name ->
                    naming = false
                    haptics.confirm()
                    vm.savePreset(name)
                }
            }
            deleting?.let { name ->
                ConfirmDialog(
                    title = stringResource(R.string.eq_preset_delete, name),
                    confirm = stringResource(R.string.eq_preset_delete_action),
                    onConfirm = { vm.deletePreset(name) },
                    onDismiss = { deleting = null },
                )
            }

            HorizontalDivider(Modifier.padding(vertical = 8.dp))

            // ---- AutoEq
            AutoEqSection(vm, notice)

            HorizontalDivider(Modifier.padding(vertical = 8.dp))

            // ---- assistant
            Card(onClick = { haptics.press(); onAssistant() }, modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
                Row(Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
                    Icon(AiIcon, null, tint = MaterialTheme.colorScheme.primary)
                    Spacer(Modifier.width(12.dp))
                    Column(Modifier.weight(1f)) {
                        Text(stringResource(R.string.eq_assistant), style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.Bold)
                        Text(stringResource(R.string.eq_assistant_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
        }
    }
}

@Composable
private fun Heading(text: Int, vararg args: Any) {
    SkinHeading(
        stringResource(text, *args),
        style = MaterialTheme.typography.titleSmall,
        color = MaterialTheme.colorScheme.primary,
        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
    )
}

@Composable
private fun Hint(text: Int) {
    Text(
        stringResource(text),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp),
    )
}

@Composable
private fun Note(text: Int, icon: androidx.compose.ui.graphics.vector.ImageVector) {
    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        Icon(icon, null, Modifier.size(16.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.width(8.dp))
        Text(stringResource(text), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

/** A/B: while pressed, the filters fade out; let go and they fade back in. */
@Composable
private fun HoldButton(held: Boolean, enabled: Boolean, onHold: (Boolean) -> Unit) {
    val motion = LocalMotion.current
    val scheme = MaterialTheme.colorScheme
    val fill by animateColorAsState(if (held) scheme.primary else scheme.secondaryContainer, motion.color(), label = "hold")
    val ink by animateColorAsState(if (held) scheme.onPrimary else scheme.onSecondaryContainer, motion.color(), label = "hold-ink")
    val haptics = haptics()
    Row(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(50))
            .background(if (enabled) fill else scheme.surfaceVariant)
            .pointerInput(enabled) {
                if (!enabled) return@pointerInput
                detectTapGestures(onPress = {
                    haptics.press()
                    onHold(true)
                    try {
                        tryAwaitRelease()
                    } finally {
                        onHold(false)
                    }
                })
            }
            .padding(vertical = 12.dp),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        val color = if (enabled) ink else scheme.onSurfaceVariant
        Icon(Icons.Rounded.HearingDisabled, null, Modifier.size(18.dp), tint = color)
        Spacer(Modifier.width(8.dp))
        Text(stringResource(if (held) R.string.eq_holding else R.string.eq_hold), color = color, style = MaterialTheme.typography.labelLarge)
    }
}

@Composable
private fun BandRow(band: EqBand, selected: Boolean, onClick: () -> Unit) {
    val scheme = MaterialTheme.colorScheme
    val motion = LocalMotion.current
    val container by animateColorAsState(if (selected) scheme.secondaryContainer else scheme.surface, motion.color(), label = "band")
    val parts = buildList {
        add("${formatHz(band.freqHz)} Hz")
        if (band.kind.hasGain()) add(stringResource(R.string.eq_db, formatDb(band.gainDb)))
        add("Q %.2f".format(band.q))
    }
    ListItem(
        colors = ListItemDefaults.colors(containerColor = container),
        headlineContent = { Text(stringResource(band.kind.labelRes()), color = if (band.enabled) scheme.onSurface else scheme.onSurfaceVariant) },
        supportingContent = { Text(parts.joinToString("  ·  "), style = MaterialTheme.typography.bodySmall.numeric()) },
        modifier = Modifier.clickable(onClick = onClick),
    )
}

@Composable
private fun BandEditor(band: EqBand, onChange: ((EqBand) -> EqBand) -> Unit, onKind: (EqBandKind) -> Unit, onRemove: () -> Unit) {
    val haptics = haptics()
    Card(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp)) {
        Column(Modifier.padding(vertical = 8.dp)) {
            Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 12.dp)) {
                EqBandKind.entries.forEach { k ->
                    SelectChip(
                        selected = band.kind == k,
                        onClick = { onKind(k) },
                        label = { Text(stringResource(k.labelRes())) },
                        modifier = Modifier.padding(end = 8.dp),
                    )
                }
            }
            LabeledSlider(stringResource(R.string.eq_freq), "${formatHz(band.freqHz)} Hz", freqToFraction(band.freqHz), 0f..1f) { f ->
                onChange { it.copy(freqHz = fractionToFreq(f)) }
            }
            if (band.kind.hasGain()) {
                LabeledSlider(stringResource(R.string.eq_gain), stringResource(R.string.eq_db, formatDb(band.gainDb)), band.gainDb, -24f..24f) { g ->
                    onChange { it.copy(gainDb = Math.round(g * 10) / 10f) }
                }
            }
            // Q on a log scale: 0.1..10 with 1 in the middle
            LabeledSlider(stringResource(R.string.eq_q), "%.2f".format(band.q), (kotlin.math.log10(band.q) + 1f) / 2f, 0f..1f) { f ->
                onChange { it.copy(q = Math.round(Math.pow(10.0, f * 2.0 - 1.0).toFloat() * 100) / 100f) }
            }
            Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                Switch(checked = band.enabled, onCheckedChange = { v -> haptics.toggle(v); onChange { it.copy(enabled = v) } }, modifier = Modifier.padding(start = 12.dp))
                Text(stringResource(R.string.eq_band_on), Modifier.padding(start = 8.dp).weight(1f))
                TextButton(onClick = { haptics.tick(); onRemove() }) {
                    Icon(Icons.Rounded.Delete, null, Modifier.size(18.dp), tint = MaterialTheme.colorScheme.error)
                    Spacer(Modifier.width(4.dp))
                    Text(stringResource(R.string.eq_band_remove), color = MaterialTheme.colorScheme.error)
                }
            }
        }
    }
}

@Composable
private fun LabeledSlider(label: String, value: String, position: Float, range: ClosedFloatingPointRange<Float>, onChange: (Float) -> Unit) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp)) {
        Row(Modifier.fillMaxWidth().padding(top = 4.dp)) {
            Text(label, style = MaterialTheme.typography.labelLarge, modifier = Modifier.weight(1f))
            Text(value, style = MaterialTheme.typography.labelLarge.numeric(), color = MaterialTheme.colorScheme.primary)
        }
        Slider(value = position.coerceIn(range), onValueChange = onChange, valueRange = range)
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun PresetChip(name: String, selected: Boolean, onClick: () -> Unit, onLongClick: () -> Unit) {
    val scheme = MaterialTheme.colorScheme
    val motion = LocalMotion.current
    val container by animateColorAsState(if (selected) scheme.secondaryContainer else scheme.surface, motion.color(), label = "preset")
    Box(
        Modifier
            .padding(end = 8.dp)
            .clip(RoundedCornerShape(8.dp))
            .background(container)
            .combinedClickable(onClick = onClick, onLongClick = onLongClick)
            .padding(horizontal = 12.dp, vertical = 8.dp),
    ) {
        Text(name, style = MaterialTheme.typography.labelLarge, color = if (selected) scheme.onSecondaryContainer else scheme.onSurfaceVariant)
    }
}

@Composable
private fun PresetNameDialog(initial: String, onDismiss: () -> Unit, onSave: (String) -> Unit) {
    var name by rememberSaveable { mutableStateOf(initial) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.eq_save_preset)) },
        text = {
            OutlinedTextField(
                value = name,
                onValueChange = { name = it.take(40) },
                label = { Text(stringResource(R.string.eq_preset_name)) },
                singleLine = true,
            )
        },
        confirmButton = { TextButton(onClick = { onSave(name) }, enabled = name.isNotBlank()) { Text(stringResource(R.string.ai_save)) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

@Composable
private fun AutoEqSection(vm: EqViewModel, notice: String?) {
    val ui by vm.autoEq.collectAsStateWithLifecycle()
    val haptics = haptics()
    val motion = LocalMotion.current
    Heading(R.string.eq_autoeq)
    Hint(R.string.eq_autoeq_hint)
    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        OutlinedTextField(
            value = ui.query,
            onValueChange = vm::setQuery,
            placeholder = { Text(stringResource(R.string.eq_autoeq_search)) },
            leadingIcon = { Icon(Icons.Rounded.Headphones, null) },
            trailingIcon = {
                if (ui.query.isNotEmpty()) IconButton(onClick = { vm.setQuery("") }) { Icon(Icons.Rounded.Close, null) }
            },
            singleLine = true,
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
            keyboardActions = KeyboardActions(onSearch = { vm.search() }),
            modifier = Modifier.weight(1f),
        )
        Spacer(Modifier.width(8.dp))
        FilledTonalButton(onClick = { haptics.tick(); vm.search() }, enabled = ui.query.isNotBlank() && !ui.searching) {
            if (ui.searching) {
                CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
            } else {
                Icon(Icons.Rounded.Search, stringResource(R.string.eq_search))
            }
        }
    }
    ui.error?.let { Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall, modifier = Modifier.padding(horizontal = 16.dp)) }
    notice?.let { Text(it, color = MaterialTheme.colorScheme.primary, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp)) }
    Column(Modifier.animateContentSize(motion.move())) {
        val results = ui.results
        if (results != null && results.isEmpty()) Hint(R.string.eq_autoeq_none)
        results?.forEach { entry ->
            key(entry.path) {
                ListItem(
                    colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surface),
                    leadingContent = { Icon(Icons.Rounded.Headphones, null, tint = MaterialTheme.colorScheme.primary) },
                    headlineContent = { Text(entry.name) },
                    supportingContent = {
                        Text(listOfNotNull(entry.source, entry.kind, entry.rig).filter(String::isNotBlank).joinToString(" · "))
                    },
                    trailingContent = { if (ui.loading == entry.path) CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp) },
                    modifier = Modifier.clickable(enabled = ui.loading == null) { haptics.confirm(); vm.pick(entry) },
                )
            }
        }
    }
}
