package com.perseusoft.homefi.ui.sound

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material.icons.rounded.ExpandLess
import androidx.compose.material.icons.rounded.ExpandMore
import androidx.compose.material.icons.rounded.HearingDisabled
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.Checkbox
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import coil3.compose.AsyncImage
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.TasteRule
import com.perseusoft.homefi.core.TasteScope
import com.perseusoft.homefi.core.TasteSettings
import com.perseusoft.homefi.player.id
import com.perseusoft.homefi.core.tasteCombinedProfile
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.EqPanel
import com.perseusoft.homefi.player.TryResult
import com.perseusoft.homefi.player.name
import com.perseusoft.homefi.ui.AiIcon
import com.perseusoft.homefi.ui.ConfirmDialog
import com.perseusoft.homefi.ui.SelectChip
import com.perseusoft.homefi.ui.eq.EqCurve
import com.perseusoft.homefi.ui.eq.outputLabel
import com.perseusoft.homefi.ui.haptics
import com.perseusoft.homefi.ui.player.coverModel
import com.perseusoft.homefi.ui.settings.SwitchRow
import com.perseusoft.homefi.ui.settings.TasteIntensityRow
import com.perseusoft.homefi.ui.theme.Card
import com.perseusoft.homefi.ui.theme.FilledTonalButton
import com.perseusoft.homefi.ui.theme.LocalMotion
import com.perseusoft.homefi.ui.theme.LocalSkin
import com.perseusoft.homefi.ui.theme.SkinHeading
import com.perseusoft.homefi.player.SuggestionState

/** Room kept under the list so the 56 dp "Ask the AI" button never covers the last row. */
private val FAB_CLEARANCE = 88.dp
private val SOUND_ROW_HEIGHT = 56.dp
private val CURVE_PREVIEW_HEIGHT = 112.dp
private const val DISABLED_ALPHA = 0.55f

private enum class YoursTab { SOUNDS, BY_MUSIC }
private enum class MusicFilter { ALL, SONGS, ARTISTS, LISTS }

/** A sound being edited: its id, and whether the curve starts open. */
private data class Editing(val id: String, val curve: Boolean)

