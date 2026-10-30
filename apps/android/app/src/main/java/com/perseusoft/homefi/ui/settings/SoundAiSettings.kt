package com.perseusoft.homefi.ui.settings

import com.perseusoft.homefi.ui.AiIcon
import android.app.Application
import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.layout.heightIn
import androidx.compose.ui.semantics.Role
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.automirrored.rounded.OpenInNew
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.Equalizer
import androidx.compose.material.icons.rounded.Lock
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
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
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.AiProvider
import com.perseusoft.homefi.core.AiProviderInfo
import com.perseusoft.homefi.core.AiProviderBadge
import com.perseusoft.homefi.ui.QualityBadge
import com.perseusoft.homefi.core.ModelInfo
import com.perseusoft.homefi.core.aiDefaultModel
import com.perseusoft.homefi.core.aiListModels
import com.perseusoft.homefi.core.aiTestKey
import com.perseusoft.homefi.core.eqProfileFor
import com.perseusoft.homefi.data.AiSettings
import com.perseusoft.homefi.data.userMessage
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.EqPanel
import com.perseusoft.homefi.player.EqTab
import com.perseusoft.homefi.core.LoudnessMode
import com.perseusoft.homefi.core.LoudnessSettings
import com.perseusoft.homefi.ui.eq.formatDb
import com.perseusoft.homefi.ui.sound.hintRes
import com.perseusoft.homefi.ui.sound.labelRes
import androidx.compose.material.icons.rounded.Layers
import com.perseusoft.homefi.ui.ConfirmDialog
import com.perseusoft.homefi.ui.SelectChip
import com.perseusoft.homefi.ui.haptics
import com.perseusoft.homefi.ui.theme.FilledTonalButton
import com.perseusoft.homefi.ui.theme.OutlinedButton
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

// ===================================================================== Sound

@Composable
internal fun soundSummary(): String {
    val eq = LocalContext.current.homefi.equalizer
    val settings by eq.settings.collectAsStateWithLifecycle()
    val output by eq.outputId.collectAsStateWithLifecycle()
    val taste by LocalContext.current.homefi.sound.taste.collectAsStateWithLifecycle()
    val active = taste.enabled && taste.rules.any { it.enabled }
    val rules = if (active) " · " + stringResource(R.string.sound_summary_rules, taste.rules.count { it.enabled }) else ""
    // off but with taste rules: not "bit-perfect" for the music they cover
    if (!settings.enabled) return if (active) stringResource(R.string.sound_summary_eq_off_rules, taste.rules.count { it.enabled }) else stringResource(R.string.eq_summary_off)
    val profile = remember(settings, output) { runCatching { eqProfileFor(settings, output) }.getOrNull() }
    val fallback = if (profile?.bands.isNullOrEmpty()) R.string.eq_flat else R.string.eq_custom
    return stringResource(R.string.eq_summary_on, profile?.name?.ifBlank { null } ?: stringResource(fallback)) + rules
}

