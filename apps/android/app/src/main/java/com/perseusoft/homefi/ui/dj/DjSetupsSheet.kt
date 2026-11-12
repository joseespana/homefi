package com.perseusoft.homefi.ui.dj

import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import com.perseusoft.homefi.ui.DialogAction
import com.perseusoft.homefi.ui.HomefiDialog
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material.icons.rounded.FileDownload
import androidx.compose.material.icons.rounded.FileUpload
import androidx.compose.material.icons.rounded.IosShare
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.Save
import androidx.compose.material.icons.rounded.SaveAs
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.DjSetupSummary
import com.perseusoft.homefi.data.DjSetupsStore
import com.perseusoft.homefi.data.FileExchange
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.ConfirmDeleteDialog
import com.perseusoft.homefi.ui.NameDialog
import com.perseusoft.homefi.ui.NoteNotices
import com.perseusoft.homefi.ui.haptics
import com.perseusoft.homefi.ui.rememberFileExchange
import com.perseusoft.homefi.ui.theme.Card
import kotlinx.coroutines.launch

private val TARGET = 48.dp

private sealed interface SetupDialog {
    data object Save : SetupDialog
    data class Rename(val setup: DjSetupSummary) : SetupDialog
    data class Overwrite(val setup: DjSetupSummary) : SetupDialog
    data class Delete(val setup: DjSetupSummary) : SetupDialog
}

