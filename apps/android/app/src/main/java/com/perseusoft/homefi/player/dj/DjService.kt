package com.perseusoft.homefi.player.dj

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
import com.perseusoft.homefi.data.AppEntry
import com.perseusoft.homefi.R

/**
 * Keeps the process (and the two decks' players) alive while DJ mode is open, so a mix does
 * not stop when the screen turns off. Discreet like the other services: low importance, silent.
 */
class DjService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) {
            val callback = onStopRequested
            Handler(Looper.getMainLooper()).post { callback?.invoke() }
            shutdown()
            return START_NOT_STICKY
        }
        if (!goForeground()) shutdown()
        return START_NOT_STICKY
    }

    private fun goForeground(): Boolean {
        createChannel(this)
        val type = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK else 0
        return try {
            ServiceCompat.startForeground(this, NOTIFICATION_ID, notification(this), type)
            true
        } catch (e: RuntimeException) {
            // started from the background (API 31+) or the type's prerequisites are missing
            Log.w(TAG, "could not start in the foreground", e)
            false
        }
    }

    private fun shutdown() {
        ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    companion object {
        private const val TAG = "HomeFiDjService"
        private const val CHANNEL_ID = "dj"
        private const val NOTIFICATION_ID = 4
        private const val ACTION_STOP = "com.perseusoft.homefi.dj.STOP"

        /** Called on the main thread when "Close" is tapped in the notification. */
        @Volatile
        var onStopRequested: (() -> Unit)? = null

        /** Must be called while the app is in front (the DJ screen opening). */
        fun start(context: Context) {
            try {
                ContextCompat.startForegroundService(context, Intent(context, DjService::class.java))
            } catch (e: IllegalStateException) {
                Log.w(TAG, "could not start", e)
            }
        }

        fun stop(context: Context) {
            context.stopService(Intent(context, DjService::class.java))
        }

        private fun createChannel(context: Context) {
            val channel = NotificationChannelCompat.Builder(CHANNEL_ID, NotificationManagerCompat.IMPORTANCE_LOW)
                .setName(context.getString(R.string.dj_service_channel))
                .setShowBadge(false)
                .setVibrationEnabled(false)
                .setSound(null, null)
                .build()
            NotificationManagerCompat.from(context).createNotificationChannel(channel)
        }

        private fun notification(context: Context): Notification {
            val open = PendingIntent.getActivity(
                context, 0, AppEntry.intent(context).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
                PendingIntent.FLAG_IMMUTABLE,
            )
            val stop = PendingIntent.getService(
                context, 1, Intent(context, DjService::class.java).setAction(ACTION_STOP),
                PendingIntent.FLAG_IMMUTABLE,
            )
            return NotificationCompat.Builder(context, CHANNEL_ID)
                .setSmallIcon(R.drawable.ic_stat_homefi)
                .setContentTitle(context.getString(R.string.dj_service_title))
                .setContentText(context.getString(R.string.dj_service_text))
                .setPriority(NotificationCompat.PRIORITY_LOW)
                .setCategory(NotificationCompat.CATEGORY_SERVICE)
                .setSilent(true)
                .setOngoing(true)
                .setOnlyAlertOnce(true)
                .setShowWhen(false)
                .setContentIntent(open)
                .addAction(0, context.getString(R.string.dj_service_stop), stop)
                .build()
        }
    }
}
