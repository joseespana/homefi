package com.perseusoft.homefi.ui

import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material3.MaterialTheme
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.ui.Modifier
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.OutlinedTextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import com.perseusoft.homefi.R

/** Asks for a name (a ride, a DJ setup). A blank name cannot be confirmed. */
@Composable
fun NameDialog(
    title: String,
    initial: String,
    confirm: String,
    onConfirm: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    var name by rememberSaveable { mutableStateOf(initial) }
    val ready = name.isNotBlank()
    HomefiDialog(
        title = title,
        icon = Icons.Rounded.Edit,
        primary = DialogAction(confirm, enabled = ready) { onConfirm(name.trim()) },
        dismiss = stringResource(R.string.xfer_cancel),
        onDismiss = onDismiss,
    ) {
        OutlinedTextField(
            value = name,
            onValueChange = { name = it },
            singleLine = true,
            keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences, imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { if (ready) onConfirm(name.trim()) }),
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

/** "Delete X?" with the item's name; [onConfirm] only after a tap on Delete. */
@Composable
fun ConfirmDeleteDialog(title: String, text: String, onConfirm: () -> Unit, onDismiss: () -> Unit) {
    HomefiDialog(
        title = title,
        text = text,
        icon = Icons.Rounded.DeleteOutline,
        accent = MaterialTheme.colorScheme.error,
        primary = DialogAction(stringResource(R.string.xfer_delete), danger = true, onClick = onConfirm),
        dismiss = stringResource(R.string.xfer_cancel),
        onDismiss = onDismiss,
    )
}
