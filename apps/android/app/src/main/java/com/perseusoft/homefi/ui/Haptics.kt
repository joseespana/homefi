package com.perseusoft.homefi.ui

import android.os.Build
import android.view.HapticFeedbackConstants
import android.view.View
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.platform.LocalView

/**
 * A little buzz for the buttons worth feeling: play, skip, seek, picking an option,
 * downloading. Off with the haptics setting; Android's own "touch feedback" setting
 * also silences it, and a player without a motor (the SR35) simply ignores it.
 */
@Stable
class Haptics(private val view: View, private val enabled: () -> Boolean) {
    /** A main button: play / pause, next, previous. */
    fun press() = perform(HapticFeedbackConstants.VIRTUAL_KEY)

    /** A small step: a seek, a chip, a tab, a lyrics line. */
    fun tick() = perform(HapticFeedbackConstants.CLOCK_TICK)

    /** A switch or a mode (shuffle, repeat) turning on or off. */
    fun toggle(on: Boolean) = perform(
        when {
            Build.VERSION.SDK_INT >= 34 -> if (on) HapticFeedbackConstants.TOGGLE_ON else HapticFeedbackConstants.TOGGLE_OFF
            else -> HapticFeedbackConstants.CLOCK_TICK
        },
    )

    /** Something was done for good: a download started, songs queued, a session joined. */
    fun confirm() = perform(if (Build.VERSION.SDK_INT >= 30) HapticFeedbackConstants.CONFIRM else HapticFeedbackConstants.VIRTUAL_KEY)

    /** A long press opened a menu. */
    fun longPress() = perform(HapticFeedbackConstants.LONG_PRESS)

    private fun perform(type: Int) {
        if (enabled()) view.performHapticFeedback(type)
    }
}

/** Null outside the app's root (previews): nothing buzzes. */
val LocalHaptics = staticCompositionLocalOf<Haptics?> { null }

/** The app's haptics, or one that never buzzes. */
@Composable
fun haptics(): Haptics {
    val view = LocalView.current
    return LocalHaptics.current ?: remember(view) { Haptics(view) { false } }
}
