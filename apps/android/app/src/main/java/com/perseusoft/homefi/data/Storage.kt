package com.perseusoft.homefi.data

import android.Manifest
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.media.MediaScannerConnection
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.os.storage.StorageManager
import android.provider.Settings
import com.perseusoft.homefi.R
import java.io.File

/**
 * Where downloads go: `Music/HomeFi` when the app may write to shared
 * storage, otherwise the app's own music folder (`Android/data/...`).
 */
object Storage {
    fun hasSharedAccess(context: Context): Boolean =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            Environment.isExternalStorageManager()
        } else {
            context.checkSelfPermission(Manifest.permission.WRITE_EXTERNAL_STORAGE) ==
                PackageManager.PERMISSION_GRANTED
        }

    @Suppress("DEPRECATION")
    fun sharedDir(): File =
        File(Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_MUSIC), "HomeFi")

    fun appDir(context: Context): File =
        File(context.getExternalFilesDir(Environment.DIRECTORY_MUSIC) ?: context.filesDir, "HomeFi")

    /** The picked folder when the app can write there, otherwise the default. */
    fun downloadDir(context: Context, picked: String = ""): File {
        if (picked.isNotBlank()) File(picked).takeIf(::isWritable)?.let { return it }
        return (if (hasSharedAccess(context)) sharedDir() else appDir(context)).also { it.mkdirs() }
    }

    fun isWritable(dir: File): Boolean = (dir.isDirectory || dir.mkdirs()) && dir.canWrite()

    /** A mounted storage volume: internal storage, an SD card, a USB drive. */
    data class Volume(val label: String, val root: File, val removable: Boolean)

    fun volumes(context: Context): List<Volume> {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            val manager = context.getSystemService(StorageManager::class.java)
            return manager.storageVolumes
                .filter { it.state == Environment.MEDIA_MOUNTED }
                .mapNotNull { v -> v.directory?.let { Volume(v.getDescription(context), it, v.isRemovable) } }
        }
        // older Android: each volume has an app folder at <root>/Android/data/<package>/files
        return context.getExternalFilesDirs(null).filterNotNull().mapIndexedNotNull { i, dir ->
            val root = dir.absolutePath.substringBefore("/Android/data/", "").takeIf { it.isNotEmpty() }
            root?.let { Volume(context.getString(if (i == 0) R.string.folder_internal else R.string.folder_sd), File(it), i > 0) }
        }
    }

    /** Settings screen that grants "All files access" (Android 11+). */
    fun accessSettingsIntent(context: Context): Intent? =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION, Uri.parse("package:${context.packageName}"))
        } else {
            null
        }

    /** Permission that lets the app read music in shared storage. */
    val audioPermission: String
        get() = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            Manifest.permission.READ_MEDIA_AUDIO
        } else {
            Manifest.permission.READ_EXTERNAL_STORAGE
        }

    fun hasAudioReadAccess(context: Context): Boolean =
        hasSharedAccess(context) ||
            context.checkSelfPermission(audioPermission) == PackageManager.PERMISSION_GRANTED

    /** Folders scanned for the library; the scanner merges nested roots. */
    @Suppress("DEPRECATION")
    fun libraryRoots(context: Context, picked: String = ""): List<File> = buildList {
        add(appDir(context))
        if (picked.isNotBlank()) File(picked).takeIf { it.canRead() }?.let(::add)
        if (hasAudioReadAccess(context)) {
            add(Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_MUSIC))
            add(Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS))
        }
    }

    /** Makes a finished file show up in music players. */
    fun scan(context: Context, path: String) {
        MediaScannerConnection.scanFile(context, arrayOf(path), null, null)
    }
}