/** Settings → Sound: the equalizer's switches and the ways into its screens. */
@Composable
internal fun SoundSettings() {
    val app = LocalContext.current.homefi
    val eq = app.equalizer
    val settings by eq.settings.collectAsStateWithLifecycle()
    val active by eq.active.collectAsStateWithLifecycle()
    val taste by app.sound.taste.collectAsStateWithLifecycle()
    val loudness by app.sound.loudness.collectAsStateWithLifecycle()
    val appSettings by app.settings.settings.collectAsStateWithLifecycle()
    val haptics = haptics()

    SettingsGroup(title = stringResource(R.string.eq_title)) {
        SwitchRow(
            stringResource(R.string.eq_on), settings.enabled,
            hint = stringResource(if (active) R.string.eq_not_bitperfect_note else R.string.eq_off_note),
        ) { eq.setEnabled(it) }
        RowDivider()
        SwitchRow(stringResource(R.string.eq_preamp_auto), settings.autoPreamp) { v -> eq.update { it.copy(autoPreamp = v) } }
        RowDivider()
        NavRow(stringResource(R.string.eq_open), soundSummary()) {
            haptics.press()
            eq.open(EqPanel.EQUALIZER)
        }
        RowDivider()
        NavRow(stringResource(R.string.eq_assistant), stringResource(R.string.eq_assistant_hint)) {
            haptics.press()
            eq.open(EqPanel.ASSISTANT)
        }
    }

    SettingsGroup(title = stringResource(R.string.sound_layers_on)) {
        SwitchRow(
            stringResource(R.string.sound_layers_toggle), taste.enabled,
            hint = stringResource(R.string.sound_layers_hint),
        ) { app.sound.setEnabled(it) }
        RowDivider()
        TasteIntensityRow()
        RowDivider()
        NavRow(stringResource(R.string.sound_open_rules), stringResource(R.string.sound_summary_rules, taste.rules.size)) {
            haptics.press()
            eq.open(EqPanel.EQUALIZER, EqTab.TASTE)
        }
    }

    SettingsGroup(title = stringResource(R.string.sound_loudness)) {
        LoudnessSettingsRows(loudness) { app.sound.setLoudness(it) }
    }

    SettingsGroup(title = stringResource(R.string.sound_how)) {
        SwitchRow(
            stringResource(R.string.sound_settings_react_button), appSettings.soundReactButton,
            hint = stringResource(R.string.sound_settings_react_button_hint),
        ) { v -> app.settings.update { it.copy(soundReactButton = v) } }
        RowDivider()
        SwitchRow(
            stringResource(R.string.sound_settings_layer_chip), appSettings.soundLayerChip,
            hint = stringResource(R.string.sound_settings_layer_chip_hint),
        ) { v -> app.settings.update { it.copy(soundLayerChip = v) } }
        RowDivider()
        SwitchRow(
            stringResource(R.string.sound_settings_suggestions), appSettings.soundSuggestions,
            hint = stringResource(R.string.sound_settings_suggestions_hint),
        ) { v -> app.settings.update { it.copy(soundSuggestions = v) } }
        RowDivider()
        SegmentedRow(
            stringResource(R.string.taste_editor_setting), TASTE_EDITOR_IDS, appSettings.tasteEditor,
            label = { stringResource(it.editorLabelRes()) },
            onSelect = { id -> app.settings.update { it.copy(tasteEditor = id) } },
            hint = stringResource(R.string.taste_editor_setting_hint),
        )
    }
}

private val TASTE_EDITOR_IDS = listOf("auto", "simple", "advanced")

private fun String.editorLabelRes(): Int = when (this) {
    "simple" -> R.string.taste_mode_simple
    "advanced" -> R.string.taste_mode_advanced
    else -> R.string.taste_editor_auto
}

/** Loudness mode, target and clipping guard as Settings rows (the EQ screen has its own layout). */
@Composable
private fun LoudnessSettingsRows(loudness: LoudnessSettings, onChange: (LoudnessSettings) -> Unit) {
    SegmentedRow(
        stringResource(R.string.sound_loudness), LOUDNESS_MODES, loudness.mode,
        label = { stringResource(it.labelRes()) },
        onSelect = { m -> onChange(loudness.copy(mode = m)) },
        hint = stringResource(loudness.mode.hintRes()),
    )
    Reveal(loudness.mode != LoudnessMode.OFF) {
        RowDivider()
        SliderRow(
            stringResource(R.string.sound_loudness_target) + " · " +
                stringResource(R.string.sound_loudness_lufs, formatDb(loudness.targetLufs).removePrefix("+")),
            loudness.targetLufs, LUFS_RANGE,
            onChange = { v -> onChange(loudness.copy(targetLufs = Math.round(v).toFloat())) },
        )
        RowDivider()
        SwitchRow(
            stringResource(R.string.sound_loudness_clipping), loudness.preventClipping,
            hint = stringResource(R.string.sound_loudness_needs_tags),
        ) { v -> onChange(loudness.copy(preventClipping = v)) }
    }
}

private val LOUDNESS_MODES = listOf(LoudnessMode.OFF, LoudnessMode.AUTO, LoudnessMode.TRACK, LoudnessMode.ALBUM)
private val LUFS_RANGE = -24f..-8f

/** A row that opens a screen: a chevron on the right. */
@Composable
private fun NavRow(title: String, subtitle: String, onClick: () -> Unit) {
    ActionRow(
        title, hint = subtitle, onClick = onClick,
        trailing = { Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, null) },
    )
}

// ===================================================================== AI

