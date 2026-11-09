package com.perseusoft.homefi.data

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import androidx.core.content.FileProvider
import com.perseusoft.homefi.R
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.io.IOException

/**
 * Moves the text of an export (a ride or a DJ setup) in and out of the phone: a document the rider
 * picks with the system file picker (local storage, Google Drive...) or a file handed to the share sheet.
 * The text itself is made and read by the core; this only does the file work.
 */
object FileExchange {
    /** Exports are small JSON files; anything bigger is not one of ours. */
    private const val MAX_BYTES = 4 * 1024 * 1024

    /** What an unnamed export is called. */
    private const val FALLBACK_NAME = "homefi"

    private const val EXPORT_DIR = "exports"
    private const val MAX_NAME_CHARS = 60
    private val UNSAFE = Regex("""[\\/:*?"<>|\p{Cntrl}]""")

    /** "My Sunday ride" + `homefiride` -> "My Sunday ride.homefiride", safe for any file system. */
    fun fileName(name: String, extension: String): String {
        val clean = name.replace(UNSAFE, " ").trim().replace(Regex("\\s+"), " ").take(MAX_NAME_CHARS).trim('.', ' ')
        return "${clean.ifEmpty { FALLBACK_NAME }}.$extension"
    }

    suspend fun read(context: Context, uri: Uri): String = withContext(Dispatchers.IO) {
        val stream = context.contentResolver.openInputStream(uri) ?: throw IOException(context.getString(R.string.xfer_cant_open))
        stream.use { input ->
            val bytes = input.readNBytes(MAX_BYTES + 1)
            if (bytes.size > MAX_BYTES) throw IOException(context.getString(R.string.xfer_too_big))
            bytes.toString(Charsets.UTF_8)
        }
    }

    suspend fun write(context: Context, uri: Uri, text: String) = withContext(Dispatchers.IO) {
        val stream = context.contentResolver.openOutputStream(uri, "wt") ?: throw IOException(context.getString(R.string.xfer_cant_save))
        stream.use { it.write(text.toByteArray(Charsets.UTF_8)) }
    }

    /** The file's own name, when the provider knows it. */
    suspend fun displayName(context: Context, uri: Uri): String? = withContext(Dispatchers.IO) {
        if (uri.scheme == "file") return@withContext uri.lastPathSegment
        runCatching {
            context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
                if (c.moveToFirst()) c.getString(0) else null
            }
        }.getOrNull()
    }

    /** The share sheet's intent for [text] as a file called [fileName] (only the latest export is kept in the cache). */
    suspend fun shareIntent(context: Context, fileName: String, text: String, title: String): Intent = withContext(Dispatchers.IO) {
        val dir = File(context.cacheDir, EXPORT_DIR).also { it.deleteRecursively(); it.mkdirs() }
        val file = File(dir, fileName).also { it.writeText(text, Charsets.UTF_8) }
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.updates", file)
        val send = Intent(Intent.ACTION_SEND)
            .setType(MIME_JSON)
            .putExtra(Intent.EXTRA_STREAM, uri)
            .putExtra(Intent.EXTRA_SUBJECT, fileName)
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        Intent.createChooser(send, title).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    }

    const val MIME_JSON = "application/json"
}
