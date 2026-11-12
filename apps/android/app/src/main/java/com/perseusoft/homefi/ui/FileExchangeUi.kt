package com.perseusoft.homefi.ui

import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.FileDownload
import android.content.ActivityNotFoundException
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.data.ExportKind
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import com.perseusoft.homefi.R
import com.perseusoft.homefi.data.FileExchange
import com.perseusoft.homefi.data.userMessage
import com.perseusoft.homefi.homefi
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch

/** What a screen can do with an export file: save it to a document, hand it to the share sheet, pick one to import. */
@Immutable
class FileExchangeActions(
    /** Opens the system "save as" (local storage, Google Drive...) for [text] named [fileName]. */
    val saveAs: (fileName: String, text: String) -> Unit,
    /** Opens the share sheet with [text] as a file named [fileName]. */
    val share: (fileName: String, text: String) -> Unit,
    /** Opens the system file picker; [rememberFileExchange]'s `onPicked` gets the text. */
    val pick: () -> Unit,
)

/**
 * The system pickers for exporting and importing HomeFi files (Storage Access Framework, so Drive and
 * any document provider work, and the share sheet). Reading and writing the document is
 * [FileExchange]'s; [onPicked] gets the text of the file the rider chose.
 */
@Composable
fun rememberFileExchange(onPicked: (String) -> Unit): FileExchangeActions {
    val context = LocalContext.current
    val app = context.homefi
    val scope = rememberCoroutineScope()
    var pending by remember { mutableStateOf<String?>(null) }
    val saved = stringResource(R.string.xfer_saved)
    val chooserTitle = stringResource(R.string.xfer_share_title)

    // "octet-stream" keeps the file's own name (the picker would add ".json" to a known type)
    val create = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument(MIME_ANY_BINARY)) { uri ->
        val text = pending
        pending = null
        if (uri != null && text != null) {
            scope.launch {
                try {
                    FileExchange.write(context, uri, text)
                    app.notes.post(saved)
                } catch (e: CancellationException) {
                    throw e
                } catch (e: Exception) {
                    app.notes.post(e.userMessage())
                }
            }
        }
    }
    val open = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) {
            scope.launch {
                try {
                    onPicked(FileExchange.read(context, uri))
                } catch (e: CancellationException) {
                    throw e
                } catch (e: Exception) {
                    app.notes.post(e.userMessage())
                }
            }
        }
    }
    return remember(create, open, chooserTitle) {
        FileExchangeActions(
            saveAs = { fileName, text ->
                pending = text
                try {
                    create.launch(fileName)
                } catch (e: ActivityNotFoundException) {
                    pending = null
                    app.notes.post(e.userMessage())
                }
            },
            share = { fileName, text ->
                scope.launch {
                    try {
                        context.startActivity(FileExchange.shareIntent(context, fileName, text, chooserTitle))
                    } catch (e: CancellationException) {
                        throw e
                    } catch (e: Exception) {
                        app.notes.post(e.userMessage())
                    }
                }
            },
            pick = {
                try {
                    open.launch(arrayOf("*/*"))
                } catch (e: ActivityNotFoundException) {
                    app.notes.post(e.userMessage())
                }
            },
        )
    }
}

private const val MIME_ANY_BINARY = "application/octet-stream"

/** Shows [com.perseusoft.homefi.HomefiApplication.notes] on the snackbar [host] (they queue behind each other). */
@Composable
fun NoteNotices(host: SnackbarHostState) {
    val app = LocalContext.current.homefi
    LaunchedEffect(host) {
        app.notes.flow.collect { note ->
            launch {
                val result = host.showSnackbar(note.text, actionLabel = note.action?.label, duration = SnackbarDuration.Long)
                if (result == SnackbarResult.ActionPerformed) note.action?.run?.invoke()
            }
        }
    }
}

/** A ride or DJ setup file opened from another app: asks before adding it. */
@Composable
fun IncomingFileDialog() {
    val app = LocalContext.current.homefi
    val file by app.incoming.pending.collectAsStateWithLifecycle()
    val incoming = file ?: return
    val name = incoming.fileName.ifBlank { stringResource(R.string.xfer_this_file) }
    HomefiDialog(
        title = stringResource(if (incoming.kind == ExportKind.RIDE) R.string.xfer_open_ride else R.string.xfer_open_setup),
        text = stringResource(R.string.xfer_open_text, name),
        icon = Icons.Rounded.FileDownload,
        primary = DialogAction(stringResource(R.string.xfer_add), onClick = app.incoming::confirm),
        dismiss = stringResource(R.string.xfer_cancel),
        onDismiss = app.incoming::dismiss,
    )
}
