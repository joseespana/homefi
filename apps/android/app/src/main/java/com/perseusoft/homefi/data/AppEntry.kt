package com.perseusoft.homefi.data

import android.app.Activity
import android.content.Context
import android.content.Intent
import com.perseusoft.homefi.MainActivity
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.tv.TvActivity

/** The one place that knows which activity is the app's front door on each [DeviceForm]. */
object AppEntry {
    /** Extra on an entry intent: open straight on the full player (set by the media notification). */
    const val EXTRA_OPEN_PLAYER = "com.perseusoft.homefi.OPEN_PLAYER"

    fun activityFor(form: DeviceForm): Class<out Activity> = when (form) {
        DeviceForm.PHONE -> MainActivity::class.java
        DeviceForm.TV -> TvActivity::class.java
    }

    /** An intent for the current form's activity (a notification's tap, a forward). */
    fun intent(context: Context): Intent = Intent(context, activityFor(context.homefi.deviceForm))
}