/**
 * "Setups": the DJ's options and screen layout kept under a name. Save what is set now, apply one, rename,
 * replace or delete it, and move it between devices: export to a file (local storage, Google Drive...) or the
 * share sheet, import from either. The files are the same the desktop reads.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DjSetupsSheet(onDismiss: () -> Unit) {
    val app = LocalContext.current.homefi
    val setups by app.djSetups.list.collectAsStateWithLifecycle()
    val haptics = haptics()
    val scope = rememberCoroutineScope()
    var dialog by remember { mutableStateOf<SetupDialog?>(null) }
    val snackbar = remember { SnackbarHostState() }
    LaunchedEffect(Unit) { app.djSetups.refresh() }
    // the sheet covers the screen's own snackbar: show messages here too
    NoteNotices(snackbar)

    val exchange = rememberFileExchange { text -> scope.launch { app.djSetups.import(text) } }

    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Box {
            Column(
                Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 16.dp).padding(bottom = 24.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Text(stringResource(R.string.djs_setups), style = MaterialTheme.typography.titleLarge)
                Text(
                    stringResource(R.string.djs_setups_hint),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Button(onClick = { dialog = SetupDialog.Save }, modifier = Modifier.weight(1f).heightIn(min = TARGET)) {
                        Icon(Icons.Rounded.Save, null, Modifier.size(18.dp))
                        Text(stringResource(R.string.djs_save_current), Modifier.padding(start = 8.dp))
                    }
                    FilledTonalButton(onClick = exchange.pick, modifier = Modifier.heightIn(min = TARGET)) {
                        Icon(Icons.Rounded.FileUpload, null, Modifier.size(18.dp))
                        Text(stringResource(R.string.rides_import), Modifier.padding(start = 8.dp))
                    }
                }
                if (setups.isEmpty()) {
                    Text(
                        stringResource(R.string.djs_empty),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(vertical = 8.dp),
                    )
                }
                setups.forEach { setup ->
                    key(setup.id) {
                        SetupRow(
                            setup = setup,
                            onApply = { haptics.confirm(); app.djSetups.apply(setup.id) },
                            onOverwrite = { dialog = SetupDialog.Overwrite(setup) },
                            onRename = { dialog = SetupDialog.Rename(setup) },
                            onDelete = { dialog = SetupDialog.Delete(setup) },
                            onSaveAs = {
                                scope.launch {
                                    app.djSetups.export(setup.id)?.let { exchange.saveAs(FileExchange.fileName(setup.name, DjSetupsStore.extension), it) }
                                }
                            },
                            onShare = {
                                scope.launch {
                                    app.djSetups.export(setup.id)?.let { exchange.share(FileExchange.fileName(setup.name, DjSetupsStore.extension), it) }
                                }
                            },
                        )
                    }
                }
            }
            SnackbarHost(snackbar, Modifier.align(Alignment.BottomCenter))
        }
    }

    when (val d = dialog) {
        SetupDialog.Save -> NameDialog(
            title = stringResource(R.string.djs_save_current),
            initial = "",
            confirm = stringResource(R.string.rides_save),
            onConfirm = { name -> dialog = null; app.djSetups.saveCurrent(name) },
            onDismiss = { dialog = null },
        )
        is SetupDialog.Rename -> NameDialog(
            title = stringResource(R.string.rides_rename),
            initial = d.setup.name,
            confirm = stringResource(R.string.rides_save),
            onConfirm = { name -> dialog = null; app.djSetups.rename(d.setup.id, name) },
            onDismiss = { dialog = null },
        )
        is SetupDialog.Overwrite -> ConfirmOverwrite(d.setup.name, onConfirm = { dialog = null; app.djSetups.overwrite(d.setup.id) }, onDismiss = { dialog = null })
        is SetupDialog.Delete -> ConfirmDeleteDialog(
            title = stringResource(R.string.djs_delete_title),
            text = stringResource(R.string.djs_delete_text, d.setup.name),
            onConfirm = { dialog = null; app.djSetups.delete(d.setup.id) },
            onDismiss = { dialog = null },
        )
        null -> Unit
    }
}

@Composable
private fun ConfirmOverwrite(name: String, onConfirm: () -> Unit, onDismiss: () -> Unit) {
    HomefiDialog(
        title = stringResource(R.string.djs_overwrite_title),
        text = stringResource(R.string.djs_overwrite_text, name),
        icon = Icons.Rounded.Save,
        primary = DialogAction(stringResource(R.string.djs_overwrite), onClick = onConfirm),
        dismiss = stringResource(R.string.xfer_cancel),
        onDismiss = onDismiss,
    )
}

@Composable
private fun SetupRow(
    setup: DjSetupSummary,
    onApply: () -> Unit,
    onOverwrite: () -> Unit,
    onRename: () -> Unit,
    onDelete: () -> Unit,
    onSaveAs: () -> Unit,
    onShare: () -> Unit,
) {
    Card(Modifier.fillMaxWidth()) {
        Row(
            Modifier.padding(start = 16.dp, top = 8.dp, bottom = 8.dp, end = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(setup.name, style = MaterialTheme.typography.titleSmall, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
            FilledTonalButton(onClick = onApply, modifier = Modifier.heightIn(min = TARGET)) { Text(stringResource(R.string.djs_apply)) }
            var open by remember { mutableStateOf(false) }
            Box {
                IconButton(onClick = { open = true }, modifier = Modifier.size(TARGET)) {
                    Icon(Icons.Rounded.MoreVert, stringResource(R.string.rides_more))
                }
                DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
                    MenuItem(R.string.djs_overwrite, Icons.Rounded.SaveAs) { open = false; onOverwrite() }
                    MenuItem(R.string.rides_rename, Icons.Rounded.Edit) { open = false; onRename() }
                    MenuItem(R.string.rides_save_file, Icons.Rounded.FileDownload) { open = false; onSaveAs() }
                    MenuItem(R.string.rides_share, Icons.Rounded.IosShare) { open = false; onShare() }
                    MenuItem(R.string.xfer_delete, Icons.Rounded.DeleteOutline) { open = false; onDelete() }
                }
            }
        }
    }
}

@Composable
private fun MenuItem(label: Int, icon: ImageVector, onClick: () -> Unit) {
    DropdownMenuItem(
        text = { Text(stringResource(label)) },
        leadingIcon = { Icon(icon, null) },
        onClick = onClick,
        modifier = Modifier.heightIn(min = TARGET),
    )
}
