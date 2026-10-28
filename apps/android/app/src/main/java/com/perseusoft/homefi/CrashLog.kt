package com.perseusoft.homefi

import android.content.Context
import android.media.MediaScannerConnection
import android.os.Build
import android.util.Log
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.concurrent.TimeUnit

/**
 * Keeps what happened when the app closed by itself, for devices without adb (the SR35):
 * files in `Android/data/<package>/files/logs/`, which a computer can read over USB (MTP).
 *
 * - A Java/Kotlin crash writes `crash-<time>.txt` with the stack trace and this app's logcat.
 * - A run that ended any other way (a native crash, or Android killed the app) leaves
 *   `previous-<time>.txt` with the logcat Android still keeps from that run.
 */
object CrashLog {
    private const val TAG = "HomeFi"
    private const val KEEP = 8

    private lateinit var dir: File
    private val alive get() = File(dir, ".running")

    fun install(context: Context) {
        val app = context.applicationContext
        dir = File(app.getExternalFilesDir(null) ?: app.filesDir, "logs").also { it.mkdirs() }

        if (alive.exists()) write("previous", "The last run ended without closing (crash or killed by Android).\n")
        alive.writeText(Date().toString())

        val previous = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { thread, error ->
            runCatching {
                write("crash", "Thread: ${thread.name}\n\n${Log.getStackTraceString(error)}\n")
                alive.delete()
            }
            previous?.uncaughtException(thread, error)
        }

        trim()
        // MTP only lists files the media scanner knows
        dir.listFiles()?.filter { it.isFile && !it.name.startsWith(".") }?.map { it.absolutePath }?.let { paths ->
            if (paths.isNotEmpty()) MediaScannerConnection.scanFile(app, paths.toTypedArray(), null, null)
        }
    }

    private fun write(kind: String, header: String) {
        val stamp = SimpleDateFormat("yyyyMMdd-HHmmss", Locale.US).format(Date())
        val text = buildString {
            append(header)
            append("\nDevice: ${Build.MANUFACTURER} ${Build.MODEL}, Android ${Build.VERSION.RELEASE} (API ${Build.VERSION.SDK_INT})\n")
            append("App: ${BuildConfig.APPLICATION_ID} ${BuildConfig.VERSION_NAME} (${BuildConfig.VERSION_CODE})\n\n")
            append(logcat())
        }
        runCatching { File(dir, "$kind-$stamp.txt").writeText(text) }.onFailure { Log.w(TAG, "crash log", it) }
    }

    /** This app's own lines (Android only shows an app its own logs), threads and times included. */
    private fun logcat(): String = runCatching {
        val process = ProcessBuilder("logcat", "-d", "-v", "threadtime", "-t", "1500").redirectErrorStream(true).start()
        val out = process.inputStream.bufferedReader().readText()
        process.waitFor(3, TimeUnit.SECONDS)
        out
    }.getOrElse { "logcat: $it\n" }

    private fun trim() {
        dir.listFiles()?.filter { it.isFile && !it.name.startsWith(".") }
            ?.sortedByDescending { it.lastModified() }
            ?.drop(KEEP)
            ?.forEach { it.delete() }
    }
}
