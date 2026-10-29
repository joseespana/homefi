package com.perseusoft.homefi.ui.sound

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material.icons.rounded.Layers
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
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
import com.perseusoft.homefi.core.SoundReaction
import com.perseusoft.homefi.core.TasteScope
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.EqPanel
import com.perseusoft.homefi.player.EqTab
import com.perseusoft.homefi.player.SoundLayers
import com.perseusoft.homefi.ui.SelectChip
import com.perseusoft.homefi.ui.haptics
import kotlinx.coroutines.launch

/**
 * "How does it sound?": where the change goes (Auto = the main genre, or a scope the user
 * picks) and the eight reactions. One tap changes the sound for that music only; a note
 * says what changed, with Undo (see [SoundNotices]).
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun HowSoundsSheet(onDismiss: () -> Unit) {
    val app = LocalContext.current.homefi
    val now by app.sound.now.collectAsStateWithLifecycle()
    val busy by app.sound.busy.collectAsStateWithLifecycle()
    val haptics = haptics()
    val scopes = now.scopes
    // Auto: the layer that plays now (a narrower rule would hide a new genre rule), else the main genre
    val playingScope = now.rule?.scope
    val auto = remember(scopes, playingScope) {
        playingScope?.let { r -> scopes.firstOrNull { SoundLayers.sameScope(it, r) } ?: r } ?: autoScope(scopes)
    }
    // -1 = automatic; an index into scopes otherwise
    var picked by rememberSaveable(now.mediaId) { mutableIntStateOf(-1) }
    val chosen: TasteScope? = scopes.getOrNull(picked)

    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).navigationBarsPadding().padding(bottom = 12.dp)) {
            Text(
                stringResource(R.string.sound_how),
                style = MaterialTheme.typography.titleLarge,
                fontWeight = FontWeight.Bold,
                modifier = Modifier.padding(horizontal = 16.dp),
            )
            if (now.context == null) {
                SmallHint(stringResource(R.string.sound_how_nothing))
                return@Column
            }
            SmallHint(stringResource(R.string.sound_how_hint, (chosen ?: auto)?.short() ?: stringResource(R.string.sound_this_music)))

            if (scopes.size > 1) {
                FlowRow(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    SelectChip(
                        selected = picked < 0,
                        onClick = { haptics.tick(); picked = -1 },
                        label = {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Icon(Icons.Rounded.AutoAwesome, null, Modifier.size(14.dp))
                                Spacer(Modifier.width(4.dp))
                                Text(auto?.let { stringResource(R.string.sound_scope_auto_named, it.chipLabel()) } ?: stringResource(R.string.sound_scope_auto), maxLines = 1, overflow = TextOverflow.Ellipsis)
                            }
                        },
                    )
                    scopes.forEachIndexed { i, s ->
                        if (s !== auto) {
                            key(i) {
                                SelectChip(
                                    selected = picked == i,
                                    onClick = { haptics.tick(); picked = i },
                                    label = {
                                        Row(verticalAlignment = Alignment.CenterVertically) {
                                            Icon(s.icon(), null, Modifier.size(14.dp))
                                            Spacer(Modifier.width(4.dp))
                                            Text(s.chipLabel(), maxLines = 1, overflow = TextOverflow.Ellipsis)
                                        }
                                    },
                                )
                            }
                        }
                    }
                }
            }

            // two columns of reactions; "Sounds perfect" stands out
            Column(Modifier.padding(horizontal = 12.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                REACTIONS.chunked(2).forEach { row ->
                    Row(Modifier.height(IntrinsicSize.Min), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        row.forEach { (reaction, label, hint) ->
                            ReactionCard(
                                label = stringResource(label),
                                hint = stringResource(if (busy == reaction) R.string.sound_adjusting else hint),
                                perfect = reaction == SoundReaction.PERFECT,
                                enabled = busy == null,
                                modifier = Modifier.weight(1f).fillMaxHeight(),
                            ) {
                                haptics.tick()
                                app.sound.react(reaction, chosen ?: playingScope)
                            }
                        }
                    }
                }
            }

            Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                val rule = now.rule
                if (rule != null) {
                    Icon(Icons.Rounded.Layers, null, Modifier.size(16.dp), tint = MaterialTheme.colorScheme.primary)
                    Spacer(Modifier.width(6.dp))
                    Column(Modifier.weight(1f)) {
                        Text(rule.label, style = MaterialTheme.typography.bodyMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(LocalContext.current.homefi.sound.ruleWords(rule), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1)
                    }
                } else {
                    Text(
                        stringResource(R.string.sound_no_layer),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.weight(1f),
                    )
                }
                TextButton(onClick = { onDismiss(); app.equalizer.open(EqPanel.EQUALIZER, EqTab.TASTE) }) { Text(stringResource(R.string.sound_rules)) }
            }
        }
    }
}

@Composable
private fun ReactionCard(label: String, hint: String, perfect: Boolean, enabled: Boolean, modifier: Modifier, onClick: () -> Unit) {
    val scheme = MaterialTheme.colorScheme
    Surface(
        onClick = onClick,
        enabled = enabled,
        shape = RoundedCornerShape(12.dp),
        color = if (perfect) scheme.primaryContainer else scheme.surfaceContainerHigh,
        contentColor = if (perfect) scheme.onPrimaryContainer else scheme.onSurface,
        border = if (perfect) BorderStroke(1.dp, scheme.primary) else null,
        modifier = modifier,
    ) {
        Column(Modifier.padding(horizontal = 12.dp, vertical = 10.dp)) {
            Text(label, style = MaterialTheme.typography.labelLarge, fontWeight = FontWeight.Bold, maxLines = 2)
            Text(hint, style = MaterialTheme.typography.bodySmall, color = if (perfect) scheme.onPrimaryContainer else scheme.onSurfaceVariant, maxLines = 2)
        }
    }
}

/**
 * The reactions' notes, over every screen: what changed and Undo. A change that took
 * gives the confirming buzz.
 */