/**
 * Sound: one screen for what plays now. A master switch (off = bit-perfect), the song
 * playing with the sound it hears and the way to change it, the Try bar while a sound is
 * tried, "Your sounds" (by sound or by music), the output and the sound settings. The
 * "Ask the AI" button opens the assistant as a sheet.
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class, ExperimentalLayoutApi::class)
@Composable
fun SoundScreen(onClose: () -> Unit) {
    val app = LocalContext.current.homefi
    val eq by app.equalizer.settings.collectAsStateWithLifecycle()
    val taste by app.sound.taste.collectAsStateWithLifecycle()
    val now by app.sound.now.collectAsStateWithLifecycle()
    val suggestion by app.sound.suggestion.collectAsStateWithLifecycle()
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val haptics = haptics()
    var editing by rememberSaveable(stateSaver = EditingSaver) { mutableStateOf<Editing?>(null) }
    var deleting by rememberSaveable { mutableStateOf<String?>(null) }
    var moving by remember { mutableStateOf<TasteScope?>(null) }
    var yoursOpen by rememberSaveable { mutableStateOf(false) }
    var yoursTab by rememberSaveable { mutableStateOf(YoursTab.SOUNDS) }
    var musicFilter by rememberSaveable { mutableStateOf(MusicFilter.ALL) }
    var musicQuery by rememberSaveable { mutableStateOf<String?>(null) }
    val entries = remember(taste, musicFilter, musicQuery) { musicEntries(taste, musicFilter, musicQuery.orEmpty()) }
    LaunchedEffect(settings.soundSuggestions) { if (settings.soundSuggestions) app.sound.loadSuggestion() }
    BackHandler(onBack = onClose)

    Box(Modifier.fillMaxSize().background(LocalSkin.current.base).systemBarsPadding()) {
        Column(Modifier.fillMaxSize()) {
            Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = onClose, modifier = Modifier.size(TOUCH_TARGET)) { Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.back)) }
                Text(stringResource(R.string.sound_title), style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold, modifier = Modifier.weight(1f))
                Text(stringResource(if (eq.enabled) R.string.sound_on else R.string.sound_off), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Switch(
                    checked = eq.enabled,
                    onCheckedChange = { v -> haptics.toggle(v); app.equalizer.setEnabled(v) },
                    modifier = Modifier.padding(start = 8.dp, end = 12.dp),
                )
            }
            TryBar()

            LazyColumn(
                Modifier.weight(1f).alpha(if (eq.enabled) 1f else DISABLED_ALPHA),
                contentPadding = PaddingValues(bottom = FAB_CLEARANCE),
            ) {
                item(key = "output-chip") { OutputChip() }

                item(key = "now") {
                    PlayingNowCard(
                        onEdit = { id, curve -> editing = Editing(id, curve) },
                        onOutputCurve = { app.equalizer.open(EqPanel.OUTPUT_CURVE) },
                    )
                }

                if (settings.soundSuggestions) {
                    when (val s = suggestion) {
                        SuggestionState.Loading -> item(key = "suggestion") { SuggestionCard(text = null, replies = emptyList(), onReply = {}, onHide = app.sound::hideSuggestion) }
                        is SuggestionState.Ready -> item(key = "suggestion") {
                            SuggestionCard(
                                text = s.turn.text,
                                replies = if (s.turn.done) emptyList() else s.turn.quickReplies,
                                onReply = { haptics.press(); app.sound.answerSuggestion(it) },
                                onHide = app.sound::hideSuggestion,
                            )
                        }
                        else -> Unit
                    }
                }

                item(key = "yours") {
                    Row(
                        Modifier.fillMaxWidth().heightIn(min = TOUCH_TARGET).clickable { haptics.tick(); yoursOpen = !yoursOpen }.padding(horizontal = 16.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        SkinHeading(stringResource(R.string.sound_your_sounds), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary, modifier = Modifier.weight(1f))
                        Text("${taste.rules.size}", style = MaterialTheme.typography.labelLarge)
                        Icon(if (yoursOpen) Icons.Rounded.ExpandLess else Icons.Rounded.ExpandMore, null)
                    }
                }
                if (yoursOpen) {
                    item(key = "yours-tabs") {
                        Row(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            SelectChip(yoursTab == YoursTab.SOUNDS, { yoursTab = YoursTab.SOUNDS }, { Text(stringResource(R.string.sound_sounds)) })
                            SelectChip(yoursTab == YoursTab.BY_MUSIC, { yoursTab = YoursTab.BY_MUSIC }, { Text(stringResource(R.string.sound_by_music)) })
                        }
                    }
                    if (yoursTab == YoursTab.SOUNDS) {
                        soundItems(
                            now = now.rule?.id.takeUnless { now.auditioning },
                            onEdit = { editing = Editing(it, false) },
                            onDelete = { deleting = it },
                            onNew = { app.sound.newSound(emptyList())?.let { editing = Editing(it.id, true) } },
                            onEverythingElse = { app.equalizer.open(EqPanel.OUTPUT_CURVE) },
                        )
                    } else {
                        byMusicItems(entries, musicFilter, { musicFilter = it }, musicQuery, { musicQuery = it }, onMove = { moving = it })
                    }
                }

                item(key = "output") { OutputCard(onEdit = { app.equalizer.open(EqPanel.OUTPUT_CURVE) }) }
                item(key = "settings") { SoundSettingsSection() }
            }
        }

        ExtendedFloatingActionButton(
            onClick = { haptics.press(); app.sound.askAssistantWith(null) },
            icon = { Icon(AiIcon, null) },
            text = { Text(stringResource(R.string.sound_ask_ai)) },
            containerColor = MaterialTheme.colorScheme.tertiaryContainer,
            contentColor = MaterialTheme.colorScheme.onTertiaryContainer,
            modifier = Modifier.align(Alignment.BottomEnd).padding(16.dp).heightIn(min = 56.dp),
        )

        editing?.let { e ->
            EditSoundSheet(e.id, e.curve, onClose = { editing = null })
        }
    }

    deleting?.let { id ->
        ConfirmDialog(
            title = stringResource(R.string.sound_rule_delete_confirm, app.sound.labelOf(id)),
            confirm = stringResource(R.string.sound_delete),
            onConfirm = { app.sound.remove(id) },
            onDismiss = { deleting = null },
        )
    }
    moving?.let { scope -> SoundMenu(scope, onDismiss = { moving = null }, title = stringResource(R.string.sound_move_to)) }
}

private val EditingSaver = androidx.compose.runtime.saveable.Saver<Editing?, String>(
    save = { it?.let { e -> "${if (e.curve) 1 else 0}${e.id}" } ?: "" },
    restore = { s -> s.takeIf { it.isNotEmpty() }?.let { Editing(it.drop(1), it.first() == '1') } },
)

// ------------------------------------------------------------------ header chip and cards

/** "✓ WH-1000XM5 · corrected": the output playing and whether its own correction is on. */
@Composable
private fun OutputChip() {
    val app = LocalContext.current.homefi
    val now by app.sound.now.collectAsStateWithLifecycle()
    val connected by app.outputs.outputs.collectAsStateWithLifecycle()
    val id by app.equalizer.outputId.collectAsStateWithLifecycle()
    val name = outputLabel(id, connected)
    val corrected = now.eqProfile != null
    Surface(
        shape = RoundedCornerShape(50),
        color = MaterialTheme.colorScheme.secondaryContainer,
        contentColor = MaterialTheme.colorScheme.onSecondaryContainer,
        modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
    ) {
        Row(Modifier.padding(horizontal = 12.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            if (corrected) {
                Icon(Icons.Rounded.Check, null, Modifier.size(16.dp))
                Spacer(Modifier.width(6.dp))
            }
            Text(
                if (corrected) stringResource(R.string.sound_output_corrected, name) else name,
                style = MaterialTheme.typography.labelLarge,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

/** The song playing, the sound it hears and how to change it. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun PlayingNowCard(onEdit: (id: String, curve: Boolean) -> Unit, onOutputCurve: () -> Unit) {
    val app = LocalContext.current.homefi
    val now by app.sound.now.collectAsStateWithLifecycle()
    val eq by app.equalizer.settings.collectAsStateWithLifecycle()
    val player by app.player.state.collectAsStateWithLifecycle()
    val held by app.equalizer.held.collectAsStateWithLifecycle()
    val active by app.equalizer.active.collectAsStateWithLifecycle()
    val connected by app.outputs.outputs.collectAsStateWithLifecycle()
    val outputId by app.equalizer.outputId.collectAsStateWithLifecycle()
    val onlyThis by app.sound.onlyThisOutput.collectAsStateWithLifecycle()
    val haptics = haptics()
    val rule = now.rule
    val current = player.current
    val scheme = MaterialTheme.colorScheme

    Card(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
        Column(Modifier.padding(12.dp)) {
            Text(stringResource(R.string.sound_now_playing).uppercase(), style = MaterialTheme.typography.labelMedium, color = scheme.primary)
            if (current != null && now.context != null) {
                Row(Modifier.padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    AsyncImage(
                        model = current.coverModel(),
                        contentDescription = null,
                        modifier = Modifier.size(56.dp).clip(RoundedCornerShape(8.dp)).background(scheme.surfaceContainerHigh),
                    )
                    Spacer(Modifier.width(12.dp))
                    Column {
                        Text(current.title, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(current.artist, style = MaterialTheme.typography.bodySmall, color = scheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                }
            }

            // what you hear
            Column(Modifier.fillMaxWidth().padding(top = 12.dp).clip(RoundedCornerShape(8.dp)).background(scheme.surfaceContainerHigh).padding(10.dp)) {
                when {
                    !eq.enabled -> Text(stringResource(R.string.sound_bit_perfect), style = MaterialTheme.typography.bodyMedium)
                    now.context == null -> Text(stringResource(R.string.sound_play_something), style = MaterialTheme.typography.bodyMedium)
                    rule != null -> {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Text(rule.label, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
                            AiTag(rule, Modifier.padding(start = 6.dp))
                        }
                        val target = now.target
                        Text(
                            when {
                                now.auditioning -> stringResource(R.string.sound_trying_on, rule.label)
                                target != null -> stringResource(R.string.sound_rule_for, target.kindLabel(), target.name())
                                else -> app.sound.ruleWords(rule)
                            },
                            style = MaterialTheme.typography.bodySmall,
                            color = scheme.onSurfaceVariant,
                        )
                    }
                    else -> {
                        Text(
                            stringResource(if (now.eqProfile != null) R.string.sound_flat_correction else R.string.eq_flat),
                            style = MaterialTheme.typography.titleSmall,
                            fontWeight = FontWeight.Bold,
                        )
                        Text(stringResource(R.string.sound_nothing_set), style = MaterialTheme.typography.bodySmall, color = scheme.onSurfaceVariant)
                    }
                }
            }

            if (eq.enabled && now.context != null) {
                // a thin, read-only curve: tap = Edit curve
                val device = remember(eq, outputId) { app.sound.deviceProfile() }
                val small = remember(outputId) { app.sound.smallSpeaker() }
                val shown = remember(rule, device) { rule?.let { runCatching { tasteCombinedProfile(device, it, small) }.getOrNull() } ?: device }
                EqCurve(
                    profile = shown,
                    showPoints = false,
                    maxRange = 12f,
                    dimmed = held,
                    modifier = Modifier.fillMaxWidth().height(CURVE_PREVIEW_HEIGHT).padding(top = 8.dp)
                        .clickable(onClickLabel = stringResource(R.string.sound_edit_curve)) {
                            if (rule != null && !now.auditioning) onEdit(rule.id, true) else onOutputCurve()
                        },
                )
                // + Song  + Album  ✓ Eagles  + List
                if (!now.auditioning) {
                    FlowRow(Modifier.padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        now.scopes.forEach { scope ->
                            val covered = rule != null && now.coveredBy[scope] == rule.id
                            SelectChip(
                                selected = covered,
                                onClick = {
                                    when {
                                        rule != null && covered -> app.sound.unassign(rule.id, scope)
                                        rule != null -> app.sound.assign(rule.id, scope)
                                        else -> app.sound.newSound(listOf(scope))?.let { onEdit(it.id, true) }
                                    }
                                },
                                label = {
                                    Text(
                                        if (covered) "✓ " + scope.name().ifBlank { scope.kindLabel() } else "+ ${scope.kindLabel()}",
                                        maxLines = 1,
                                        overflow = TextOverflow.Ellipsis,
                                        modifier = Modifier.widthIn(max = 140.dp),
                                    )
                                },
                                modifier = Modifier.heightIn(min = TOUCH_TARGET),
                            )
                        }
                    }
                }
                if (rule == null) {
                    Row(
                        Modifier.fillMaxWidth().heightIn(min = TOUCH_TARGET).clickable { app.sound.setOnlyThisOutput(!onlyThis) },
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Checkbox(checked = onlyThis, onCheckedChange = { app.sound.setOnlyThisOutput(it) })
                        Text(stringResource(R.string.sound_only_on, outputLabel(outputId, connected)), style = MaterialTheme.typography.bodyMedium)
                    }
                }
            }

            Row(Modifier.padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                HoldToHear(held = held, enabled = active || held, onHold = app.equalizer::hold)
                FilledTonalButton(
                    onClick = { haptics.tick(); if (rule != null && !now.auditioning) onEdit(rule.id, true) else onOutputCurve() },
                    modifier = Modifier.heightIn(min = TOUCH_TARGET),
                ) { Text(stringResource(R.string.sound_edit_curve)) }
            }
        }
    }
}

/** A/B: while pressed, the sounds fade out (the level stays); let go and they fade back in. */
@Composable
private fun HoldToHear(held: Boolean, enabled: Boolean, onHold: (Boolean) -> Unit) {
    val scheme = MaterialTheme.colorScheme
    val haptics = haptics()
    val fill = if (!enabled) scheme.surfaceVariant else if (held) scheme.primary else scheme.secondaryContainer
    val ink = if (!enabled) scheme.onSurfaceVariant else if (held) scheme.onPrimary else scheme.onSecondaryContainer
    Row(
        Modifier
            .heightIn(min = TOUCH_TARGET)
            .clip(RoundedCornerShape(50))
            .background(fill)
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
            .padding(horizontal = 16.dp),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(Icons.Rounded.HearingDisabled, null, Modifier.size(18.dp), tint = ink)
        Spacer(Modifier.width(8.dp))
        Text(stringResource(if (held) R.string.eq_holding else R.string.sound_hear_without), color = ink, style = MaterialTheme.typography.labelLarge)
    }
}

// ------------------------------------------------------------------ Your sounds

private fun LazyListScope.soundItems(
    now: String?,
    onEdit: (String) -> Unit,
    onDelete: (String) -> Unit,
    onNew: () -> Unit,
    onEverythingElse: () -> Unit,
) {
    item(key = "sounds-new") {
        Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(stringResource(R.string.sound_precedence), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.weight(1f))
            TextButton(onClick = onNew, modifier = Modifier.heightIn(min = TOUCH_TARGET)) { Text("+ " + stringResource(R.string.sound_new)) }
        }
    }
    item(key = "sounds-rows") {
        val app = LocalContext.current.homefi
        val taste by app.sound.taste.collectAsStateWithLifecycle()
        val connected by app.outputs.outputs.collectAsStateWithLifecycle()
        val haptics = haptics()
        Column {
            taste.rules.forEach { rule ->
                androidx.compose.runtime.key(rule.id) {
                    SoundRow(
                        rule = rule,
                        playing = rule.id == now,
                        connected = connected,
                        onTry = {
                            haptics.tick()
                            when (app.sound.tryNow(rule)) {
                                TryResult.Playing -> Unit
                                TryResult.SoundOff -> app.sound.notify(app.getString(R.string.sound_turn_on_to_try))
                                TryResult.NothingPlaying -> app.sound.notify(app.getString(R.string.sound_how_nothing))
                            }
                        },
                        onEdit = { onEdit(rule.id) },
                        onDelete = { onDelete(rule.id) },
                    )
                }
            }
            if (taste.rules.isEmpty()) {
                Text(stringResource(R.string.sound_rules_empty), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
            }
            HorizontalDivider(Modifier.padding(horizontal = 16.dp))
            Row(Modifier.fillMaxWidth().heightIn(min = SOUND_ROW_HEIGHT).clickable(onClick = onEverythingElse).padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                KindBadge(null)
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(stringResource(R.string.sound_everything_else), style = MaterialTheme.typography.bodyLarge)
                    Text(stringResource(R.string.sound_everything_else_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                TextButton(onClick = onEverythingElse, modifier = Modifier.heightIn(min = TOUCH_TARGET)) { Text(stringResource(R.string.sound_edit)) }
            }
        }
    }
}

@Composable
private fun SoundRow(rule: TasteRule, playing: Boolean, connected: List<com.perseusoft.homefi.player.AudioOutput>, onTry: () -> Unit, onEdit: () -> Unit, onDelete: () -> Unit) {
    val scheme = MaterialTheme.colorScheme
    val outputNote = rule.outputs.takeIf { it.isNotEmpty() }?.map { outputLabel(it, connected) }?.joinToString(", ")
    Row(
        Modifier.fillMaxWidth().heightIn(min = SOUND_ROW_HEIGHT)
            .background(if (playing) scheme.secondaryContainer.copy(alpha = 0.4f) else androidx.compose.ui.graphics.Color.Transparent)
            .clickable(onClickLabel = stringResource(R.string.sound_edit), onClick = onEdit)
            .padding(start = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        KindBadge(rule.targets.firstOrNull()?.scope, empty = rule.targets.isEmpty())
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f).padding(vertical = 6.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(rule.label, style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.Medium, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
                AiTag(rule, Modifier.padding(start = 6.dp))
            }
            val meta = listOfNotNull(
                targetsSummary(rule).ifBlank { stringResource(R.string.sound_no_targets) },
                outputNote?.let { stringResource(R.string.sound_output_only, it) },
                if (playing) stringResource(R.string.sound_playing_now) else null,
            ).joinToString(" · ")
            if (meta.isNotEmpty()) Text(meta, style = MaterialTheme.typography.bodySmall, color = scheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        TextButton(onClick = onTry, modifier = Modifier.heightIn(min = TOUCH_TARGET).widthIn(min = TOUCH_TARGET)) { Text(stringResource(R.string.sound_try)) }
        IconButton(onClick = onEdit, modifier = Modifier.size(TOUCH_TARGET)) { Icon(Icons.Rounded.Edit, stringResource(R.string.sound_edit)) }
        IconButton(onClick = onDelete, modifier = Modifier.size(TOUCH_TARGET)) { Icon(Icons.Rounded.Delete, stringResource(R.string.sound_delete)) }
    }
}

private typealias MusicEntry = Pair<TasteScope, TasteRule>

/** Every target of every sound with the sound holding it, filtered as the chips and the search say. */
private fun musicEntries(taste: TasteSettings, filter: MusicFilter, query: String): List<MusicEntry> =
    taste.rules.flatMap { r -> r.targets.map { it.scope to r } }
        .filter { (scope, _) ->
            when (filter) {
                MusicFilter.ALL -> true
                MusicFilter.SONGS -> scope is TasteScope.Track
                MusicFilter.ARTISTS -> scope is TasteScope.Artist
                MusicFilter.LISTS -> scope is TasteScope.Playlist
            }
        }
        .filter { (scope, _) -> query.isBlank() || scope.name().contains(query.trim(), ignoreCase = true) }

/** "By music": the filters, then the targets under sticky kind headers (song, album, artist, playlist, genre). */
@OptIn(ExperimentalFoundationApi::class)
private fun LazyListScope.byMusicItems(
    entries: List<MusicEntry>,
    filter: MusicFilter,
    onFilter: (MusicFilter) -> Unit,
    query: String?,
    onQuery: (String?) -> Unit,
    onMove: (TasteScope) -> Unit,
) {
    item(key = "music-controls") {
        val haptics = haptics()
        Column {
            Row(Modifier.padding(horizontal = 16.dp, vertical = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                listOf(
                    MusicFilter.ALL to R.string.sound_filter_all,
                    MusicFilter.SONGS to R.string.sound_kinds_songs,
                    MusicFilter.ARTISTS to R.string.sound_kinds_artists,
                    MusicFilter.LISTS to R.string.sound_kinds_playlists,
                ).forEach { (f, label) -> SelectChip(filter == f, { onFilter(f) }, { Text(stringResource(label), maxLines = 1) }) }
                IconButton(onClick = { haptics.tick(); onQuery(if (query == null) "" else null) }, modifier = Modifier.size(TOUCH_TARGET)) {
                    Icon(if (query == null) Icons.Rounded.Search else Icons.Rounded.Close, stringResource(R.string.eq_search))
                }
            }
            if (query != null) {
                OutlinedTextField(
                    value = query,
                    onValueChange = { onQuery(it.take(80)) },
                    placeholder = { Text(stringResource(R.string.sound_search_hint)) },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
                )
            }
            if (entries.isEmpty()) {
                Text(stringResource(R.string.sound_by_music_empty), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(16.dp))
            }
        }
    }
    KIND_ORDER.forEach { kind ->
        val group = entries.filter { it.first::class == kind }
        if (group.isNotEmpty()) {
            stickyHeader(key = "music-head-${kind.simpleName}") {
                Text(
                    kindsLabel(kind).uppercase(),
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.fillMaxWidth().background(MaterialTheme.colorScheme.surface).padding(horizontal = 16.dp, vertical = 6.dp),
                )
            }
            items(group, key = { (scope, rule) -> "music-${rule.id}-${kind.simpleName}-${scope.id()}-${scope.name()}" }) { (scope, rule) ->
                MusicRow(scope, rule, onMove = { onMove(scope) })
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun MusicRow(scope: TasteScope, rule: TasteRule, onMove: () -> Unit) {
    val app = LocalContext.current.homefi
    val haptics = haptics()
    val state = rememberSwipeToDismissBoxState(
        confirmValueChange = { v ->
            if (v == SwipeToDismissBoxValue.EndToStart) {
                haptics.confirm()
                app.sound.removeTarget(rule.id, scope)
                true
            } else {
                false
            }
        },
    )
    SwipeToDismissBox(
        state = state,
        enableDismissFromStartToEnd = false,
        backgroundContent = {
            Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.errorContainer).padding(end = 16.dp), contentAlignment = Alignment.CenterEnd) {
                Icon(Icons.Rounded.Delete, null, tint = MaterialTheme.colorScheme.onErrorContainer)
            }
        },
    ) {
        Row(
            Modifier.fillMaxWidth().heightIn(min = SOUND_ROW_HEIGHT).background(MaterialTheme.colorScheme.surface).padding(start = 16.dp, end = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(scope.name().ifBlank { "?" }, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
            Surface(
                onClick = { haptics.tick(); onMove() },
                shape = RoundedCornerShape(50),
                color = MaterialTheme.colorScheme.secondaryContainer,
                contentColor = MaterialTheme.colorScheme.onSecondaryContainer,
                modifier = Modifier.heightIn(min = TOUCH_TARGET).widthIn(max = 160.dp),
            ) {
                Row(Modifier.padding(horizontal = 14.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.Center) {
                    Text(rule.label, style = MaterialTheme.typography.labelLarge, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
                    Icon(Icons.Rounded.ExpandMore, null, Modifier.size(18.dp))
                }
            }
        }
    }
}

// ------------------------------------------------------------------ output and settings

@Composable
private fun OutputCard(onEdit: () -> Unit) {
    val app = LocalContext.current.homefi
    val now by app.sound.now.collectAsStateWithLifecycle()
    val connected by app.outputs.outputs.collectAsStateWithLifecycle()
    val id by app.equalizer.outputId.collectAsStateWithLifecycle()
    Card(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
        Column(Modifier.padding(12.dp)) {
            Text(stringResource(R.string.eq_output).uppercase(), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.primary)
            Text(outputLabel(id, connected), style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold)
            Text(
                now.eqProfile?.let { stringResource(R.string.sound_correction_named, it.ifBlank { stringResource(R.string.eq_custom) }) } ?: stringResource(R.string.sound_no_correction),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            FilledTonalButton(onClick = onEdit, modifier = Modifier.padding(top = 8.dp).heightIn(min = TOUCH_TARGET)) { Text(stringResource(R.string.sound_edit_output)) }
        }
    }
}

/** Intensity, loudness, suggestions and the buttons' switches: the end of the scroll. */
@Composable
private fun SoundSettingsSection() {
    val app = LocalContext.current.homefi
    val loudness by app.sound.loudness.collectAsStateWithLifecycle()
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    Column(Modifier.padding(top = 8.dp)) {
        SkinHeading(stringResource(R.string.sound_settings_title), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
        TasteIntensityRow()
        Text(stringResource(R.string.sound_loudness), style = MaterialTheme.typography.titleSmall, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
        LoudnessControls(loudness) { app.sound.setLoudness(it) }
        SwitchRow(stringResource(R.string.sound_settings_suggestions), settings.soundSuggestions, hint = stringResource(R.string.sound_settings_suggestions_hint)) { v -> app.settings.update { it.copy(soundSuggestions = v) } }
        SwitchRow(stringResource(R.string.sound_settings_sound_button), settings.soundButton, hint = stringResource(R.string.sound_settings_sound_button_hint)) { v -> app.settings.update { it.copy(soundButton = v) } }
        SwitchRow(stringResource(R.string.sound_settings_tilt_sliders), settings.soundTiltSliders) { v -> app.settings.update { it.copy(soundTiltSliders = v) } }
    }
}
