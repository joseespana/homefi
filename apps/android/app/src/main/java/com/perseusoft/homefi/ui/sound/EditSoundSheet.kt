package com.perseusoft.homefi.ui.sound

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.ui.focus.focusRequester
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.TargetHit
import com.perseusoft.homefi.core.TargetSource
import com.perseusoft.homefi.core.TasteRule
import com.perseusoft.homefi.core.TasteScope
import com.perseusoft.homefi.core.tasteCurveProfile
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.TryResult
import com.perseusoft.homefi.player.name
import com.perseusoft.homefi.ui.ConfirmDialog
import com.perseusoft.homefi.ui.SelectChip
import com.perseusoft.homefi.ui.eq.EqCurve
import com.perseusoft.homefi.ui.eq.outputLabel
import com.perseusoft.homefi.ui.haptics
import com.perseusoft.homefi.ui.theme.FilledTonalButton
import com.perseusoft.homefi.ui.theme.LocalSkin
import kotlinx.coroutines.delay

private const val SEARCH_DEBOUNCE_MS = 300L

/**
 * Edit sound: its name, curve, the outputs it plays on and everything it applies to (with a
 * search to add more). Full screen over Sound; back saves what was drawn. A sound is one
 * rule for many targets; the core keeps each target in one sound per output.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun EditSoundSheet(soundId: String, openCurve: Boolean, onClose: () -> Unit) {
    val app = LocalContext.current.homefi
    val taste by app.sound.taste.collectAsStateWithLifecycle()
    val rule = taste.rules.firstOrNull { it.id == soundId }
    if (rule == null) {
        LaunchedEffect(Unit) { onClose() }
        return
    }
    val haptics = haptics()
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val audition by app.sound.audition.collectAsStateWithLifecycle()
    val connected by app.outputs.outputs.collectAsStateWithLifecycle()
    val playingOn by app.equalizer.outputId.collectAsStateWithLifecycle()

    var tilt by remember(soundId) { mutableStateOf(rule.tilt) }
    var bands by remember(soundId) { mutableStateOf(rule.bands) }
    var curveOpen by rememberSaveable(soundId) { mutableStateOf(openCurve) }
    var adding by rememberSaveable(soundId) { mutableStateOf(false) }
    var renaming by rememberSaveable(soundId) { mutableStateOf(false) }
    var deleting by rememberSaveable(soundId) { mutableStateOf(false) }
    val draft = remember(rule, tilt, bands) { rule.copy(tilt = tilt, bands = bands) }
    val dirty = tilt != rule.tilt || bands != rule.bands

    // a try that is running follows the curve being drawn
    LaunchedEffect(tilt, bands) {
        if (dirty && audition?.rule?.id == soundId) app.sound.tryNow(draft)
    }
    fun saveAndClose() {
        if (dirty) app.sound.save(draft)
        if (audition?.rule?.id == soundId) app.sound.stopTrying()
        onClose()
    }
    BackHandler { if (adding) adding = false else saveAndClose() }

    Column(Modifier.fillMaxSize().background(LocalSkin.current.base).systemBarsPadding().imePadding()) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = ::saveAndClose, modifier = Modifier.size(TOUCH_TARGET)) { Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.back)) }
            Text(rule.label, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
            AiTag(rule, Modifier.padding(start = 8.dp, end = 4.dp))
            TextButton(
                onClick = {
                    haptics.tick()
                    when (app.sound.tryNow(draft)) {
                        TryResult.Playing -> Unit
                        TryResult.SoundOff -> app.sound.notify(app.getString(R.string.sound_turn_on_to_try))
                        TryResult.NothingPlaying -> app.sound.notify(app.getString(R.string.sound_how_nothing))
                    }
                },
                modifier = Modifier.heightIn(min = TOUCH_TARGET),
            ) { Text(stringResource(R.string.sound_try)) }
        }
        TryBar()

        Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp).padding(bottom = 24.dp)) {
            val curve = remember(draft) { runCatching { tasteCurveProfile(draft) }.getOrNull() }
            EqCurve(profile = curve, curve = curve, showPoints = false, maxRange = 12f, modifier = Modifier.fillMaxWidth().height(110.dp))
            Text(
                app.sound.ruleWords(draft),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp),
            )
            FlowRow(Modifier.padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                FilledTonalButton(onClick = { haptics.tick(); curveOpen = !curveOpen }, modifier = Modifier.heightIn(min = TOUCH_TARGET)) { Text(stringResource(R.string.sound_edit_curve)) }
                FilledTonalButton(onClick = { haptics.tick(); renaming = true }, modifier = Modifier.heightIn(min = TOUCH_TARGET)) { Text(stringResource(R.string.sound_rename)) }
                TextButton(onClick = { haptics.tick(); deleting = true }, modifier = Modifier.heightIn(min = TOUCH_TARGET)) {
                    Text(stringResource(R.string.sound_delete), color = MaterialTheme.colorScheme.error)
                }
            }
            if (curveOpen) {
                val aiOn = remember { app.ai.hasProvider() }
                TasteCurveEditor(
                    tilt = tilt,
                    bands = bands,
                    onTilt = { tilt = it },
                    onBands = { bands = it },
                    onRefine = if (aiOn && !ruleNeutral(tilt, bands)) { { app.sound.save(draft); app.sound.refine(draft) } } else null,
                    tiltSliders = settings.soundTiltSliders,
                )
            }

            // ---- plays on
            SectionTitle(stringResource(R.string.sound_plays_on))
            val ids = remember(connected, playingOn, rule.outputs) { (listOf(playingOn) + connected.map { it.eqId } + rule.outputs).distinct() }
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                SelectChip(
                    selected = rule.outputs.isEmpty(),
                    onClick = { app.sound.setOutputs(soundId, emptyList()) },
                    label = { Text(stringResource(R.string.sound_all_outputs)) },
                )
                ids.forEach { id ->
                    SelectChip(
                        selected = id in rule.outputs,
                        onClick = { app.sound.setOutputs(soundId, if (id in rule.outputs) rule.outputs - id else rule.outputs + id) },
                        label = { Text(outputLabel(id, connected)) },
                    )
                }
            }

            // ---- applies to
            Row(Modifier.fillMaxWidth().padding(top = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(stringResource(R.string.sound_applies_to).uppercase(), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.primary, modifier = Modifier.weight(1f))
                FilledTonalButton(onClick = { haptics.tick(); adding = !adding }, modifier = Modifier.heightIn(min = TOUCH_TARGET)) {
                    Icon(if (adding) Icons.Rounded.Close else Icons.Rounded.Add, null, Modifier.size(18.dp))
                    Spacer(Modifier.width(4.dp))
                    Text(stringResource(if (adding) R.string.sound_done else R.string.sound_add))
                }
            }
            if (adding) TargetSearch(rule, onAdded = { adding = false })
            if (rule.targets.isEmpty()) {
                Text(
                    stringResource(R.string.sound_applies_to_nothing),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(vertical = 12.dp),
                )
            }
            KIND_ORDER.forEach { kind ->
                val group = rule.targets.filter { it.scope::class == kind }
                if (group.isNotEmpty()) {
                    Text(kindsLabel(kind), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 12.dp, bottom = 4.dp))
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        group.forEach { t -> TargetChip(t.scope.name().ifBlank { "?" }) { haptics.tick(); app.sound.unassign(soundId, t.scope) } }
                    }
                }
            }
        }
    }

    if (renaming) RenameDialog(rule.label, onDismiss = { renaming = false }) { name -> renaming = false; app.sound.save(draft.copy(label = name)) }
    if (deleting) {
        ConfirmDialog(
            title = stringResource(R.string.sound_rule_delete_confirm, rule.label),
            confirm = stringResource(R.string.sound_delete),
            onConfirm = { app.sound.remove(soundId); onClose() },
            onDismiss = { deleting = false },
        )
    }
}

@Composable
private fun SectionTitle(text: String) {
    Text(text.uppercase(), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(top = 16.dp, bottom = 8.dp))
}

/** A target with its remove button (48 dp to hit). */
@Composable
private fun TargetChip(name: String, onRemove: () -> Unit) {
    Surface(shape = RoundedCornerShape(50), color = MaterialTheme.colorScheme.secondaryContainer, contentColor = MaterialTheme.colorScheme.onSecondaryContainer) {
        Row(Modifier.heightIn(min = TOUCH_TARGET).padding(start = 14.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(name, style = MaterialTheme.typography.labelLarge, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.widthIn(max = 200.dp))
            IconButton(onClick = onRemove, modifier = Modifier.size(TOUCH_TARGET)) { Icon(Icons.Rounded.Close, stringResource(R.string.sound_remove_target, name), Modifier.size(18.dp)) }
        }
    }
}

@Composable
private fun RenameDialog(initial: String, onDismiss: () -> Unit, onSave: (String) -> Unit) {
    var name by rememberSaveable { mutableStateOf(initial) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.sound_rename)) },
        text = {
            OutlinedTextField(value = name, onValueChange = { name = it.take(60) }, label = { Text(stringResource(R.string.sound_rule_name)) }, singleLine = true)
        },
        confirmButton = { TextButton(onClick = { onSave(name.trim()) }, enabled = name.isNotBlank()) { Text(stringResource(R.string.sound_save)) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

/**
 * Search for something to add: the library and Tidal, ranked by the core. Typing waits
 * [SEARCH_DEBOUNCE_MS]; a new query cancels the last search. A result already in another
 * sound says so (it moves, with Undo).
 */
@Composable
private fun TargetSearch(rule: TasteRule, onAdded: () -> Unit) {
    val app = LocalContext.current.homefi
    val haptics = haptics()
    var query by rememberSaveable { mutableStateOf("") }
    var results by remember { mutableStateOf<List<TargetHit>?>(null) }
    LaunchedEffect(query) {
        if (query.isBlank()) { results = null; return@LaunchedEffect }
        delay(SEARCH_DEBOUNCE_MS)
        results = app.sound.searchTargets(query)
    }
    val focus = remember { androidx.compose.ui.focus.FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }
    OutlinedTextField(
        value = query,
        onValueChange = { query = it.take(80) },
        placeholder = { Text(stringResource(R.string.sound_search_hint), maxLines = 1, overflow = TextOverflow.Ellipsis) },
        singleLine = true,
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
        modifier = Modifier.fillMaxWidth().padding(top = 8.dp).focusRequester(focus),
    )
    val found = results
    if (found != null && found.isEmpty()) {
        Text(stringResource(R.string.sound_search_none), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(vertical = 8.dp))
    }
    found?.forEach { hit ->
        val other = hit.inSound?.takeIf { it != rule.id }
        Column(
            Modifier.fillMaxWidth().heightIn(min = 56.dp).clickable {
                haptics.confirm()
                app.sound.assign(rule.id, hit.scope)
                onAdded()
            }.padding(vertical = 4.dp),
            verticalArrangement = Arrangement.Center,
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                KindBadge(hit.scope)
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(hit.scope.name(), style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(
                        listOf(hit.scope.kindLabel(), hit.subtitle).filter { it.isNotBlank() }.joinToString(" · "),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                Text(
                    stringResource(if (hit.source == TargetSource.TIDAL) R.string.sound_source_tidal else R.string.sound_source_library),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            if (other != null) {
                Text(
                    stringResource(R.string.sound_moves_from, hit.scope.name(), app.sound.labelOf(other)),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.tertiary,
                    modifier = Modifier.padding(start = 48.dp),
                )
            }
        }
    }
}
