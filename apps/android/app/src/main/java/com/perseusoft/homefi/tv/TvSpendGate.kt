package com.perseusoft.homefi.tv

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.data.SpendGate
import com.perseusoft.homefi.homefi

/** The TV's "ask before spending" card: Cancel takes the focus first, so a stray OK never spends. */
@Composable
fun TvSpendGate() {
    val gate = LocalContext.current.homefi.spend
    val ask by gate.ask.collectAsStateWithLifecycle()
    when (val question = ask) {
        null -> Unit
        is SpendGate.Ask.Confirm -> TvConfirmDialog(
            title = question.title,
            text = question.text,
            confirmLabel = stringResource(R.string.spend_continue),
            onConfirm = { gate.answer(true) },
            onDismiss = { gate.answer(false) },
        )
        is SpendGate.Ask.Blocked -> TvConfirmDialog(
            title = question.title,
            text = question.text,
            confirmLabel = stringResource(R.string.spend_ok),
            cancelLabel = stringResource(R.string.cancel),
            onConfirm = { gate.answer(false) },
            onDismiss = { gate.answer(false) },
        )
    }
}
