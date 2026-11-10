package com.perseusoft.homefi.tv

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Lock
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.AiProvider
import com.perseusoft.homefi.core.AiProviderInfo
import com.perseusoft.homefi.core.ModelInfo
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.settings.AiSettingsViewModel
import com.perseusoft.homefi.ui.settings.KeyStatus

/**
 * Settings, AI page: the provider, its key (typed with the TV keyboard, kept in the Keystore by the phone's
 * `KeyVault`), the model and the server. It drives the same `AiSettingsViewModel` as the phone's page, so
 * checking a key, saving it and listing models are the one implementation. The key is typed into a prompt
 * that only holds it until Done, then it goes to the vault; it is shown as dots and never logged.
 */
@Composable
fun TvAiSettings(firstRow: FocusRequester, back: FocusRequester, onRowFocused: () -> Unit) {
    val app = LocalContext.current.homefi
    val vm: AiSettingsViewModel = viewModel()
    val ui by vm.ui.collectAsStateWithLifecycle()
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val saved by app.ai.keys.saved.collectAsStateWithLifecycle()
    val look = LocalTvLook.current
    val ai = settings.ai
    val chosen = app.ai.provider(ai.provider)
    val hasKey = chosen != null && chosen.id in saved
    var pickProvider by remember { mutableStateOf(false) }
    var typingKey by remember { mutableStateOf(false) }
    var typingUrl by remember { mutableStateOf(false) }
    var removing by remember { mutableStateOf(false) }
    var showModels by remember { mutableStateOf(false) }
    val noneLabel = stringResource(R.string.ai_none)
    val canCall = chosen != null && (!chosen.needsKey || hasKey)

    // the model list arrived after OK on "Model": show it
    LaunchedEffect(ui.models) { if (ui.models == null) showModels = false }

    val statusLine = status(ui.status)
    val edge = Modifier.tvLeftTo(back)
    val rowFocus = { focused: Boolean -> if (focused) onRowFocused() }
    LazyColumn(
        Modifier.fillMaxSize(),
        contentPadding = PaddingValues(start = 12.dp, end = 14.dp, top = 12.dp, bottom = 24.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        row("provider") {
            AiLine(
                title = stringResource(R.string.ai_provider),
                value = chosen?.name ?: noneLabel,
                modifier = edge.focusRequester(firstRow),
                onClick = { pickProvider = true },
                onFocused = rowFocus,
            )
        }
        if (chosen != null) {
            chosen.hint.takeIf { it.isNotBlank() }?.let { hint -> row("hint") { Note(hint) } }
            if (chosen.customBaseUrl) row("url") {
                val url = ai.baseUrls[chosen.id].orEmpty()
                AiLine(
                    title = stringResource(R.string.ai_base_url),
                    value = url.ifBlank { chosen.defaultBaseUrl.ifBlank { stringResource(R.string.tv_set_value_default) } },
                    modifier = edge,
                    onClick = { typingUrl = true },
                    onFocused = rowFocus,
                )
            }
            if (chosen.needsKey) {
                row("key") {
                    AiLine(
                        title = stringResource(R.string.ai_key),
                        value = stringResource(if (hasKey) R.string.ai_key_saved else R.string.ai_key_missing),
                        modifier = edge,
                        onClick = { typingKey = true },
                        onFocused = rowFocus,
                        hint = stringResource(if (hasKey) R.string.ai_key_replace else R.string.tv_ai_key_type),
                    )
                }
                if (chosen.keyUrl.isNotBlank()) row("keyUrl") { Note(stringResource(R.string.tv_ai_get_key, chosen.keyUrl)) }
            }
            row("test") {
                AiLine(
                    title = stringResource(R.string.ai_test),
                    value = null,
                    modifier = edge,
                    enabled = canCall && ui.status != KeyStatus.Testing,
                    onClick = { vm.test(chosen, "") },
                    onFocused = rowFocus,
                )
            }
            statusLine?.let { (text, error) ->
                row("status") { Note(text, error = error) }
            }
            row("model") {
                val model = ai.models[chosen.id].orEmpty()
                val auto = (ui.defaultModel ?: chosen.defaultModel)?.let { stringResource(R.string.ai_model_auto_named, it) } ?: stringResource(R.string.ai_model_auto)
                AiLine(
                    title = stringResource(R.string.ai_model),
                    value = if (ui.loadingModels) stringResource(R.string.tv_ai_loading) else model.ifBlank { auto },
                    modifier = edge,
                    enabled = canCall && !ui.loadingModels,
                    onFocused = rowFocus,
                    onClick = {
                        showModels = true
                        if (ui.models == null) vm.loadModels(chosen, "")
                    },
                )
            }
            ui.modelsError?.let { error -> row("modelsError") { Note(error, error = true) } }
            if (hasKey) row("remove") {
                AiLine(title = stringResource(R.string.ai_remove), value = null, modifier = edge, onClick = { removing = true }, danger = true, onFocused = rowFocus)
            }
        }
        row("privacy") {
            Row(Modifier.padding(horizontal = 12.dp, vertical = 8.dp), verticalAlignment = Alignment.Top, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Icon(Icons.Rounded.Lock, null, tint = look.ink3, modifier = Modifier.size(16.dp).padding(top = 2.dp))
                Text(
                    stringResource(R.string.tv_ai_privacy, chosen?.name ?: stringResource(R.string.ai_privacy_provider)),
                    style = TvType.secondary,
                    color = look.ink3,
                )
            }
        }
    }

    if (pickProvider) {
        val providers = app.ai.providers
        val labels = listOf(noneLabel) + providers.map { it.name }
        TvOptionPicker(
            title = stringResource(R.string.ai_provider),
            options = labels,
            selected = (providers.indexOfFirst { it.id == ai.provider } + 1).coerceAtLeast(0),
            onPick = { i ->
                vm.selectProvider(providers.getOrNull(i - 1)?.id.orEmpty())
                pickProvider = false
            },
            onDismiss = { pickProvider = false },
        )
    }
    if (typingKey && chosen != null) {
        TvTextPrompt(
            title = stringResource(R.string.ai_key),
            initial = "",
            placeholder = stringResource(R.string.ai_key),
            doneLabel = stringResource(R.string.ai_save),
            secret = true,
            onDone = { typed ->
                typingKey = false
                if (typed.isNotBlank()) vm.save(chosen, typed)
            },
            onDismiss = { typingKey = false },
        )
    }
    if (typingUrl && chosen != null) {
        TvTextPrompt(
            title = stringResource(R.string.ai_base_url),
            initial = ai.baseUrls[chosen.id].orEmpty(),
            placeholder = chosen.defaultBaseUrl.ifBlank { stringResource(R.string.ai_base_url) },
            onDone = { typed ->
                typingUrl = false
                vm.setBaseUrl(chosen, typed)
            },
            onDismiss = { typingUrl = false },
        )
    }
    val models = ui.models
    if (showModels && chosen != null && models != null) {
        val sorted = remember(models) { models.sortedWith(compareBy({ !it.recommended }, { !it.economical }, { it.small })) }
        val auto = stringResource(R.string.ai_model_auto)
        val current = ai.models[chosen.id].orEmpty()
        TvOptionPicker(
            title = stringResource(R.string.ai_model),
            options = listOf(auto) + sorted.map { it.chipText() },
            selected = (sorted.indexOfFirst { it.id == current } + 1).coerceAtLeast(0),
            onPick = { i ->
                vm.setModel(chosen, sorted.getOrNull(i - 1)?.id.orEmpty())
                showModels = false
            },
            onDismiss = { showModels = false },
        )
    }
    if (removing && chosen != null) {
        TvConfirmDialog(
            title = stringResource(R.string.ai_remove_confirm, chosen.name),
            text = stringResource(R.string.ai_privacy_provider).replaceFirstChar { it.uppercase() } + ".",
            confirmLabel = stringResource(R.string.ai_remove),
            onConfirm = { vm.remove(chosen) },
            onDismiss = { removing = false },
        )
    }
}

private fun LazyListScope.row(key: String, content: @Composable () -> Unit) {
    item(key = key) { content() }
}

/** The result of the last check, save or model list, as words (and whether it is an error). */
@Composable
private fun status(status: KeyStatus): Pair<String, Boolean>? = when (status) {
    KeyStatus.Idle -> null
    KeyStatus.Testing -> stringResource(R.string.ai_testing) to false
    KeyStatus.Works -> stringResource(R.string.ai_test_ok) to false
    KeyStatus.Saved -> stringResource(R.string.ai_saved) to false
    KeyStatus.SaveFailed -> stringResource(R.string.ai_save_failed) to true
    is KeyStatus.Failed -> status.message to true
}

/** "Claude Sonnet · recommended": a model's name with its tags. */
@Composable
private fun ModelInfo.chipText(): String {
    val tags = buildList {
        if (recommended) add(stringResource(R.string.ai_model_recommended))
        if (economical) add(stringResource(R.string.ai_model_economical))
        if (small) add(stringResource(R.string.ai_model_small))
    }
    return (listOf(name.ifBlank { id }) + tags).joinToString(" · ")
}

/** A line of text that is not a control (a hint, a result): no focus. */
@Composable
private fun Note(text: String, error: Boolean = false) {
    val look = LocalTvLook.current
    Text(
        text,
        style = TvType.secondary.copy(fontSize = 13.sp),
        color = if (error) look.error else look.ink2,
        maxLines = 3,
        overflow = TextOverflow.Ellipsis,
        modifier = Modifier.padding(horizontal = 12.dp, vertical = 4.dp),
    )
}

/** A focusable line of the AI page; [value] is what the row holds, [danger] tints a destructive one. */
@Composable
private fun AiLine(
    title: String,
    value: String?,
    modifier: Modifier,
    onClick: () -> Unit,
    enabled: Boolean = true,
    hint: String? = null,
    danger: Boolean = false,
    onFocused: (Boolean) -> Unit = {},
) = TvSettingLine(title, hint, enabled, modifier, onClick, danger, onFocused) { focused -> if (value != null) ValueText(value, focused, arrows = false) }
