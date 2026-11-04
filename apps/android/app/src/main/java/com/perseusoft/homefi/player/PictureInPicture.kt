package com.perseusoft.homefi.player

import android.app.PendingIntent
import android.app.PictureInPictureParams
import android.app.RemoteAction
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.graphics.Rect
import android.graphics.drawable.Icon
import android.os.Build
import android.util.Rational
import androidx.activity.ComponentActivity
import androidx.core.content.ContextCompat
import com.perseusoft.homefi.R
import com.perseusoft.homefi.data.AppSettings

/** What the picture-in-picture window needs to know; compared to skip identical updates. */
data class PipSpec(
    val enabled: Boolean,
    val playing: Boolean,
    val hasMedia: Boolean,
    val lyrics: Boolean,
    val mascot: Boolean,
    val sourceRect: Rect?,
) {
    /** Leaving the app now should float the player. */
    val armed: Boolean get() = enabled && playing && hasMedia

    /** Lyrics need a wide window; the cover and HomeFito sit well in a square. */
    val aspect: Rational get() = if (lyrics) WIDE else SQUARE

    companion object {
        private val WIDE = Rational(16, 9)
        private val SQUARE = Rational(1, 1)

        fun of(settings: AppSettings, state: PlayerUiState, sourceRect: Rect?) = PipSpec(
            enabled = settings.pipEnabled,
            playing = state.isPlaying,
            hasMedia = state.current != null,
            lyrics = settings.pipLyrics,
            mascot = settings.pipMascot,
            sourceRect = sourceRect,
        )
    }
}

/**
 * Picture-in-picture for [MainActivity]: the window's parameters (ratio, source rect,
 * auto-enter) and its previous / play-pause / next buttons, which come back as broadcasts
 * to a receiver that lives while the activity is started. Every call is a no-op where the
 * device has no picture-in-picture.
 */
class PictureInPicture(private val activity: ComponentActivity, private val player: PlayerController) {
    val supported: Boolean =
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.O &&
            activity.packageManager.hasSystemFeature(PackageManager.FEATURE_PICTURE_IN_PICTURE)

    private var last: PipSpec? = null
    private var registered = false
    private val receiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            when (intent.action) {
                ACTION_PREVIOUS -> player.previous()
                ACTION_TOGGLE -> player.togglePlayPause()
                ACTION_NEXT -> player.next()
            }
        }
    }

    /** Listen for the window's buttons (call from onStart). */
    fun start() {
        if (!supported || registered) return
        val filter = IntentFilter().apply {
            addAction(ACTION_PREVIOUS)
            addAction(ACTION_TOGGLE)
            addAction(ACTION_NEXT)
        }
        ContextCompat.registerReceiver(activity, receiver, filter, ContextCompat.RECEIVER_NOT_EXPORTED)
        registered = true
    }

    /** Stop listening (call from onStop). */
    fun stop() {
        if (!registered) return
        activity.unregisterReceiver(receiver)
        registered = false
    }

    /** Hand the system the current parameters, so the next home gesture floats the player. */
    fun update(spec: PipSpec) {
        if (!supported || spec == last) return
        last = spec
        runCatching { activity.setPictureInPictureParams(build(spec)) }
    }

    /** Float the player now, if [spec] says leaving should (Android 8-11: the system doesn't do it by itself). */
    fun enterIfArmed(spec: PipSpec) {
        if (!supported || !spec.armed || Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) return
        runCatching { activity.enterPictureInPictureMode(build(spec)) }
    }

    private fun build(spec: PipSpec): PictureInPictureParams {
        val builder = PictureInPictureParams.Builder()
            .setAspectRatio(spec.aspect)
            .setActions(actions(spec))
        spec.sourceRect?.let(builder::setSourceRectHint)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            builder.setAutoEnterEnabled(spec.armed).setSeamlessResizeEnabled(true)
        }
        return builder.build()
    }

    private fun actions(spec: PipSpec): List<RemoteAction> = listOf(
        action(android.R.drawable.ic_media_previous, R.string.pip_previous, ACTION_PREVIOUS, REQUEST_PREVIOUS),
        if (spec.playing) {
            action(android.R.drawable.ic_media_pause, R.string.pip_pause, ACTION_TOGGLE, REQUEST_TOGGLE)
        } else {
            action(android.R.drawable.ic_media_play, R.string.pip_play, ACTION_TOGGLE, REQUEST_TOGGLE)
        },
        action(android.R.drawable.ic_media_next, R.string.pip_next, ACTION_NEXT, REQUEST_NEXT),
    )

    private fun action(icon: Int, label: Int, action: String, request: Int): RemoteAction {
        val text = activity.getString(label)
        val intent = PendingIntent.getBroadcast(
            activity,
            request,
            Intent(action).setPackage(activity.packageName),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        return RemoteAction(Icon.createWithResource(activity, icon), text, text, intent)
    }

    private companion object {
        const val ACTION_PREVIOUS = "com.perseusoft.homefi.PIP_PREVIOUS"
        const val ACTION_TOGGLE = "com.perseusoft.homefi.PIP_TOGGLE"
        const val ACTION_NEXT = "com.perseusoft.homefi.PIP_NEXT"
        const val REQUEST_PREVIOUS = 1
        const val REQUEST_TOGGLE = 2
        const val REQUEST_NEXT = 3
    }
}