@Composable
internal fun aiSummary(ai: AiSettings): String {
    val setup = LocalContext.current.homefi.ai
    val saved by setup.keys.saved.collectAsStateWithLifecycle()
    val info = setup.provider(ai.provider) ?: return stringResource(R.string.ai_summary_none)
    val status = when {
        !info.needsKey -> stringResource(R.string.ai_key_not_needed)
        info.id in saved -> stringResource(R.string.ai_key_saved)
        else -> stringResource(R.string.ai_key_missing)
    }
    return "${info.name} · $status"
}

/** What a key check, a save or the model list did, per provider. */
sealed interface KeyStatus {
    data object Idle : KeyStatus
    data object Testing : KeyStatus
    data object Works : KeyStatus
    data object Saved : KeyStatus
    data object SaveFailed : KeyStatus
    data class Failed(val message: String) : KeyStatus
}

data class AiSettingsUi(
    val status: KeyStatus = KeyStatus.Idle,
    val models: List<ModelInfo>? = null,
    val loadingModels: Boolean = false,
    val defaultModel: String? = null,
    val modelsError: String? = null,
)

/**
 * Key checks, saving and model lists for Settings → AI. A typed key is used for the call
 * and then dropped; the saved one is read from the Keystore right before each call.
 */
class AiSettingsViewModel(app: Application) : AndroidViewModel(app) {
    private val home = app.homefi
    private val setup = home.ai
    private val _ui = MutableStateFlow(AiSettingsUi())
    val ui: StateFlow<AiSettingsUi> = _ui.asStateFlow()
    private var job: Job? = null

    private fun keyFor(info: AiProviderInfo, typed: String): String? =
        if (!info.needsKey) null else typed.trim().ifEmpty { setup.keys.get(info.id) }

    fun selectProvider(id: String) {
        job?.cancel()
        _ui.value = AiSettingsUi()
        home.settings.updateAi { it.copy(provider = id) }
    }

    fun setModel(info: AiProviderInfo, model: String) =
        home.settings.updateAi { it.copy(models = it.models + (info.id to model)) }

    fun setBaseUrl(info: AiProviderInfo, url: String) {
        home.settings.updateAi { it.copy(baseUrls = if (url.isBlank()) it.baseUrls - info.id else it.baseUrls + (info.id to url.trim())) }
        _ui.update { it.copy(models = null, status = KeyStatus.Idle) }
    }

    fun test(info: AiProviderInfo, typed: String) = call {
        _ui.update { it.copy(status = KeyStatus.Testing) }
        aiTestKey(setup.config(info), keyFor(info, typed))
        _ui.update { it.copy(status = KeyStatus.Works) }
    }

    fun save(info: AiProviderInfo, typed: String) {
        val ok = setup.keys.put(info.id, typed)
        _ui.update { it.copy(status = if (ok) KeyStatus.Saved else KeyStatus.SaveFailed, models = null) }
    }

    fun remove(info: AiProviderInfo) {
        setup.keys.remove(info.id)
        _ui.update { it.copy(status = KeyStatus.Idle, models = null) }
    }

    fun loadModels(info: AiProviderInfo, typed: String) {
        job?.cancel()
        job = viewModelScope.launch {
            _ui.update { it.copy(loadingModels = true, modelsError = null) }
            try {
                val key = keyFor(info, typed)
                val config = setup.config(info).copy(model = "")
                val models = aiListModels(config, key)
                val default = runCatching { aiDefaultModel(config, key) }.getOrNull()
                _ui.update { it.copy(loadingModels = false, models = models, defaultModel = default) }
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                _ui.update { it.copy(loadingModels = false, modelsError = e.userMessage()) }
            }
        }
    }

    private fun call(block: suspend () -> Unit) {
        job?.cancel()
        job = viewModelScope.launch {
            try {
                block()
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                _ui.update { it.copy(status = KeyStatus.Failed(e.userMessage())) }
            }
        }
    }
}

