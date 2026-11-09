package com.perseusoft.homefi.data

import android.content.Context
import android.net.Uri
import com.perseusoft.homefi.R
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import org.json.JSONObject

/** What kind of HomeFi export a file is. */
enum class ExportKind { RIDE, DJ_SETUP }

/** A ride or DJ setup file opened from another app (Drive, Files, a chat), waiting for the rider's yes. */
data class IncomingFile(val kind: ExportKind, val fileName: String, val text: String)

/**
 * Files that arrive from outside ("Open with HomeFi"): the file is read and recognised by its envelope
 * (the core's `kind`), then the rider confirms before it is added. Nothing is saved without that yes.
 */
class IncomingFiles(
    private val context: Context,
    private val scope: CoroutineScope,
    private val notes: Notes,
    private val rides: SavedRides,
    private val setups: DjSetupsStore,
) {
    private val _pending = MutableStateFlow<IncomingFile?>(null)
    val pending: StateFlow<IncomingFile?> = _pending.asStateFlow()

    fun offer(uri: Uri) {
        scope.launch {
            try {
                val text = FileExchange.read(context, uri)
                val kind = kindOf(text)
                if (kind == null) notes.post(context.getString(R.string.xfer_not_homefi))
                else _pending.value = IncomingFile(kind, FileExchange.displayName(context, uri).orEmpty(), text)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                notes.post(e.userMessage())
            }
        }
    }

    fun dismiss() {
        _pending.value = null
    }

    fun confirm() {
        val file = _pending.value ?: return
        _pending.value = null
        scope.launch {
            when (file.kind) {
                ExportKind.RIDE -> rides.import(file.text)
                ExportKind.DJ_SETUP -> setups.import(file.text)
            }
        }
    }

    private fun kindOf(text: String): ExportKind? = runCatching {
        val envelope = JSONObject(text)
        when {
            !envelope.has(ENVELOPE_KEY) -> null
            envelope.optString("kind") == "ride" -> ExportKind.RIDE
            envelope.optString("kind") == "dj_setup" -> ExportKind.DJ_SETUP
            else -> null
        }
    }.getOrNull()

    private companion object {
        /** The core's export envelope: `{"homefi_export": 1, "kind": ...}`. */
        const val ENVELOPE_KEY = "homefi_export"
    }
}
