package com.perseusoft.homefi.tv

import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.AiAskMode
import com.perseusoft.homefi.core.AiSpendGuard
import com.perseusoft.homefi.core.aiUsdText
import com.perseusoft.homefi.ui.settings.ASK_ABOVE_PRESETS_USD
import com.perseusoft.homefi.ui.settings.AiSpendingUi
import com.perseusoft.homefi.ui.settings.AiSpendingViewModel
import com.perseusoft.homefi.ui.settings.BalanceState
import com.perseusoft.homefi.ui.settings.LIMIT_PRESETS_USD
import com.perseusoft.homefi.ui.settings.OPENROUTER_ID
import com.perseusoft.homefi.ui.settings.hint
import com.perseusoft.homefi.ui.settings.label
import com.perseusoft.homefi.ui.settings.line
import com.perseusoft.homefi.ui.settings.monthHint

/** History lines the TV lists (the phone and the desktop show the whole history). */
private const val TV_HISTORY_ROWS = 10

/** The spending picker open on the TV's AI page. */
internal enum class SpendPick { ASK, ABOVE, LIMIT, CLEAR }

private val ASK_MODES = listOf(AiAskMode.NEVER, AiAskMode.ALWAYS, AiAskMode.ABOVE)

private fun AiSpendingUi.guard(): AiSpendGuard = summary?.guard ?: AiSpendGuard(AiAskMode.NEVER, ASK_ABOVE_PRESETS_USD[2], null)

/**
 * The AI page's spending rows on the TV, the same as the phone's section (one [AiSpendingViewModel]): this
 * month, the balance on OpenRouter, the cost per kind of action, "ask before spending", the monthly limit and
 * the latest history. Every line takes focus, so the remote scrolls through all of it.
 */
internal fun LazyListScope.spendingRows(
    vm: AiSpendingViewModel,
    ui: AiSpendingUi,
    provider: String,
    edge: Modifier,
    rowFocus: (Boolean) -> Unit,
    open: (SpendPick) -> Unit,
) {
    val summary = ui.summary
    val guard = ui.guard()
    row("spendTitle") { Note(stringResource(R.string.spend_intro)) }
    row("spendMonth") {
        AiLine(
            title = stringResource(R.string.spend_month),
            value = summary?.monthText ?: "—",
            hint = summary?.let { monthHint(it) },
            modifier = edge,
            onClick = vm::refresh,
            onFocused = rowFocus,
        )
    }
    if (provider == OPENROUTER_ID) row("spendBalance") {
        val known = ui.balance as? BalanceState.Known
        AiLine(
            title = stringResource(R.string.spend_balance),
            value = known?.text ?: stringResource(R.string.spend_balance_check),
            hint = ui.balance.hint(),
            modifier = edge,
            busy = ui.balance == BalanceState.Loading,
            onClick = vm::checkBalance,
            onFocused = rowFocus,
        )
    }
    summary?.features.orEmpty().forEach { f ->
        row("spendFeature-${f.feature}") {
            AiLine(title = f.label, value = null, hint = f.text, modifier = edge, onClick = {}, onFocused = rowFocus)
        }
    }
    row("spendAsk") {
        AiLine(
            title = stringResource(R.string.spend_ask),
            value = guard.ask.label(),
            hint = guard.ask.hint(),
            modifier = edge,
            onClick = { open(SpendPick.ASK) },
            onFocused = rowFocus,
        )
    }
    if (guard.ask == AiAskMode.ABOVE) row("spendAbove") {
        AiLine(
            title = stringResource(R.string.spend_ask_above_amount, aiUsdText(guard.askAboveUsd)),
            value = null,
            modifier = edge,
            onClick = { open(SpendPick.ABOVE) },
            onFocused = rowFocus,
        )
    }
    row("spendLimit") {
        AiLine(
            title = stringResource(R.string.spend_limit),
            value = guard.monthlyLimitUsd?.let { aiUsdText(it) } ?: stringResource(R.string.spend_limit_off),
            hint = stringResource(R.string.spend_limit_hint),
            modifier = edge,
            onClick = { open(SpendPick.LIMIT) },
            onFocused = rowFocus,
        )
    }
    val history = summary?.history.orEmpty()
    row("spendHistory") {
        Note(stringResource(if (history.isEmpty()) R.string.spend_history_empty else R.string.spend_history))
    }
    history.take(TV_HISTORY_ROWS).forEach { item ->
        row("spendItem-${item.id}") {
            AiLine(title = item.label, value = item.costText, hint = item.line(), modifier = edge, onClick = {}, onFocused = rowFocus)
        }
    }
    if (history.isNotEmpty()) row("spendClear") {
        AiLine(
            title = stringResource(R.string.spend_clear),
            value = null,
            hint = stringResource(R.string.spend_clear_hint),
            modifier = edge,
            busy = ui.clearing,
            danger = true,
            onClick = { open(SpendPick.CLEAR) },
            onFocused = rowFocus,
        )
    }
}

/** The picker or question behind a spending row. */
@Composable
internal fun TvSpendingDialog(pick: SpendPick, vm: AiSpendingViewModel, ui: AiSpendingUi, close: () -> Unit) {
    val guard = ui.guard()
    when (pick) {
        SpendPick.ASK -> TvOptionPicker(
            title = stringResource(R.string.spend_ask),
            options = ASK_MODES.map { it.label() },
            selected = ASK_MODES.indexOf(guard.ask).coerceAtLeast(0),
            onPick = { i ->
                vm.setGuard(guard.copy(ask = ASK_MODES[i]))
                close()
            },
            onDismiss = close,
        )
        SpendPick.ABOVE -> TvOptionPicker(
            title = stringResource(R.string.spend_ask_above_amount, aiUsdText(guard.askAboveUsd)),
            options = ASK_ABOVE_PRESETS_USD.map { aiUsdText(it) },
            selected = ASK_ABOVE_PRESETS_USD.indexOf(guard.askAboveUsd).coerceAtLeast(0),
            onPick = { i ->
                vm.setGuard(guard.copy(askAboveUsd = ASK_ABOVE_PRESETS_USD[i]))
                close()
            },
            onDismiss = close,
        )
        SpendPick.LIMIT -> {
            val limit = guard.monthlyLimitUsd
            TvOptionPicker(
                title = stringResource(R.string.spend_limit),
                options = listOf(stringResource(R.string.spend_limit_off)) + LIMIT_PRESETS_USD.map { aiUsdText(it) },
                selected = if (limit == null) 0 else (LIMIT_PRESETS_USD.indexOf(limit) + 1).coerceAtLeast(0),
                onPick = { i ->
                    vm.setGuard(guard.copy(monthlyLimitUsd = LIMIT_PRESETS_USD.getOrNull(i - 1)))
                    close()
                },
                onDismiss = close,
            )
        }
        SpendPick.CLEAR -> TvConfirmDialog(
            title = stringResource(R.string.spend_clear_title),
            text = stringResource(R.string.spend_clear_text),
            confirmLabel = stringResource(R.string.spend_clear),
            onConfirm = vm::clear,
            onDismiss = close,
        )
    }
}