@Composable
fun SoundNotices(host: SnackbarHostState) {
    val app = LocalContext.current.homefi
    val haptics = haptics()
    val undo = stringResource(R.string.sound_undo)
    LaunchedEffect(host) {
        app.sound.notices.collect { n ->
            if (n.undoToken != null) haptics.confirm()
            launch {
                host.currentSnackbarData?.dismiss()
                val result = host.showSnackbar(
                    n.message,
                    actionLabel = n.undoToken?.let { undo },
                    withDismissAction = n.undoToken == null,
                    duration = if (n.undoToken != null) SnackbarDuration.Long else SnackbarDuration.Short,
                )
                val token = n.undoToken
                if (result == SnackbarResult.ActionPerformed && token != null) app.sound.undo(token)
            }
        }
    }
}

/**
 * "Sound for this…" on an album, playlist or artist: ask the assistant about it, or set a
 * tilt by hand (the rule is saved and heard when that music plays).
 */
@Composable
fun SoundForDialog(scope: TasteScope, onDismiss: () -> Unit) {
    val app = LocalContext.current.homefi
    val taste by app.sound.taste.collectAsStateWithLifecycle()
    val existing = remember(taste, scope) { app.sound.ruleFor(scope) }
    var manual by rememberSaveable { mutableStateOf(false) }
    val haptics = haptics()
    val phrase = scope.phrase()
    if (manual) {
        val aiOn = remember { app.ai.hasProvider() }
        TasteRuleDialog(
            title = stringResource(R.string.sound_for_title, phrase),
            subtitle = stringResource(R.string.sound_for_hint, phrase),
            scope = scope,
            initial = existing,
            onDismiss = onDismiss,
            onSave = { tilt, bands, label ->
                app.sound.setRule(scope, tilt, label, bands = bands)
                onDismiss()
            },
            onDelete = existing?.let { r -> { app.sound.removeRule(r.id); onDismiss() } },
            onRefine = if (aiOn) { tilt, bands, label ->
                app.sound.setRule(scope, tilt, label, bands = bands)
                app.sound.ruleFor(scope)?.let(app.sound::refine)
                onDismiss()
            } else null,
        )
        return
    }
    AlertDialog(
        onDismissRequest = onDismiss,
        icon = { Icon(scope.icon(), null) },
        title = { Text(stringResource(R.string.sound_for_title, phrase), maxLines = 2, overflow = TextOverflow.Ellipsis) },
        text = {
            Column {
                Text(stringResource(R.string.sound_for_hint, phrase))
                existing?.let {
                    Spacer(Modifier.size(8.dp))
                    Text(
                        stringResource(R.string.sound_for_existing, "${it.label} · ${app.sound.ruleWords(it)}"),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.primary,
                    )
                }
            }
        },
        confirmButton = {
            TextButton(onClick = { haptics.press(); app.sound.askAssistant(scope) }) { Text(stringResource(R.string.sound_for_ask)) }
        },
        dismissButton = {
            TextButton(onClick = { haptics.tick(); manual = true }) { Text(stringResource(R.string.sound_for_manual)) }
        },
    )
}
