package com.perseusoft.homefi

import android.app.Activity
import android.app.ActivityManager
import android.app.Application
import android.app.ApplicationExitInfo
import android.content.Context
import android.media.MediaScannerConnection
import android.os.Build
import android.os.Bundle
import android.util.Log
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.concurrent.TimeUnit

/**
 * Why a run is worth a `previous-<time>.txt`, or null when it ended normally. Pure, so it is tested.
 *
 * - Android 11+ (API 30) keeps the reason each process died ([lastExitReason], an
 *   `ApplicationExitInfo.REASON_*`): a native crash, an ANR kill or a failed start is reported; the
 *   user closing the app, Android freeing memory, a Java crash (written by the handler) or an
 *   update are not.
 * - Older Android has no such history: the `.running` marker ([markerLeft]) is written while an
 *   activity is on screen and removed when the app goes to the background, so finding it at the
 *   next start means the process died in the foreground (native crash or ANR kill).
 */
internal fun previousRunProblem(apiLevel: Int, markerLeft: Boolean, lastExitReason: Int?): String? = when {
    apiLevel >= Build.VERSION_CODES.R -> when (lastExitReason) {
        ApplicationExitInfo.REASON_CRASH_NATIVE -> "The last run ended with a native crash."
        ApplicationExitInfo.REASON_ANR -> "The last run was killed for not responding (ANR)."
        ApplicationExitInfo.REASON_INITIALIZATION_FAILURE -> "The last run failed while starting."
        else -> null
    }
    markerLeft -> "The last run ended in the foreground without closing (native crash or ANR)."
    else -> null
}

/**
 * Keeps what happened when the app closed by itself, for devices without adb (the SR35):
 * files in `Android/data/<package>/files/logs/`, which a computer can read over USB (MTP).
 *
 * - A Java/Kotlin crash writes `crash-<time>.txt` with the stack trace and this app's logcat.
 * - A run that ended any other way (a native crash, an ANR) leaves `previous-<time>.txt` with
 *   the logcat Android still keeps from that run ([previousRunProblem] decides).
 *
 * Only the uncaught-exception handler is installed on the main thread; reading the previous run,
 * the logcat, trimming and the media scanner run on a background thread.
 */
object CrashLog {
    private const val TAG = "HomeFi"
    private const val KEEP = 8
    private const val LOGCAT_LINES = "1500"
    private const val LOGCAT_TIMEOUT_S = 3L

    private lateinit var app: Context
    /** Resolved on first use (off the main thread), the handler included. */
    private val dir: File by lazy {
        File(app.getExternalFilesDir(null) ?: app.filesDir, "logs").also { it.mkdirs() }
    }
    private val alive get() = File(dir, ".running")

    fun install(context: Context) {
        app = context.applicationContext
        val previous = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { thread, error ->
            runCatching {
                write("crash", "Thread: ${thread.name}\n\n${Log.getStackTraceString(error)}\n")
                if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) alive.delete()
            }
            previous?.uncaughtException(thread, error)
        }
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) trackForeground(app as Application)
        Thread(::checkPreviousRun, "homefi-crashlog").apply { isDaemon = true }.start()
    }

    /** Before Android 11: the marker exists while the app is on screen. */
    private fun trackForeground(application: Application) {
        var started = 0
        application.registerActivityLifecycleCallbacks(object : Application.ActivityLifecycleCallbacks {
            override fun onActivityStarted(activity: Activity) {
                if (started++ == 0) Thread { runCatching { alive.writeText(Date().toString()) } }.start()
            }

            override fun onActivityStopped(activity: Activity) {
                // a rotation stops and starts again at once: only the last stop of a real exit counts
                if (--started == 0 && !activity.isChangingConfigurations) Thread { runCatching { alive.delete() } }.start()
            }

            override fun onActivityCreated(activity: Activity, savedInstanceState: Bundle?) = Unit
            override fun onActivityResumed(activity: Activity) = Unit
            override fun onActivityPaused(activity: Activity) = Unit
            override fun onActivitySaveInstanceState(activity: Activity, outState: Bundle) = Unit
            override fun onActivityDestroyed(activity: Activity) = Unit
        })
    }

    private fun checkPreviousRun() {
        runCatching {
            val markerLeft = Build.VERSION.SDK_INT < Build.VERSION_CODES.R && alive.exists()
            val exit = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                app.getSystemService(ActivityManager::class.java)
                    .getHistoricalProcessExitReasons(app.packageName, 0, 1).firstOrNull()?.reason
            } else null
            previousRunProblem(Build.VERSION.SDK_INT, markerLeft, exit)?.let { write("previous", "$it\n") }
            trim()
            // MTP only lists files the media scanner knows
            dir.listFiles()?.filter { it.isFile && !it.name.startsWith(".") }?.map { it.absolutePath }?.let { paths ->
                if (paths.isNotEmpty()) MediaScannerConnection.scanFile(app, paths.toTypedArray(), null, null)
            }
        }.onFailure { Log.w(TAG, "crash log", it) }
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

    /**
     * This app's own lines (Android only shows an app its own logs), threads and times included.
     * Goes through a file so the pipe never fills, and the process is stopped after [LOGCAT_TIMEOUT_S].
     */
    private fun logcat(): String = runCatching {
        val out = File(dir, ".logcat.tmp")
        val process = ProcessBuilder("logcat", "-d", "-v", "threadtime", "-t", LOGCAT_LINES)
            .redirectErrorStream(true).redirectOutput(out).start()
        val done = process.waitFor(LOGCAT_TIMEOUT_S, TimeUnit.SECONDS)
        if (!done) process.destroyForcibly()
        val text = out.readText()
        out.delete()
        if (done) text else "$text\n(logcat did not finish in $LOGCAT_TIMEOUT_S s)\n"
    }.getOrElse { "logcat: $it\n" }

    private fun trim() {
        dir.listFiles()?.filter { it.isFile && !it.name.startsWith(".") }
            ?.sortedByDescending { it.lastModified() }
            ?.drop(KEEP)
            ?.forEach { it.delete() }
    }
}
