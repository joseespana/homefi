package com.perseusoft.homefi.player

import android.content.ContentProvider
import android.content.ContentValues
import android.content.Context
import android.database.Cursor
import android.net.Uri
import android.os.ParcelFileDescriptor
import com.perseusoft.homefi.HomefiApplication
import com.perseusoft.homefi.core.readCover
import java.io.File
import java.io.FileNotFoundException
import java.security.MessageDigest

/**
 * Covers for Android Auto. A car can't read a file path, so a library song's embedded cover is
 * served as `content://<app>.covers/cover?path=<file>`. Only songs of the library are served,
 * read-only, from a small cache.
 */
class CoverProvider : ContentProvider() {
    override fun onCreate(): Boolean = true

    override fun openFile(uri: Uri, mode: String): ParcelFileDescriptor {
        if (mode != "r") throw SecurityException("covers are read-only")
        val context = context ?: throw FileNotFoundException()
        val path = uri.getQueryParameter(PATH) ?: throw FileNotFoundException()
        // never an arbitrary file: only songs the library knows
        (context.applicationContext as HomefiApplication).library.find(path) ?: throw FileNotFoundException(path)
        val cached = File(File(context.cacheDir, CACHE_DIR).apply { mkdirs() }, key(path))
        if (!cached.isFile) {
            val bytes = readCover(path) ?: throw FileNotFoundException("no cover")
            val tmp = File(cached.parentFile, cached.name + ".tmp")
            tmp.writeBytes(bytes)
            tmp.renameTo(cached)
        }
        return ParcelFileDescriptor.open(cached, ParcelFileDescriptor.MODE_READ_ONLY)
    }

    override fun getType(uri: Uri): String = "image/*"
    override fun query(uri: Uri, p: Array<out String>?, s: String?, a: Array<out String>?, o: String?): Cursor? = null
    override fun insert(uri: Uri, values: ContentValues?): Uri? = null
    override fun delete(uri: Uri, s: String?, a: Array<out String>?): Int = 0
    override fun update(uri: Uri, v: ContentValues?, s: String?, a: Array<out String>?): Int = 0

    companion object {
        private const val PATH = "path"
        private const val CACHE_DIR = "car-covers"

        /** The cover of the library file at [path], for a car. */
        fun uriFor(context: Context, path: String): Uri =
            Uri.Builder().scheme("content").authority("${context.packageName}.covers").path("cover")
                .appendQueryParameter(PATH, path).build()

        private fun key(path: String): String =
            MessageDigest.getInstance("SHA-1").digest(path.toByteArray()).joinToString("") { "%02x".format(it) }
    }
}
