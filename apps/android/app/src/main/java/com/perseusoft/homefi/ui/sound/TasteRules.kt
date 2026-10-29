package com.perseusoft.homefi.ui.sound

import com.perseusoft.homefi.ui.AiIcon
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Hearing
import androidx.compose.material.icons.rounded.Layers
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
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
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.TasteRule
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.SuggestionState
import com.perseusoft.homefi.ui.ConfirmDialog
import com.perseusoft.homefi.ui.QualityBadge
import com.perseusoft.homefi.ui.eq.formatDb
import com.perseusoft.homefi.ui.haptics
import com.perseusoft.homefi.ui.theme.Button
import com.perseusoft.homefi.ui.theme.Card
import com.perseusoft.homefi.ui.theme.FilledTonalButton
import com.perseusoft.homefi.ui.theme.SkinHeading

/**
 * The equalizer's "Taste rules" tab: a suggestion, what plays now and its layer, the
 * switch for every layer, loudness, and the rules (on/off, edit, delete).
 */
@Composable
fun TasteRulesTab() {
    val app = LocalContext.current.homefi
    val taste by app.sound.taste.collectAsStateWithLifecycle()
    val loudness by app.sound.loudness.collectAsStateWithLifecycle()
    val now by app.sound.now.collectAsStateWithLifecycle()
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val suggestion by app.sound.suggestion.collectAsStateWithLifecycle()
    val haptics = haptics()
    var editing by rememberSaveable { mutableStateOf<String?>(null) }
    var deleting by rememberSaveable { mutableStateOf<String?>(null) }
    LaunchedEffect(settings.soundSuggestions) { if (settings.soundSuggestions) app.sound.loadSuggestion() }

    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
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

        item(key = "now") {
            Card(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
                Column(Modifier.padding(12.dp)) {
                    Text(stringResource(R.string.sound_now_playing), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.primary)
                    if (now.context == null) {
                        Text(stringResource(R.string.sound_now_nothing), style = MaterialTheme.typography.bodyMedium)
                    } else {
                        Text(now.title.orEmpty(), style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        val rule = now.rule
                        Row(Modifier.padding(top = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                            Icon(Icons.Rounded.Layers, null, Modifier.size(16.dp), tint = if (rule != null) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant)
                            Spacer(Modifier.width(6.dp))
                            Text(
                                if (rule != null) stringResource(R.string.sound_layer_now, "${rule.label} · ${app.sound.ruleWords(rule)}") else stringResource(R.string.sound_no_layer),
                                style = MaterialTheme.typography.bodySmall,
                                maxLines = 2,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                        if (kotlin.math.abs(now.loudnessDb) >= 0.05f) {
                            Text(stringResource(R.string.sound_loudness_now, formatDb(now.loudnessDb)), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                        FilledTonalButton(onClick = { haptics.press(); app.sound.openHow(true) }, modifier = Modifier.padding(top = 8.dp)) {
                            Icon(Icons.Rounded.Hearing, null, Modifier.size(18.dp))
                            Spacer(Modifier.width(6.dp))
                            Text(stringResource(R.string.sound_how))
                        }
                    }
                }
            }
        }

        item(key = "switch") {
            ListItem(
                colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surface),
                headlineContent = { Text(stringResource(R.string.sound_layers_on)) },
                supportingContent = { Text(stringResource(R.string.sound_layers_hint)) },
                trailingContent = { Switch(checked = taste.enabled, onCheckedChange = { v -> haptics.toggle(v); app.sound.setEnabled(v) }) },
            )
        }

        item(key = "loudness") {
            Column {
                HorizontalDivider(Modifier.padding(vertical = 8.dp))
                Heading(stringResource(R.string.sound_loudness))
                LoudnessControls(loudness) { app.sound.setLoudness(it) }
                HorizontalDivider(Modifier.padding(vertical = 8.dp))
                Heading(stringResource(R.string.sound_rules_count, taste.rules.size))
                if (taste.rules.isEmpty()) SmallHint(stringResource(R.string.sound_rules_empty))
            }
        }

        items(taste.rules, key = { it.id }) { rule ->
            RuleRow(
                rule = rule,
                dimmed = !taste.enabled,
                onToggle = { v -> haptics.toggle(v); app.sound.patchRule(rule.id) { it.copy(enabled = v) } },
                onClick = { haptics.tick(); editing = rule.id },
            )
        }
    }

    editing?.let { id ->
        val rule = taste.rules.firstOrNull { it.id == id }
        if (rule == null) {
            editing = null
        } else {
            val aiOn = remember { app.ai.hasProvider() }
            fun save(tilt: com.perseusoft.homefi.core.Tilt, bands: List<com.perseusoft.homefi.core.TasteBand>, label: String?) {
                app.sound.patchRule(rule.id) { it.copy(tilt = tilt, bands = bands, label = label?.trim()?.ifEmpty { null } ?: it.label) }
            }
            TasteRuleDialog(
                title = stringResource(R.string.sound_rule_edit),
                subtitle = rule.scope.phrase(),
                scope = rule.scope,
                initial = rule,
                onDismiss = { editing = null },
                onSave = { tilt, bands, label ->
                    save(tilt, bands, label)
                    editing = null
                },
                onDelete = { editing = null; deleting = rule.id },
                onRefine = if (aiOn) { tilt, bands, label ->
                    save(tilt, bands, label)
                    editing = null
                    app.sound.taste.value.rules.firstOrNull { it.id == rule.id }?.let(app.sound::refine)
                } else null,
            )
        }
    }
    deleting?.let { id ->
        val rule = taste.rules.firstOrNull { it.id == id }
        ConfirmDialog(
            title = stringResource(R.string.sound_rule_delete_confirm, rule?.label.orEmpty()),
            confirm = stringResource(R.string.sound_rule_delete),
            onConfirm = { app.sound.removeRule(id) },
            onDismiss = { deleting = null },
        )
    }
}

@Composable
private fun Heading(text: String) {
    SkinHeading(
        text,
        style = MaterialTheme.typography.titleSmall,
        color = MaterialTheme.colorScheme.primary,
        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
    )
}

@Composable
private fun RuleRow(rule: TasteRule, dimmed: Boolean, onToggle: (Boolean) -> Unit, onClick: () -> Unit) {
    val scheme = MaterialTheme.colorScheme
    val on = rule.enabled && !dimmed
    ListItem(
        colors = ListItemDefaults.colors(containerColor = scheme.surface),
        leadingContent = { Icon(rule.scope.icon(), null, tint = if (on) scheme.primary else scheme.onSurfaceVariant) },
        headlineContent = { Text(rule.label, maxLines = 1, overflow = TextOverflow.Ellipsis, color = if (on) scheme.onSurface else scheme.onSurfaceVariant) },
        supportingContent = {
            Column {
                Text(rule.scope.phrase(), maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(LocalContext.current.homefi.sound.ruleWords(rule), style = MaterialTheme.typography.bodySmall)
                rule.explanation.ifBlank { null }?.let { Text(it, style = MaterialTheme.typography.bodySmall, maxLines = 2, overflow = TextOverflow.Ellipsis) }
                QualityBadge(rule.createdBy.label(), Modifier.padding(top = 4.dp))
            }
        },
        trailingContent = { Switch(checked = rule.enabled, onCheckedChange = onToggle) },
        modifier = Modifier.clickable(onClickLabel = stringResource(R.string.sound_rule_edit), onClick = onClick),
    )
}

/** A proactive suggestion: nothing changes before a yes, which goes on in the assistant. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun SuggestionCard(text: String?, replies: List<String>, onReply: (String) -> Unit, onHide: () -> Unit) {
    Card(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
        Row(Modifier.padding(start = 12.dp, top = 12.dp, bottom = 12.dp), verticalAlignment = Alignment.Top) {
            Icon(AiIcon, null, tint = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(top = 2.dp))
            Spacer(Modifier.width(10.dp))
            Column(Modifier.weight(1f)) {
                Text(stringResource(R.string.sound_suggestion), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.primary)
                if (text == null) {
                    CircularProgressIndicator(Modifier.padding(vertical = 8.dp).size(20.dp), strokeWidth = 2.dp)
                } else {
                    Text(text, style = MaterialTheme.typography.bodyMedium)
                    if (replies.isNotEmpty()) {
                        FlowRow(Modifier.padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            replies.forEachIndexed { i, r ->
                                if (i == 0) Button(onClick = { onReply(r) }) { Text(r) } else FilledTonalButton(onClick = { onReply(r) }) { Text(r) }
                            }
                        }
                    }
                }
            }
            IconButton(onClick = onHide) { Icon(Icons.Rounded.Close, stringResource(R.string.sound_suggestion_hide)) }
        }
    }
}
