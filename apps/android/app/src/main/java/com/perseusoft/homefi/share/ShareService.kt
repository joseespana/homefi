package com.perseusoft.homefi.share

import android.app.Notification
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.util.Log
import androidx.core.app.NotificationChannelCompat
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import com.perseusoft.homefi.MainActivity
import com.perseusoft.homefi.R

/**
 * Keeps the process alive while sharing (hub) or joined (guest), so Android does not kill
 * the hub when the screen is off or the music is paused. Discreet: low importance, silent,
 * no heads-up (the owner may be driving).
 */
class ShareService : Service() {

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) {
            // A plain startService from the notification: no startForeground needed.
            val callback = onStopRequested
            Handler(Looper.getMainLooper()).post { callback?.invoke() }
            shutdown()
            return START_NOT_STICKY
        }
        if (!goForeground()) {
            shutdown()
            return START_NOT_STICKY
        }
        running = true
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        running = false
        super.onDestroy()
    }

    private fun goForeground(): Boolean {
        createChannel(this)
        val type = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE else 0
        return try {
            ServiceCompat.startForeground(this, NOTIFICATION_ID, notification(this, title, text), type)
            true
        } catch (e: RuntimeException) {
            // ForegroundServiceStartNotAllowedException (started from the background, API 31+)
            // or SecurityException (connectedDevice prerequisites missing, API 34+).
            Log.w(TAG, "could not start in the foreground", e)
            false
        }
    }

    private fun shutdown() {
        running = false
        ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    companion object {
        private const val TAG = "HomeFiShareService"
        private const val CHANNEL_ID = "share"
        private const val NOTIFICATION_ID = 3
        private const val ACTION_STOP = "com.perseusoft.homefi.share.STOP"

        @Volatile
        private var running = false

        @Volatile
        private var title = ""

        @Volatile
        private var text = ""

        /** Called (on the main thread) when the user taps "Stop" in the notification. */
        @Volatile
        var onStopRequested: (() -> Unit)? = null

        /** Must be called while the app is in the foreground (a UI action). */
        fun start(context: Context, title: String, text: String) {
            this.title = title
            this.text = text
            try {
                ContextCompat.startForegroundService(context, Intent(context, ShareService::class.java))
            } catch (e: IllegalStateException) {
                // Not allowed from the background on API 31+.
                Log.w(TAG, "could not start", e)
            }
        }

        /** Changes the notification text; does nothing when the service is not running. */
        fun update(context: Context, title: String, text: String) {
            this.title = title
            this.text = text
            if (!running) return
            val manager = NotificationManagerCompat.from(context)
            if (!manager.areNotificationsEnabled()) return
            try {
                manager.notify(NOTIFICATION_ID, notification(context, title, text))
            } catch (e: SecurityException) {
                Log.w(TAG, "notify", e)
            }
        }

        fun stop(context: Context) {
            running = false
            context.stopService(Intent(context, ShareService::class.java))
        }

        private fun createChannel(context: Context) {
            val channel = NotificationChannelCompat.Builder(CHANNEL_ID, NotificationManagerCompat.IMPORTANCE_LOW)
                .setName(context.getString(R.string.share_service_channel))
                .setShowBadge(false)
                .setVibrationEnabled(false)
                .setSound(null, null)
                .build()
            NotificationManagerCompat.from(context).createNotificationChannel(channel)
        }

        private fun notification(context: Context, title: String, text: String): Notification {
            val open = PendingIntent.getActivity(
                context, 0, Intent(context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
                PendingIntent.FLAG_IMMUTABLE,
            )
            val stop = PendingIntent.getService(
                context, 1, Intent(context, ShareService::class.java).setAction(ACTION_STOP),
                PendingIntent.FLAG_IMMUTABLE,
            )
            return NotificationCompat.Builder(context, CHANNEL_ID)
                .setSmallIcon(R.drawable.ic_stat_homefi)
                .setContentTitle(title.ifEmpty { context.getString(R.string.share_service_title) })
                .setContentText(text)
                .setPriority(NotificationCompat.PRIORITY_LOW)
                .setCategory(NotificationCompat.CATEGORY_SERVICE)
                .setSilent(true)
                .setOngoing(true)
                .setOnlyAlertOnce(true)
                .setShowWhen(false)
                .setContentIntent(open)
                .addAction(0, context.getString(R.string.share_service_stop), stop)
                .build()
        }
    }
}
