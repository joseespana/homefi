package com.perseusoft.homefi.data

import android.app.UiModeManager
import android.content.Context
import android.content.pm.PackageManager
import android.content.res.Configuration

/** What HomeFi is running on: decides which UI (the touch app or the D-pad TV app) opens. */
enum class DeviceForm {
    PHONE,
    TV;

    /**
     * The activity that opened is the wrong front door for this form: a TV must not stay in the
     * touch `MainActivity`, a phone must never stay in `TvActivity` (it hands over and finishes).
     */
    fun mustHandOver(activityIsTv: Boolean): Boolean = (this == TV) != activityIsTv

    companion object {
        /**
         * The form for the two facts the system gives ([tvMode]: the UI mode is television;
         * [leanback]: the device declares the leanback feature) and the user's [choice].
         * A choice of PHONE or TV wins; AUTO follows the device. Pure, so it is tested.
         */
        fun of(tvMode: Boolean, leanback: Boolean, choice: UiForm): DeviceForm = when (choice) {
            UiForm.PHONE -> PHONE
            UiForm.TV -> TV
            UiForm.AUTO -> if (tvMode || leanback) TV else PHONE
        }

        /** Reads the device once; call it a single time (see `HomefiApplication.deviceForm`). */
        fun detect(context: Context, choice: UiForm): DeviceForm {
            val modes = context.getSystemService(Context.UI_MODE_SERVICE) as? UiModeManager
            val tvMode = modes?.currentModeType == Configuration.UI_MODE_TYPE_TELEVISION
            val leanback = context.packageManager.hasSystemFeature(PackageManager.FEATURE_LEANBACK)
            return of(tvMode, leanback, choice)
        }
    }
}
