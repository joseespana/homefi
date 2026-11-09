package com.perseusoft.homefi.ui.exercise

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import com.perseusoft.homefi.ui.tvpair.TvPairingBlock
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.Send
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.selection.selectable
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Remove
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import com.perseusoft.homefi.core.PresetId
import com.perseusoft.homefi.core.RideSegment
import com.perseusoft.homefi.core.exercisePreset
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.CardDefaults
import androidx.compose.ui.draw.shadow
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.TvAudio
import com.perseusoft.homefi.core.exerciseHealthNotice
import com.perseusoft.homefi.core.exerciseNeedsAiMessage
import com.perseusoft.homefi.core.exercisePresets
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.exercise.RideChat
import com.perseusoft.homefi.player.exercise.RideForm
import com.perseusoft.homefi.ui.playlists.languageTag
import com.perseusoft.homefi.ui.theme.Card

private const val MIN_MINUTES = 10
private const val MAX_MINUTES = 90
private const val MINUTES_STEP = 5
private val STEP_BUTTON = 56.dp
private const val BIG_MINUTES_SP = 56
private val QUICK_MINUTES = listOf(20, 30, 45, 60)

/** A section's trailing text this short is a value beside the title; longer, a hint under it. */
private const val SHORT_TRAILING = 12
private val STYLE_MOUNTAIN = 44.dp
private val STYLE_BORDER = 2.dp
private val STYLE_GLOW = 12.dp

private fun PresetId.descriptionRes(): Int = when (this) {
    PresetId.FIRST_RIDE -> R.string.ex_style_first_d
    PresetId.EASY -> R.string.ex_style_easy_d
    PresetId.CLASSIC -> R.string.ex_style_classic_d
    PresetId.INTERVALS -> R.string.ex_style_intervals_d
}