/** Settings → AI: provider, key (Keystore), model and server, with the privacy note. */
@Composable
internal fun AiSettingsSection(vm: AiSettingsViewModel = viewModel()) {
    val app = LocalContext.current.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val saved by app.ai.keys.saved.collectAsStateWithLifecycle()
    val haptics = haptics()
    val providers = app.ai.providers
    val chosen = app.ai.provider(settings.ai.provider)

    SettingsGroup(title = stringResource(R.string.ai_provider), description = stringResource(R.string.ai_used_for)) {
        Column(Modifier.selectableGroup()) {
            ProviderRow(stringResource(R.string.ai_none), null, selected = chosen == null) {
                haptics.tick()
                vm.selectProvider("")
            }
            providers.forEach { info ->
                key(info.id) {
                    RowDivider()
                    val status = when {
                        !info.needsKey -> stringResource(R.string.ai_key_not_needed)
                        info.id in saved -> stringResource(R.string.ai_key_saved)
                        else -> stringResource(R.string.ai_key_missing)
                    }
                    ProviderRow(info.name, status, selected = chosen?.id == info.id, badge = info.badge, hint = info.hint) {
                        haptics.tick()
                        vm.selectProvider(info.id)
                    }
                }
            }
        }
    }
    Reveal(chosen != null) {
        if (chosen != null) key(chosen.id) { ProviderSetup(chosen, settings.ai, chosen.id in saved, vm) }
    }
    SettingsGroup {
        Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp), verticalAlignment = Alignment.Top) {
            Icon(Icons.Rounded.Lock, null, Modifier.size(18.dp).padding(top = 2.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.width(8.dp))
            Text(
                stringResource(R.string.ai_privacy, chosen?.name ?: stringResource(R.string.ai_privacy_provider)),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ProviderRow(
    name: String,
    status: String?,
    selected: Boolean,
    badge: AiProviderBadge? = null,
    hint: String = "",
    onClick: () -> Unit,
) {
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .selectable(selected = selected, role = Role.RadioButton, onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Column(Modifier.weight(1f)) {
            // the research's pick, the cheapest, one key for all, the private local one
            FlowRow(verticalArrangement = Arrangement.Center, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(name, style = MaterialTheme.typography.bodyLarge)
                badge?.let { QualityBadge(stringResource(it.labelRes()), Modifier.align(Alignment.CenterVertically)) }
            }
            if (hint.isNotBlank()) Text(hint, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            status?.let { Text(it, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.primary) }
        }
        RadioButton(selected = selected, onClick = null)
    }
}

private fun AiProviderBadge.labelRes(): Int = when (this) {
    AiProviderBadge.RECOMMENDED -> R.string.ai_badge_recommended
    AiProviderBadge.CHEAPEST -> R.string.ai_badge_cheapest
    AiProviderBadge.ONE_KEY_FOR_ALL -> R.string.ai_badge_one_key
    AiProviderBadge.PRIVATE_LOCAL -> R.string.ai_badge_private_local
}

/** "Claude Sonnet · Recommended", "Llama 8B · small". */
@Composable
private fun ModelInfo.chipText(): String {
    val tags = buildList {
        if (recommended) add(stringResource(R.string.ai_model_recommended))
        if (economical) add(stringResource(R.string.ai_model_economical))
        if (small) add(stringResource(R.string.ai_model_small))
    }
    return (listOf(name.ifBlank { id }) + tags).joinToString(" · ")
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ProviderSetup(info: AiProviderInfo, ai: AiSettings, hasKey: Boolean, vm: AiSettingsViewModel) {
    val context = LocalContext.current
    val ui by vm.ui.collectAsStateWithLifecycle()
    val haptics = haptics()
    // never saved in the instance state: a typed key only lives while the page is open
    var typed by remember { mutableStateOf("") }
    var removing by rememberSaveable { mutableStateOf(false) }

    SettingsGroup(title = stringResource(R.string.settings_ai_connection)) {
        if (info.keyUrl.isNotBlank()) {
            ActionRow(
                stringResource(R.string.ai_get_key),
                onClick = {
                    try {
                        context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(info.keyUrl)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
                    } catch (_: ActivityNotFoundException) {
                    }
                },
                trailing = { Icon(Icons.AutoMirrored.Rounded.OpenInNew, null, Modifier.size(18.dp)) },
            )
        }

        if (info.customBaseUrl) {
            var url by remember(info.id) { mutableStateOf(ai.baseUrls[info.id].orEmpty()) }
            val required = info.provider is AiProvider.OpenAiCompatible
            FieldRow { m ->
                OutlinedTextField(
                    value = url,
                    onValueChange = {
                        url = it
                        vm.setBaseUrl(info, it)
                    },
                    label = { Text(stringResource(R.string.ai_base_url)) },
                    placeholder = { if (info.defaultBaseUrl.isNotBlank()) Text(info.defaultBaseUrl) },
                    supportingText = {
                        Text(
                            if (required) stringResource(R.string.ai_base_url_required)
                            else stringResource(R.string.ai_base_url_hint, info.defaultBaseUrl),
                        )
                    },
                    isError = required && url.isBlank(),
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
                    modifier = m,
                )
            }
        }

        if (info.needsKey) {
            FieldRow { m ->
                OutlinedTextField(
                    value = typed,
                    onValueChange = { typed = it.trim() },
                    label = { Text(stringResource(R.string.ai_key)) },
                    supportingText = { Text(stringResource(if (hasKey) R.string.ai_key_replace else R.string.ai_key_missing)) },
                    leadingIcon = { if (hasKey) Icon(Icons.Rounded.CheckCircle, stringResource(R.string.ai_key_saved), tint = MaterialTheme.colorScheme.primary) },
                    visualTransformation = PasswordVisualTransformation(),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, autoCorrectEnabled = false),
                    singleLine = true,
                    modifier = m,
                )
            }
        }

        FlowRow(Modifier.padding(horizontal = 16.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            val canCall = !info.needsKey || hasKey || typed.isNotBlank()
            OutlinedButton(onClick = { haptics.tick(); vm.test(info, typed) }, enabled = canCall && ui.status != KeyStatus.Testing) {
                Text(stringResource(R.string.ai_test))
            }
            if (info.needsKey) {
                FilledTonalButton(
                    onClick = {
                        haptics.confirm()
                        vm.save(info, typed)
                        typed = ""
                    },
                    enabled = typed.isNotBlank(),
                ) { Text(stringResource(R.string.ai_save)) }
                if (hasKey) {
                    TextButton(onClick = { removing = true }) { Text(stringResource(R.string.ai_remove), color = MaterialTheme.colorScheme.error) }
                }
            }
        }
        val status = when (val s = ui.status) {
            KeyStatus.Idle -> null
            KeyStatus.Testing -> stringResource(R.string.ai_testing) to false
            KeyStatus.Works -> stringResource(R.string.ai_test_ok) to false
            KeyStatus.Saved -> stringResource(R.string.ai_saved) to false
            KeyStatus.SaveFailed -> stringResource(R.string.ai_save_failed) to true
            is KeyStatus.Failed -> s.message to true
        }
        status?.let { (text, isError) -> SettingsNote(text, error = isError) }
    }

    SettingsGroup(title = stringResource(R.string.settings_ai_model)) {
        val model = ai.models[info.id].orEmpty()
        val autoName = ui.defaultModel ?: info.defaultModel
        val autoLabel = autoName?.let { stringResource(R.string.ai_model_auto_named, it) } ?: stringResource(R.string.ai_model_auto)
        StackedSetting(stringResource(R.string.ai_model)) {
            Text(
                if (model.isBlank()) autoLabel else model,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            val models = ui.models
            if (models == null) {
                TextButton(
                    onClick = { haptics.tick(); vm.loadModels(info, typed) },
                    enabled = !ui.loadingModels && (!info.needsKey || hasKey || typed.isNotBlank()),
                ) {
                    if (ui.loadingModels) {
                        CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp)
                        Spacer(Modifier.width(8.dp))
                    }
                    Text(stringResource(R.string.ai_models_load))
                }
            } else {
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    SelectChip(
                        selected = model.isBlank(),
                        onClick = { vm.setModel(info, "") },
                        label = { Text(autoLabel) },
                    )
                    // the recommended ones first, small ones last
                    models.sortedWith(compareBy({ !it.recommended }, { !it.economical }, { it.small })).forEach { m ->
                        key(m.id) {
                            SelectChip(
                                selected = model == m.id,
                                onClick = { vm.setModel(info, m.id) },
                                label = { Text(m.chipText()) },
                            )
                        }
                    }
                }
                // a small model picked: cheaper, but it gets more wrong
                if (models.firstOrNull { it.id == model }?.small == true) {
                    Text(stringResource(R.string.ai_model_small_warning), color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                }
            }
            ui.modelsError?.let { Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall) }
        }
    }

    if (removing) {
        ConfirmDialog(
            title = stringResource(R.string.ai_remove_confirm, info.name),
            confirm = stringResource(R.string.ai_remove),
            onConfirm = { vm.remove(info) },
            onDismiss = { removing = false },
        )
    }
}
