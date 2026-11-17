package com.perseusoft.homefi.ui

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Savings
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.data.SpendGate
import com.perseusoft.homefi.homefi

/** The phone's "ask before spending" dialog: whatever [SpendGate] is asking, over every screen. */
@Composable
fun SpendGateDialog() {
    val gate = LocalContext.current.homefi.spend
    val ask by gate.ask.collectAsStateWithLifecycle()
    when (val question = ask) {
        null -> Unit
        is SpendGate.Ask.Confirm -> HomefiDialog(
            title = question.title,
            text = question.text,
            icon = Icons.Rounded.Savings,
            onDismiss = { gate.answer(false) },
            primary = DialogAction(stringResource(R.string.spend_continue)) { gate.answer(true) },
            dismiss = stringResource(R.string.cancel),
        )
        is SpendGate.Ask.Blocked -> HomefiDialog(
            title = question.title,
            text = question.text,
            icon = Icons.Rounded.Savings,
            onDismiss = { gate.answer(false) },
            primary = DialogAction(stringResource(R.string.spend_ok)) { gate.answer(false) },
        )
    }
}
