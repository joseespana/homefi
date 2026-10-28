package com.perseusoft.homefi.ui.login

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.DialogProperties
import com.perseusoft.homefi.R

/**
 * The personal-use notice and its "I understand and accept" box, shown before connecting
 * Tidal (the login screen and, once, to accounts signed in before the notice existed).
 */
@Composable
fun TermsNotice(checked: Boolean, onCheckedChange: (Boolean) -> Unit, modifier: Modifier = Modifier) {
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Surface(color = MaterialTheme.colorScheme.surfaceVariant, shape = MaterialTheme.shapes.medium) {
            Text(
                stringResource(R.string.tidal_terms),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier
                    .heightIn(max = 220.dp)
                    .verticalScroll(rememberScrollState())
                    .padding(12.dp),
            )
        }
        Row(
            Modifier
                .fillMaxWidth()
                .toggleable(value = checked, role = Role.Checkbox, onValueChange = onCheckedChange),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            // the whole row toggles it; the box itself only shows the state
            Checkbox(checked = checked, onCheckedChange = null)
            Text(stringResource(R.string.tidal_terms_check), modifier = Modifier.padding(start = 12.dp))
        }
    }
}

/**
 * Signed in before the notice existed (or before its text changed): accept it to keep
 * going, or sign out. Nothing is lost either way; the library and downloads stay.
 */
@Composable
fun TermsDialog(onAccept: () -> Unit, onSignOut: () -> Unit) {
    var checked by rememberSaveable { mutableStateOf(false) }
    AlertDialog(
        onDismissRequest = {},
        properties = DialogProperties(dismissOnBackPress = false, dismissOnClickOutside = false),
        title = { Text(stringResource(R.string.tidal_terms_dialog_title)) },
        text = { TermsNotice(checked, { checked = it }) },
        confirmButton = {
            TextButton(onClick = onAccept, enabled = checked) { Text(stringResource(R.string.tidal_terms_continue)) }
        },
        dismissButton = {
            TextButton(onClick = onSignOut) { Text(stringResource(R.string.tidal_terms_sign_out)) }
        },
    )
}
