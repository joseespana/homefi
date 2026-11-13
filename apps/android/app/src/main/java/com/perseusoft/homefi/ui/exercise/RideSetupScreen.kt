package com.perseusoft.homefi.ui.exercise

import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
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
import androidx.compose.material.icons.rounded.FavoriteBorder
import com.perseusoft.homefi.ui.DialogAction
import com.perseusoft.homefi.ui.HomefiDialog
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
import com.perseusoft.homefi.core.Popularity
import com.perseusoft.homefi.core.PresetId
import com.perseusoft.homefi.core.RideTaste
import com.perseusoft.homefi.core.exerciseDecades
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.ExpandLess
import androidx.compose.material.icons.rounded.ExpandMore
import androidx.compose.material3.AssistChip
import androidx.compose.material3.InputChip
import androidx.compose.runtime.LaunchedEffect
import com.perseusoft.homefi.core.RideSegment
import com.perseusoft.homefi.core.exercisePreset
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.CardDefaults
import androidx.compose.ui.draw.shadow
import androidx.compose.runtime.Composable
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
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
    val suggestions by session.suggestions.collectAsStateWithLifecycle()
    LaunchedEffect(form.genres) { session.loadSuggestions(form.genres) }

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
                                f.copy(genres = toggleGenre(f.genres, genre.token))
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
            TasteBlock(settings.taste, suggestions, session::updateTaste)
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
        HomefiDialog(
            title = stringResource(R.string.ex_health_title),
            text = exerciseHealthNotice(language),
            icon = Icons.Rounded.FavoriteBorder,
            primary = DialogAction(stringResource(R.string.ex_health_ok)) {
                notice = false
                app.settings.setExercise(settings.copy(healthNoticeSeen = true))
                session.build()
            },
            dismiss = stringResource(R.string.ex_cancel),
            onDismiss = { notice = false },
        )
    }
}

private const val MAX_SENTENCE = 200

/**
 * The taste questions under the genres, compact with smart defaults (big hits, any era, no artists): how famous,
 * which era, and folded away the artists liked (with the library's suggestions) and the ones never to play.
 * Every answer is a setting, saved as it changes, so the next ride starts from it.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun TasteBlock(taste: RideTaste, suggestions: List<String>, onTaste: ((RideTaste) -> RideTaste) -> Unit) {
    val decades = remember { exerciseDecades() }
    var open by rememberSaveable { mutableStateOf(false) }
    val label = MaterialTheme.typography.labelLarge
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Text(stringResource(R.string.ex_taste_popularity), style = label, color = muted, modifier = Modifier.padding(top = 8.dp))
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Popularity.entries.forEach { p ->
            FilterChip(
                selected = taste.popularity == p,
                onClick = { onTaste { it.copy(popularity = p) } },
                label = { Text(stringResource(p.labelRes())) },
                modifier = Modifier.selectScale(taste.popularity == p),
            )
        }
    }
    Text(stringResource(R.string.ex_taste_era), style = label, color = muted)
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        FilterChip(
            selected = taste.eras.isEmpty(),
            onClick = { onTaste { it.copy(eras = emptyList()) } },
            label = { Text(stringResource(R.string.ex_era_any)) },
        )
        decades.forEach { decade ->
            FilterChip(
                selected = decade in taste.eras,
                onClick = { onTaste { t -> t.copy(eras = toggleDecade(t.eras, decade)) } },
                label = { Text(decadeLabel(decade)) },
            )
        }
    }
    val count = taste.artists.size + taste.avoid.size
    TextButton(onClick = { open = !open }) {
        Icon(if (open) Icons.Rounded.ExpandLess else Icons.Rounded.ExpandMore, null)
        Text(
            if (count > 0) stringResource(R.string.ex_taste_artists_count, count) else stringResource(R.string.ex_taste_artists),
            Modifier.padding(start = 4.dp),
        )
    }
    if (!open) return
    Text(stringResource(R.string.ex_taste_like), style = label, color = muted)
    val offered = suggestions.filter { s -> taste.artists.none { it.equals(s, ignoreCase = true) } }
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        taste.artists.forEach { artist -> NameChip(artist) { onTaste { it.copy(artists = it.artists - artist) } } }
        offered.forEach { artist ->
            val add = stringResource(R.string.ex_taste_add_suggestion, artist)
            AssistChip(
                onClick = { onTaste { it.copy(artists = it.artists + artist) } },
                label = { Text(artist) },
                leadingIcon = { Icon(Icons.Rounded.Add, add, Modifier.size(FilterChipDefaults.IconSize)) },
            )
        }
    }
    NameField(stringResource(R.string.ex_taste_like_add)) { name -> onTaste { it.copy(artists = it.artists + name) } }
    Text(stringResource(R.string.ex_taste_avoid), style = label, color = muted)
    if (taste.avoid.isNotEmpty()) {
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            taste.avoid.forEach { artist -> NameChip(artist) { onTaste { it.copy(avoid = it.avoid - artist) } } }
        }
    }
    NameField(stringResource(R.string.ex_taste_avoid_add)) { name -> onTaste { it.copy(avoid = it.avoid + name) } }
}

/** A chosen artist: tapping removes it. */
@Composable
private fun NameChip(name: String, onRemove: () -> Unit) {
    InputChip(
        selected = true,
        onClick = onRemove,
        label = { Text(name) },
        trailingIcon = { Icon(Icons.Rounded.Close, stringResource(R.string.ex_taste_remove, name), Modifier.size(FilterChipDefaults.IconSize)) },
    )
}

/** A one-line field that hands its text over on "Done" (the core trims, de-duplicates and caps the list). */
@Composable
private fun NameField(label: String, onAdd: (String) -> Unit) {
    var text by rememberSaveable { mutableStateOf("") }
    OutlinedTextField(
        value = text,
        onValueChange = { text = it },
        label = { Text(label) },
        singleLine = true,
        keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Words, imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions(onDone = {
            if (text.isNotBlank()) onAdd(text.trim())
            text = ""
        }),
        modifier = Modifier.fillMaxWidth(),
    )
}

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

/** Where the ride is watched, chosen right before it starts (next to "Start ride"): a TV or not, and where the music plays. */
@Composable
fun RideTvChoice() {
    val app = LocalContext.current.homefi
    val settings by app.settings.exercise.collectAsStateWithLifecycle()
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
