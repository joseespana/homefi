package com.perseusoft.homefi.ui.sound

import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.Album
import androidx.compose.material.icons.rounded.Category
import androidx.compose.material.icons.rounded.Hearing
import androidx.compose.material.icons.rounded.MusicNote
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material3.AlertDialog
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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.LoudnessMode
import com.perseusoft.homefi.core.LoudnessSettings
import com.perseusoft.homefi.core.SoundReaction
import com.perseusoft.homefi.core.TasteCreator
import com.perseusoft.homefi.core.TasteScope
import com.perseusoft.homefi.core.Tilt
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.EqPanel
import com.perseusoft.homefi.player.EqTab
import com.perseusoft.homefi.player.name
import com.perseusoft.homefi.ui.QualityBadge
import com.perseusoft.homefi.ui.SelectChip
import com.perseusoft.homefi.ui.eq.formatDb
import com.perseusoft.homefi.ui.haptics
import com.perseusoft.homefi.ui.theme.numeric

/** Each taste value stays within ± this (dB), like homefi-core. */
const val MAX_TILT_DB = 6f

/** The reaction chips, in the order they are shown: label and a short description. */
val REACTIONS: List<Triple<SoundReaction, Int, Int>> = listOf(
    Triple(SoundReaction.NEEDS_BASS, R.string.sound_react_needs_bass, R.string.sound_react_needs_bass_hint),
    Triple(SoundReaction.TOO_BOOMY, R.string.sound_react_too_boomy, R.string.sound_react_too_boomy_hint),
    Triple(SoundReaction.TOO_BRIGHT, R.string.sound_react_too_bright, R.string.sound_react_too_bright_hint),
    Triple(SoundReaction.DULL, R.string.sound_react_dull, R.string.sound_react_dull_hint),
    Triple(SoundReaction.VOCALS_BURIED, R.string.sound_react_vocals, R.string.sound_react_vocals_hint),
    Triple(SoundReaction.TOO_QUIET, R.string.sound_react_quiet, R.string.sound_react_quiet_hint),
    Triple(SoundReaction.TOO_LOUD, R.string.sound_react_loud, R.string.sound_react_loud_hint),
    Triple(SoundReaction.PERFECT, R.string.sound_react_perfect, R.string.sound_react_perfect_hint),
)

fun TasteScope.icon(): ImageVector = when (this) {
    is TasteScope.Genre -> Icons.Rounded.Category
    is TasteScope.Album -> Icons.Rounded.Album
    is TasteScope.Artist -> Icons.Rounded.Person
    is TasteScope.Playlist -> Icons.AutoMirrored.Rounded.QueueMusic
    is TasteScope.Track -> Icons.Rounded.MusicNote
}

/** A chip's word for a scope: the genre's name, else "This track", "Album"... */
@Composable
fun TasteScope.chipLabel(): String = when (this) {
    is TasteScope.Genre -> name
    is TasteScope.Album -> stringResource(R.string.sound_scope_album)
    is TasteScope.Artist -> stringResource(R.string.sound_scope_artist)
    is TasteScope.Playlist -> stringResource(R.string.sound_scope_playlist)
    is TasteScope.Track -> stringResource(R.string.sound_scope_track)
}

/** "Electronic", "this album". */
@Composable
fun TasteScope.short(): String = when (this) {
    is TasteScope.Genre -> name
    is TasteScope.Album -> stringResource(R.string.sound_this_album)
    is TasteScope.Artist -> stringResource(R.string.sound_this_artist)
    is TasteScope.Playlist -> stringResource(R.string.sound_this_playlist)
    is TasteScope.Track -> stringResource(R.string.sound_this_track)
}

/** "the album Discovery", "Electronic music". */
@Composable
fun TasteScope.phrase(): String = when (this) {
    is TasteScope.Genre -> stringResource(R.string.sound_phrase_genre, name)
    is TasteScope.Album -> stringResource(R.string.sound_phrase_album, name)
    is TasteScope.Artist -> stringResource(R.string.sound_phrase_artist, name)
    is TasteScope.Playlist -> stringResource(R.string.sound_phrase_playlist, name)
    is TasteScope.Track -> stringResource(R.string.sound_phrase_track, name)
}

/** The scope a reaction uses when the user doesn't pick one: the main genre, else the artist, else the first. */
fun autoScope(scopes: List<TasteScope>): TasteScope? =
    scopes.firstOrNull { it is TasteScope.Genre } ?: scopes.firstOrNull { it is TasteScope.Artist } ?: scopes.firstOrNull()

