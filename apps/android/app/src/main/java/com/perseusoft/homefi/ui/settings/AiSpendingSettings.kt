package com.perseusoft.homefi.ui.settings

import android.app.Application
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.AiAskMode
import com.perseusoft.homefi.core.AiCostKind
import com.perseusoft.homefi.core.AiSpendGuard
import com.perseusoft.homefi.core.AiSpendItem
import com.perseusoft.homefi.core.AiSpendSummary
import com.perseusoft.homefi.core.aiBalance
import com.perseusoft.homefi.core.aiSpendingClear
import com.perseusoft.homefi.core.aiSpendingSetGuard
import com.perseusoft.homefi.core.aiSpendingSummary
import com.perseusoft.homefi.core.aiUsdText
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.ButtonSpinner
import com.perseusoft.homefi.ui.DialogAction
import com.perseusoft.homefi.ui.HomefiDialog
import com.perseusoft.homefi.data.userMessage
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.text.DateFormat
import java.util.Date
import java.util.Locale

/** "Ask only above": the amounts offered (USD), the same on phone and TV. */
internal val ASK_ABOVE_PRESETS_USD = listOf(0.01, 0.05, 0.10, 0.25, 0.50, 1.0)
/** Monthly limits offered (USD). */
internal val LIMIT_PRESETS_USD = listOf(1.0, 2.0, 5.0, 10.0, 20.0, 50.0)
/** What the limit starts at when it is turned on (core `DEFAULT_MONTHLY_LIMIT_USD`). */
internal const val DEFAULT_LIMIT_USD = 5.0
/** History rows shown at first, and added by "Show more". */
internal const val HISTORY_PAGE = 20
/** The provider whose balance can be asked for. */
internal const val OPENROUTER_ID = "openrouter"

/** What's left on the AI account, as last asked. */
sealed interface BalanceState {
    data object Idle : BalanceState
    data object Loading : BalanceState
    data class Known(val text: String) : BalanceState
    data object NotSaid : BalanceState
    data class Failed(val message: String) : BalanceState
}

data class AiSpendingUi(
    val summary: AiSpendSummary? = null,
    val balance: BalanceState = BalanceState.Idle,
    val shown: Int = HISTORY_PAGE,
    val clearing: Boolean = false,
)

/** The AI spending page of phone and TV: the core's ledger, read and changed off the main thread. */
class AiSpendingViewModel(app: Application) : AndroidViewModel(app) {
    private val home = app.homefi
    private val _ui = MutableStateFlow(AiSpendingUi())
    val ui: StateFlow<AiSpendingUi> = _ui.asStateFlow()
    private var balanceJob: Job? = null

    init {
        refresh()
    }

    private fun language() = Locale.getDefault().toLanguageTag()

    fun refresh() {
        viewModelScope.launch {
            val summary = withContext(Dispatchers.IO) { runCatching { aiSpendingSummary(language()) }.getOrNull() }
            if (summary != null) _ui.update { it.copy(summary = summary) }
        }
    }

    fun setGuard(guard: AiSpendGuard) {
        viewModelScope.launch {
            withContext(Dispatchers.IO) { runCatching { aiSpendingSetGuard(guard) } }
            refresh()
        }
    }

    fun showMore() = _ui.update { it.copy(shown = it.shown + HISTORY_PAGE) }

    fun clear() {
        if (_ui.value.clearing) return
        _ui.update { it.copy(clearing = true) }
        viewModelScope.launch {
            withContext(Dispatchers.IO) { runCatching { aiSpendingClear() } }
            _ui.update { it.copy(clearing = false, shown = HISTORY_PAGE) }
            refresh()
        }
    }

    /** Asks the provider what's left (read-only, spends nothing). */
    fun checkBalance() {
        if (_ui.value.balance == BalanceState.Loading) return
        _ui.update { it.copy(balance = BalanceState.Loading) }
        balanceJob = viewModelScope.launch {
            val state = try {
                val ready = withContext(Dispatchers.IO) { home.ai.ready() }
                val left = ready?.let { aiBalance(it.config, it.key) }
                if (left == null) BalanceState.NotSaid else BalanceState.Known(aiUsdText(left))
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                BalanceState.Failed(e.userMessage())
            }
            _ui.update { it.copy(balance = state) }
        }
    }
}

/** "3 AI actions · 1 without a known price, not counted". */
@Composable
internal fun monthHint(summary: AiSpendSummary): String {
    val actions = pluralStringResource(R.plurals.spend_month_actions, summary.monthActions.toInt(), summary.monthActions.toInt())
    return if (summary.monthUnknown > 0u) {
        actions + " · " + pluralStringResource(R.plurals.spend_month_unknown, summary.monthUnknown.toInt(), summary.monthUnknown.toInt())
    } else {
        actions
    }
}

@Composable
internal fun AiAskMode.label(): String = stringResource(
    when (this) {
        AiAskMode.NEVER -> R.string.spend_ask_never
        AiAskMode.ALWAYS -> R.string.spend_ask_always
        AiAskMode.ABOVE -> R.string.spend_ask_above
    },
)

@Composable
internal fun AiAskMode.hint(): String = stringResource(
    when (this) {
        AiAskMode.NEVER -> R.string.spend_ask_never_hint
        AiAskMode.ALWAYS -> R.string.spend_ask_always_hint
        AiAskMode.ABOVE -> R.string.spend_ask_above_hint
    },
)

/** "Nov 16, 9:41 PM · model · 3 calls · 12.3k tokens". */
internal fun AiSpendItem.line(): String =
    DateFormat.getDateTimeInstance(DateFormat.MEDIUM, DateFormat.SHORT).format(Date(atMs)) + " · " + detail

