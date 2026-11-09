package com.perseusoft.homefi.data

import android.content.Context
import android.os.Environment
import android.os.StatFs
import java.io.File

/** A mounted storage volume with its free space: what the TV's Downloads screen shows as a card. */
data class VolumeInfo(
    val label: String,
    val root: File,
    val removable: Boolean,
    val freeBytes: Long,
    val totalBytes: Long,
) {
    /** Share of the volume that is used, 0..1. */
    val usedFraction: Float get() = if (totalBytes > 0) (1f - freeBytes.toFloat() / totalBytes).coerceIn(0f, 1f) else 0f
}

/** Where one download job writes, decided from `tv.downloadTarget` and what is plugged in. */
sealed interface DownloadDestination {
    data class Dir(val dir: File, val onUsb: Boolean) : DownloadDestination

    /** The user chose USB but the drive is not there: the job waits. */
    data object UsbMissing : DownloadDestination

    /** `tv.downloadTarget` is "ask" and a drive is plugged in: the user decides. */
    data object Ask : DownloadDestination
}

/** Pure rules of TV storage (volumes, free space, where a download goes): unit-tested without a device. */
object TvStorage {
    private const val BYTES_PER_MB = 1024L * 1024L

    /** The free space is under what the core's `download::space` rule keeps: new jobs would be refused. */
    fun isLow(freeBytes: Long, minFreeMb: Int): Boolean = freeBytes < minFreeMb * BYTES_PER_MB

    /** What a target setting means with the drive present or not. */
    fun destination(target: DownloadTarget, internalDir: File, usbDir: File?): DownloadDestination = when (target) {
        DownloadTarget.INTERNAL -> DownloadDestination.Dir(internalDir, onUsb = false)
        DownloadTarget.USB -> usbDir?.let { DownloadDestination.Dir(it, onUsb = true) } ?: DownloadDestination.UsbMissing
        DownloadTarget.ASK -> if (usbDir == null) DownloadDestination.Dir(internalDir, onUsb = false) else DownloadDestination.Ask
    }

    /** The volume that holds [path] (the longest root that is a parent of it), if any. */
    fun volumeOf(path: String, volumes: List<VolumeInfo>): VolumeInfo? =
        volumes.filter { path == it.root.path || path.startsWith(it.root.path + File.separator) }.maxByOrNull { it.root.path.length }

    /**
     * Roots of [known] removable folders that are not readable now: their songs stay in the list
     * (the core keeps its cache of a missing root) but are marked unavailable.
     */
    fun missingRoots(known: List<String>, exists: (String) -> Boolean = { File(it).exists() }): List<String> = known.filterNot(exists)

    /** [path] lies under one of the [missing] roots. */
    fun isUnavailable(path: String, missing: List<String>): Boolean =
        missing.any { path == it || path.startsWith(it.trimEnd(File.separatorChar) + File.separator) }

    /** The folder downloads use on [volume]: `HomeFi/` with "All files access", else the app's own folder on that drive. */
    fun downloadDirOn(context: Context, volume: VolumeInfo, sharedAccess: Boolean): File {
        if (sharedAccess) return File(volume.root, "HomeFi")
        val own = context.getExternalFilesDirs(Environment.DIRECTORY_MUSIC).filterNotNull()
            .firstOrNull { volumeOf(it.path, listOf(volume)) != null }
        // no app folder on the drive: nothing safe to write to
        return File(own ?: File(volume.root, "Android/data/${context.packageName}/files/Music"), "HomeFi")
    }

    /** Mounted volumes with their free space (a statvfs per volume: call off the main thread). */
    fun volumes(context: Context): List<VolumeInfo> = Storage.volumes(context).map { v ->
        val stat = runCatching { StatFs(v.root.path) }.getOrNull()
        VolumeInfo(v.label, v.root, v.removable, stat?.availableBytes ?: 0L, stat?.totalBytes ?: 0L)
    }
}