@Composable
fun TasteCreator.label(): String = stringResource(
    when (this) {
        TasteCreator.MANUAL -> R.string.sound_by_manual
        TasteCreator.AI -> R.string.sound_by_ai
        TasteCreator.GUIDED -> R.string.sound_by_guided
        TasteCreator.REACTION -> R.string.sound_by_reaction
    },
)

/** `+2 dB bass · −1 dB treble`. */
@Composable
fun tiltWords(t: Tilt): String {
    val parts = buildList {
        if (kotlin.math.abs(t.bassDb) >= 0.05f) add(stringResource(R.string.sound_tilt_bass, formatDb(t.bassDb)))
        if (kotlin.math.abs(t.trebleDb) >= 0.05f) add(stringResource(R.string.sound_tilt_treble, formatDb(t.trebleDb)))
        val c = t.clarityDb ?: 0f
        if (kotlin.math.abs(c) >= 0.05f) add(stringResource(R.string.sound_tilt_clarity, formatDb(c)))
    }
    return if (parts.isEmpty()) stringResource(R.string.sound_tilt_none) else parts.joinToString(" · ")
}

fun LoudnessMode.labelRes(): Int = when (this) {
    LoudnessMode.OFF -> R.string.sound_loudness_off
    LoudnessMode.AUTO -> R.string.sound_loudness_auto
    LoudnessMode.TRACK -> R.string.sound_loudness_track
    LoudnessMode.ALBUM -> R.string.sound_loudness_album
}

fun LoudnessMode.hintRes(): Int = when (this) {
    LoudnessMode.OFF -> R.string.sound_loudness_off_hint
    LoudnessMode.AUTO -> R.string.sound_loudness_auto_hint
    LoudnessMode.TRACK -> R.string.sound_loudness_track_hint
    LoudnessMode.ALBUM -> R.string.sound_loudness_album_hint
}

/** The loudness mode (Off / Auto / Per song / Per album), its target and clipping guard. */
@Composable
fun LoudnessControls(loudness: LoudnessSettings, onChange: (LoudnessSettings) -> Unit) {
    val haptics = haptics()
    Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
        listOf(LoudnessMode.OFF, LoudnessMode.AUTO, LoudnessMode.TRACK, LoudnessMode.ALBUM).forEach { m ->
            SelectChip(
                selected = loudness.mode == m,
                onClick = { haptics.tick(); onChange(loudness.copy(mode = m)) },
                label = { Text(stringResource(m.labelRes())) },
                modifier = Modifier.padding(end = 8.dp),
            )
        }
    }
    SmallHint(stringResource(loudness.mode.hintRes()))
    if (loudness.mode != LoudnessMode.OFF) {
        DbSlider(
            label = stringResource(R.string.sound_loudness_target),
            value = stringResource(R.string.sound_loudness_lufs, formatDb(loudness.targetLufs).removePrefix("+")),
            position = loudness.targetLufs,
            range = -24f..-8f,
            onChange = { onChange(loudness.copy(targetLufs = Math.round(it).toFloat())) },
        )
        ListItem(
            colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surface),
            headlineContent = { Text(stringResource(R.string.sound_loudness_clipping)) },
            trailingContent = {
                Switch(checked = loudness.preventClipping, onCheckedChange = { v -> haptics.toggle(v); onChange(loudness.copy(preventClipping = v)) })
            },
        )
        SmallHint(stringResource(R.string.sound_loudness_needs_tags))
    }
}

@Composable
fun SmallHint(text: String, modifier: Modifier = Modifier) {
    Text(
        text,
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp),
    )
}

@Composable
fun DbSlider(
    label: String,
    value: String,
    position: Float,
    range: ClosedFloatingPointRange<Float>,
    inset: androidx.compose.ui.unit.Dp = 16.dp,
    onChange: (Float) -> Unit,
) {
    Column(Modifier.fillMaxWidth().padding(horizontal = inset)) {
        Row(Modifier.fillMaxWidth().padding(top = 4.dp)) {
            Text(label, style = MaterialTheme.typography.labelLarge, modifier = Modifier.weight(1f))
            Text(value, style = MaterialTheme.typography.labelLarge.numeric(), color = MaterialTheme.colorScheme.primary)
        }
        Slider(value = position.coerceIn(range), onValueChange = onChange, valueRange = range)
    }
}