/** The balance row's words. */
@Composable
internal fun BalanceState.hint(): String = when (this) {
    BalanceState.Idle -> stringResource(R.string.spend_balance_hint)
    BalanceState.Loading -> stringResource(R.string.spend_balance_checking)
    is BalanceState.Known -> stringResource(R.string.spend_balance_left, text)
    BalanceState.NotSaid -> stringResource(R.string.spend_balance_unknown)
    is BalanceState.Failed -> message
}

/** Settings → AI → spending: this month, per feature, the guard and the history. */
@Composable
internal fun AiSpendingSection(vm: AiSpendingViewModel = viewModel()) {
    val app = LocalContext.current.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val ui by vm.ui.collectAsStateWithLifecycle()
    val summary = ui.summary
    val guard = summary?.guard ?: AiSpendGuard(AiAskMode.NEVER, ASK_ABOVE_PRESETS_USD[2], null)
    var confirmClear by remember { mutableStateOf(false) }
    // the page may come back after AI actions: show the latest totals
    LaunchedEffect(Unit) { vm.refresh() }

    SettingsGroup(title = stringResource(R.string.spend_title), description = stringResource(R.string.spend_intro)) {
        SettingRow(
            label = stringResource(R.string.spend_month),
            hint = summary?.let { monthHint(it) },
            trailing = { Text(summary?.monthText ?: "—", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold) },
        )
        if (settings.ai.provider == OPENROUTER_ID) {
            RowDivider()
            val loading = ui.balance == BalanceState.Loading
            ActionRow(label = stringResource(R.string.spend_balance), hint = ui.balance.hint()) {
                FilledTonalButton(onClick = vm::checkBalance, enabled = !loading) {
                    if (loading) ButtonSpinner() else Text(stringResource(R.string.spend_balance_check))
                }
            }
        }
        val features = summary?.features.orEmpty()
        if (summary != null && features.isEmpty()) {
            RowDivider()
            SettingRow(label = stringResource(R.string.spend_nothing), hint = stringResource(R.string.spend_nothing_hint))
        }
        features.forEach { f ->
            RowDivider()
            SettingRow(label = f.label, hint = f.text)
        }
    }

    SettingsGroup(title = stringResource(R.string.spend_guard_title), description = stringResource(R.string.spend_guard_intro)) {
        SegmentedRow(
            title = stringResource(R.string.spend_ask),
            options = listOf(AiAskMode.NEVER, AiAskMode.ALWAYS, AiAskMode.ABOVE),
            selected = guard.ask,
            label = { it.label() },
            onSelect = { vm.setGuard(guard.copy(ask = it)) },
            hint = guard.ask.hint(),
        )
        if (guard.ask == AiAskMode.ABOVE) {
            SegmentedRow(
                title = stringResource(R.string.spend_ask_above_amount, aiUsdText(guard.askAboveUsd)),
                options = ASK_ABOVE_PRESETS_USD,
                selected = guard.askAboveUsd.takeIf { it in ASK_ABOVE_PRESETS_USD },
                label = { aiUsdText(it) },
                onSelect = { vm.setGuard(guard.copy(askAboveUsd = it)) },
            )
        }
        RowDivider()
        val limit = guard.monthlyLimitUsd
        SwitchRow(
            label = stringResource(R.string.spend_limit),
            checked = limit != null,
            hint = stringResource(R.string.spend_limit_hint),
        ) { on -> vm.setGuard(guard.copy(monthlyLimitUsd = if (on) DEFAULT_LIMIT_USD else null)) }
        if (limit != null) {
            SegmentedRow(
                title = stringResource(R.string.spend_limit_amount, aiUsdText(limit)),
                options = LIMIT_PRESETS_USD,
                selected = limit.takeIf { it in LIMIT_PRESETS_USD },
                label = { aiUsdText(it) },
                onSelect = { vm.setGuard(guard.copy(monthlyLimitUsd = it)) },
            )
        }
    }

    SettingsGroup(title = stringResource(R.string.spend_history)) {
        val history = summary?.history.orEmpty()
        if (history.isEmpty()) {
            SettingRow(label = stringResource(R.string.spend_history_empty))
        } else {
            history.take(ui.shown).forEachIndexed { i, item ->
                if (i > 0) RowDivider()
                HistoryRow(item)
            }
            if (history.size > ui.shown) {
                RowDivider()
                ActionRow(label = stringResource(R.string.spend_more, history.size - ui.shown), onClick = vm::showMore)
            }
            RowDivider()
            ActionRow(label = stringResource(R.string.spend_clear), hint = stringResource(R.string.spend_clear_hint)) {
                TextButton(onClick = { confirmClear = true }, enabled = !ui.clearing) {
                    if (ui.clearing) {
                        ButtonSpinner()
                    } else {
                        Text(stringResource(R.string.spend_clear), color = MaterialTheme.colorScheme.error)
                    }
                }
            }
        }
    }

    if (confirmClear) {
        HomefiDialog(
            title = stringResource(R.string.spend_clear_title),
            text = stringResource(R.string.spend_clear_text),
            onDismiss = { confirmClear = false },
            primary = DialogAction(stringResource(R.string.spend_clear), danger = true) {
                confirmClear = false
                vm.clear()
            },
            dismiss = stringResource(R.string.cancel),
        )
    }
}

@Composable
private fun HistoryRow(item: AiSpendItem) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 56.dp).padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Column(Modifier.weight(1f)) {
            Text(item.label, style = MaterialTheme.typography.bodyLarge)
            Text(
                item.line(),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }
        val unknown = item.costKind == AiCostKind.UNKNOWN
        Text(
            item.costText,
            style = MaterialTheme.typography.bodyMedium,
            fontWeight = if (unknown) FontWeight.Normal else FontWeight.SemiBold,
            color = if (unknown) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onSurface,
        )
    }
}