/** A style as a radio card: its mountain at the chosen length, its name and one line. */
@Composable
private fun StyleCard(name: String, description: String, segments: List<RideSegment>, selected: Boolean, onClick: () -> Unit) {
    val profile = remember(segments) { stylePreviewProfile(segments) }
    val accent = MaterialTheme.colorScheme.primary
    val glow = if (selected) STYLE_GLOW else 0.dp
    Card(
        onClick = onClick,
        modifier = Modifier
            .fillMaxWidth()
            .selectScale(selected)
            .shadow(glow, CardDefaults.shape, ambientColor = accent, spotColor = accent)
            .selectable(selected = selected, role = Role.RadioButton, onClick = onClick),
        border = BorderStroke(STYLE_BORDER, if (selected) accent else Color.Transparent),
    ) {
        Row(Modifier.padding(12.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            MountainProfile(profile, marker = null, segments = segments, modifier = Modifier.width(STYLE_MOUNTAIN * 2).height(STYLE_MOUNTAIN))
            Column(Modifier.weight(1f)) {
                Text(name, style = MaterialTheme.typography.titleSmall)
                Text(description, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

/** Genres of the setup chips: the core and Tidal search by the token (English), the chip shows the label. */
internal enum class Genre(val token: String, val label: Int) {
    ROCK("rock", R.string.ex_genre_rock),
    METAL("metal", R.string.ex_genre_metal),
    POP("pop", R.string.ex_genre_pop),
    DANCE("dance", R.string.ex_genre_dance),
    HIP_HOP("hip-hop", R.string.ex_genre_hip_hop),
    LATIN("latin", R.string.ex_genre_latin),
    FUNK("funk", R.string.ex_genre_funk),
    EIGHTIES("80s", R.string.ex_genre_80s),
    INDIE("indie", R.string.ex_genre_indie),
    PUNK("punk", R.string.ex_genre_punk);

    companion object {
        fun ofToken(token: String): Genre? = entries.firstOrNull { it.token == token }
    }
}

/** Step 1: how long, what kind of ride, which music. Everything else is in Settings → Exercise. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun RideSetup(form: RideForm, chat: RideChat) {
    val app = LocalContext.current.homefi
    val session = app.ride
    val settings by app.settings.exercise.collectAsStateWithLifecycle()
    val aiReady = remember(form.useAi) { app.ai.hasProvider() }
    val language = languageTag()
    val presets = remember(language) { exercisePresets(language) }

    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
        Section(stringResource(R.string.ex_duration)) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                FilledTonalIconButton(
                    onClick = { session.updateForm { it.copy(minutes = (it.minutes - MINUTES_STEP).coerceAtLeast(MIN_MINUTES), workout = null) } },
                    enabled = form.minutes > MIN_MINUTES,
                    modifier = Modifier.size(STEP_BUTTON),
                ) { Icon(Icons.Rounded.Remove, stringResource(R.string.ex_length_minus)) }
                Row(Modifier.weight(1f), horizontalArrangement = Arrangement.Center, verticalAlignment = Alignment.Bottom) {
                    Text("${form.minutes}", fontSize = BIG_MINUTES_SP.sp, fontWeight = FontWeight.ExtraBold)
                    Text(
                        stringResource(R.string.ex_minutes_unit),
                        style = MaterialTheme.typography.titleMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(start = 6.dp, bottom = 8.dp),
                    )
                }
                FilledTonalIconButton(
                    onClick = { session.updateForm { it.copy(minutes = (it.minutes + MINUTES_STEP).coerceAtMost(MAX_MINUTES), workout = null) } },
                    enabled = form.minutes < MAX_MINUTES,
                    modifier = Modifier.size(STEP_BUTTON),
                ) { Icon(Icons.Rounded.Add, stringResource(R.string.ex_length_plus)) }
            }
            // a clean slider: it snaps to the step by itself, no dots
            Slider(
                value = form.minutes.toFloat(),
                onValueChange = { v -> session.updateForm { it.copy(minutes = snapMinutes(v), workout = null) } },
                valueRange = MIN_MINUTES.toFloat()..MAX_MINUTES.toFloat(),
            )
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                QUICK_MINUTES.forEach { minutes ->
                    FilterChip(
                        selected = form.minutes == minutes,
                        onClick = { session.updateForm { it.copy(minutes = minutes, workout = null) } },
                        label = { Text(stringResource(R.string.ex_minutes_value, minutes), Modifier.padding(vertical = 8.dp), maxLines = 1, softWrap = false) },
                        modifier = Modifier.weight(1f).selectScale(form.minutes == minutes),
                    )
                }
            }
        }

        Section(stringResource(R.string.ex_style)) {
            presets.forEach { preset ->
                val workout = remember(preset.id, form.minutes, settings) {
                    runCatching { exercisePreset(preset.id, form.minutes.toUShort(), settings) }.getOrNull()
                }
                StyleCard(
                    name = preset.name,
                    description = stringResource(preset.id.descriptionRes()),
                    segments = workout?.segments.orEmpty(),
                    selected = form.style == preset.id && form.workout == null,
                    onClick = { session.updateForm { it.copy(style = preset.id, workout = null) } },
                )
            }
            if (form.workout != null) {
                Text(
                    stringResource(R.string.ex_workout_by_ai, form.workout.name),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }

        Section(stringResource(R.string.ex_music), stringResource(R.string.ex_music_hint)) {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Genre.entries.forEach { genre ->
                    FilterChip(
                        selected = genre.token in form.genres,
                        onClick = {
                            session.updateForm { f ->
                                f.copy(genres = if (genre.token in f.genres) f.genres - genre.token else f.genres + genre.token)
                            }
                        },
                        label = { Text(stringResource(genre.label)) },
                        leadingIcon = if (genre.token in form.genres) {
                            { Icon(Icons.Rounded.Check, null, Modifier.size(FilterChipDefaults.IconSize)) }
                        } else null,
                        modifier = Modifier.selectScale(genre.token in form.genres),
                    )
                }
                FilterChip(
                    selected = form.mix,
                    onClick = { session.updateForm { it.copy(mix = !it.mix) } },
                    label = { Text(stringResource(R.string.ex_genre_mix)) },
                )
            }
            OutlinedTextField(
                value = form.sentence,
                onValueChange = { v -> session.updateForm { it.copy(sentence = v.take(MAX_SENTENCE)) } },
                label = { Text(stringResource(R.string.ex_sentence)) },
                placeholder = { Text(stringResource(R.string.ex_sentence_hint)) },
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences, imeAction = ImeAction.Done),
                modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
            )
        }

        Section(stringResource(R.string.ex_ai)) {
            SwitchLine(
                stringResource(R.string.ex_ai_use),
                form.useAi && aiReady,
                enabled = aiReady,
            ) { v -> session.updateForm { it.copy(useAi = v) } }
            if (!aiReady) {
                Text(
                    exerciseNeedsAiMessage(language),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            } else if (form.useAi) {
                RideChatBox(chat, onStart = session::startChat, onSay = session::say)
            }
        }

        Section(stringResource(R.string.ex_tv)) {
            SwitchLine(stringResource(R.string.ex_tv_on), settings.tvEnabled) { v ->
                app.settings.setExercise(settings.copy(tvEnabled = v))
            }
            if (settings.tvEnabled) {
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    FilterChip(
                        selected = settings.tvAudio == TvAudio.PHONE,
                        onClick = { app.settings.setExercise(settings.copy(tvAudio = TvAudio.PHONE)) },
                        label = { Text(stringResource(R.string.ex_tv_audio_phone)) },
                    )
                    FilterChip(
                        selected = settings.tvAudio == TvAudio.TV,
                        onClick = { app.settings.setExercise(settings.copy(tvAudio = TvAudio.TV)) },
                        label = { Text(stringResource(R.string.ex_tv_audio_tv)) },
                    )
                }
            }
            TvPairingBlock()
        }

    }
}

/** The pinned "Build my ride": a big gradient button; the first time it shows the health notice. */
@Composable
fun BuildBar() {
    val app = LocalContext.current.homefi
    val session = app.ride
    val settings by app.settings.exercise.collectAsStateWithLifecycle()
    var notice by rememberSaveable { mutableStateOf(false) }
    val language = languageTag()
    GradientButton(
        text = stringResource(R.string.ex_build),
        onClick = { if (settings.healthNoticeSeen) session.build() else notice = true },
        icon = Icons.Rounded.AutoAwesome,
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp),
    )
    if (notice) {
        AlertDialog(
            onDismissRequest = { notice = false },
            title = { Text(stringResource(R.string.ex_health_title)) },
            text = { Text(exerciseHealthNotice(language)) },
            confirmButton = {
                TextButton(onClick = {
                    notice = false
                    app.settings.setExercise(settings.copy(healthNoticeSeen = true))
                    session.build()
                }) { Text(stringResource(R.string.ex_health_ok)) }
            },
            dismissButton = { TextButton(onClick = { notice = false }) { Text(stringResource(R.string.ex_cancel)) } },
        )
    }
}

private const val MAX_SENTENCE = 200

private fun snapMinutes(value: Float): Int =
    (Math.round(value / MINUTES_STEP) * MINUTES_STEP).coerceIn(MIN_MINUTES, MAX_MINUTES)

@Composable
private fun Section(title: String, trailing: String? = null, content: @Composable ColumnScope.() -> Unit) {
    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            // a short value sits beside the title; a longer line (a hint) goes under it
            if (trailing == null || trailing.length <= SHORT_TRAILING) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(title, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary, modifier = Modifier.weight(1f))
                    if (trailing != null) Text(trailing, style = MaterialTheme.typography.titleMedium)
                }
            } else {
                Text(title, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
                Text(trailing, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            content()
        }
    }
}

@Composable
private fun SwitchLine(label: String, checked: Boolean, enabled: Boolean = true, onChange: (Boolean) -> Unit) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Text(label, Modifier.weight(1f), style = MaterialTheme.typography.bodyLarge)
        Switch(checked = checked, onCheckedChange = onChange, enabled = enabled)
    }
}

/** The optional chat: the AI turns a sentence into a workout and a music wish; the form shows the result. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun RideChatBox(chat: RideChat, onStart: () -> Unit, onSay: (String) -> Unit) {
    var text by rememberSaveable { mutableStateOf("") }
    val send = {
        if (text.isNotBlank()) {
            onSay(text)
            text = ""
        }
    }
    if (!chat.started) {
        OutlinedButton(onClick = onStart, modifier = Modifier.fillMaxWidth()) {
            Icon(Icons.Rounded.AutoAwesome, null)
            Text(stringResource(R.string.ex_ai_chat), Modifier.padding(start = 8.dp))
        }
        return
    }
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        chat.lines.forEach { line ->
            Box(Modifier.fillMaxWidth(), contentAlignment = if (line.fromUser) Alignment.CenterEnd else Alignment.CenterStart) {
                Text(
                    line.text,
                    style = MaterialTheme.typography.bodyMedium,
                    color = if (line.fromUser) MaterialTheme.colorScheme.onPrimaryContainer else MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier
                        .background(
                            if (line.fromUser) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceVariant,
                            RoundedCornerShape(12.dp),
                        )
                        .padding(horizontal = 12.dp, vertical = 8.dp),
                )
            }
        }
        if (chat.thinking) LinearProgressIndicator(Modifier.fillMaxWidth())
        chat.error?.let { Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall) }
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            chat.replies.forEach { reply ->
                FilterChip(selected = false, onClick = { onSay(reply) }, label = { Text(reply) }, enabled = !chat.thinking)
            }
        }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedTextField(
                value = text,
                onValueChange = { text = it },
                placeholder = { Text(stringResource(R.string.ex_ai_say)) },
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences, imeAction = ImeAction.Send),
                keyboardActions = KeyboardActions(onSend = { send() }),
                modifier = Modifier.weight(1f),
                enabled = !chat.thinking,
            )
            FilledIconButton(onClick = send, enabled = !chat.thinking && text.isNotBlank()) {
                Icon(Icons.AutoMirrored.Rounded.Send, stringResource(R.string.ex_ai_send))
            }
        }
    }
}