/** Bass, treble and clarity sliders (±[MAX_TILT_DB], half-dB steps). */
@Composable
fun TiltSliders(tilt: Tilt, inset: androidx.compose.ui.unit.Dp = 16.dp, onChange: (Tilt) -> Unit) {
    val r = -MAX_TILT_DB..MAX_TILT_DB
    fun step(v: Float) = Math.round(v * 2) / 2f
    DbSlider(stringResource(R.string.eq_bass), stringResource(R.string.eq_db, formatDb(tilt.bassDb)), tilt.bassDb, r, inset) { onChange(tilt.copy(bassDb = step(it))) }
    DbSlider(stringResource(R.string.eq_treble), stringResource(R.string.eq_db, formatDb(tilt.trebleDb)), tilt.trebleDb, r, inset) { onChange(tilt.copy(trebleDb = step(it))) }
    val c = tilt.clarityDb ?: 0f
    DbSlider(stringResource(R.string.eq_clarity), stringResource(R.string.eq_db, formatDb(c)), c, r, inset) {
        onChange(tilt.copy(clarityDb = step(it).takeIf { v -> v != 0f }))
    }
}

/** A small tilt editor: name, bass / treble / clarity; [onDelete] adds a delete button. */
@Composable
fun TiltDialog(
    title: String,
    subtitle: String?,
    initial: Tilt,
    initialLabel: String?,
    onDismiss: () -> Unit,
    onSave: (Tilt, String?) -> Unit,
    onDelete: (() -> Unit)? = null,
) {
    var tilt by remember { mutableStateOf(initial) }
    var label by rememberSaveable { mutableStateOf(initialLabel.orEmpty()) }
    val haptics = haptics()
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                subtitle?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
                if (initialLabel != null) {
                    OutlinedTextField(
                        value = label,
                        onValueChange = { label = it.take(60) },
                        label = { Text(stringResource(R.string.sound_rule_name)) },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
                    )
                }
                TiltSliders(tilt, inset = 0.dp) { tilt = it }
                if (onDelete != null) {
                    TextButton(onClick = { haptics.tick(); onDelete() }) {
                        Text(stringResource(R.string.sound_rule_delete), color = MaterialTheme.colorScheme.error)
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = { haptics.confirm(); onSave(tilt, label.takeIf { initialLabel != null }) }) { Text(stringResource(R.string.sound_save)) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

/**
 * The signal badge while anything works on what this phone plays: "EQ · HD 650 + Electronic
 * · −3.2 dB" (or "EQ · not bit-perfect" with the layer names off). A tap opens the equalizer.
 */
@Composable
fun SoundBadge(heardHere: Boolean, modifier: Modifier = Modifier) {
    val app = LocalContext.current.homefi
    val now by app.sound.now.collectAsStateWithLifecycle()
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    if (!now.active || !heardHere) return
    val text = if (settings.soundLayerChip) {
        val layers = listOfNotNull(
            now.eqProfile?.ifBlank { stringResource(R.string.eq_custom) },
            now.rule?.scope?.name()?.ifBlank { null },
        ).joinToString(" + ")
        val gain = now.loudnessDb.takeIf { kotlin.math.abs(it) >= 0.05f }?.let { stringResource(R.string.eq_db, formatDb(it)) }
        val parts = listOf(layers, gain.orEmpty()).filter(String::isNotEmpty).joinToString(" · ")
        if (parts.isEmpty()) stringResource(R.string.eq_not_bitperfect) else stringResource(R.string.eq_badge_layers, parts)
    } else {
        stringResource(R.string.eq_not_bitperfect)
    }
    QualityBadge(
        text,
        modifier
            .clip(RoundedCornerShape(50))
            .clickable {
                app.equalizer.open(EqPanel.EQUALIZER, if (now.rule != null && now.eqProfile == null) EqTab.TASTE else null)
            },
    )
}

/** The player's visible "How does it sound?" button (hidden by its setting or for a hub's song). */
@Composable
fun HowSoundsButton(remote: Boolean, size: androidx.compose.ui.unit.Dp = 32.dp) {
    val app = LocalContext.current.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    if (!settings.soundReactButton || remote) return
    val haptics = haptics()
    IconButton(onClick = { haptics.press(); app.sound.openHow(true) }, modifier = Modifier.size(size)) {
        Icon(Icons.Rounded.Hearing, stringResource(R.string.sound_how), Modifier.size(size * 0.62f), tint = MaterialTheme.colorScheme.primary)
    }
}
