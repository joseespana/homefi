package com.perseusoft.homefi.tv

import android.view.KeyEvent
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update

// What the remote's keys do. Pure functions and one small state holder, so the player's key map
// (docs/TV_UI.md section 6.5) is unit-tested without a screen.

/** What a key does in the full-screen player. */
enum class PlayerKeyAction {
    /** Nothing of ours: the focus system (or the system) handles it. */
    NONE,
    PLAY_PAUSE,

    /** LEFT / RIGHT with the controls hidden: a tap skips, a hold seeks (see [holdSeekStep]). */
    LEFT,
    RIGHT,
    CYCLE_PANEL,
    SHOW_CONTROLS,
    SEEK_FORWARD,
    SEEK_BACK,
}

/** Key map of the player: [controlsVisible] decides whether the arrows and OK belong to the focus system. */
fun playerKeyAction(keyCode: Int, controlsVisible: Boolean): PlayerKeyAction = when (keyCode) {
    KeyEvent.KEYCODE_MEDIA_FAST_FORWARD -> PlayerKeyAction.SEEK_FORWARD
    KeyEvent.KEYCODE_MEDIA_REWIND -> PlayerKeyAction.SEEK_BACK
    KeyEvent.KEYCODE_DPAD_CENTER, KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_NUMPAD_ENTER ->
        if (controlsVisible) PlayerKeyAction.NONE else PlayerKeyAction.PLAY_PAUSE
    KeyEvent.KEYCODE_DPAD_LEFT -> if (controlsVisible) PlayerKeyAction.NONE else PlayerKeyAction.LEFT
    KeyEvent.KEYCODE_DPAD_RIGHT -> if (controlsVisible) PlayerKeyAction.NONE else PlayerKeyAction.RIGHT
    KeyEvent.KEYCODE_DPAD_UP -> if (controlsVisible) PlayerKeyAction.NONE else PlayerKeyAction.CYCLE_PANEL
    KeyEvent.KEYCODE_DPAD_DOWN -> if (controlsVisible) PlayerKeyAction.NONE else PlayerKeyAction.SHOW_CONTROLS
    else -> PlayerKeyAction.NONE
}

/** The system repeats a held key about every 50 ms after 500 ms: seek on the first repeat, then every [HOLD_SEEK_EVERY]. */
const val HOLD_SEEK_EVERY = 5

/** True on the repeats of a held arrow that should seek once more ([repeatCount] counts from 0 on the first press). */
fun holdSeekStep(repeatCount: Int): Boolean = repeatCount >= 1 && (repeatCount - 1) % HOLD_SEEK_EVERY == 0

/** [positionMs] moved by [seconds] (negative = back), kept inside the song. */
fun seekTarget(positionMs: Long, durationMs: Long, seconds: Int): Long {
    val moved = positionMs + seconds * MS_PER_SECOND
    return if (durationMs > 0) moved.coerceIn(0, durationMs) else moved.coerceAtLeast(0)
}

private const val MS_PER_SECOND = 1_000L

/**
 * Ambient mode's wake-up: while the screensaver shows, the first key only wakes (it is not
 * forwarded as an action, nor is its release); any key restarts the idle timer.
 */
class TvIdle {
    private val _ambient = MutableStateFlow(false)
    val ambient: StateFlow<Boolean> = _ambient.asStateFlow()

    private val _activity = MutableStateFlow(0)

    /** Counts key presses: the idle timer starts over whenever it changes. */
    val activity: StateFlow<Int> = _activity.asStateFlow()

    private var swallowing = HashSet<Int>()

    fun enterAmbient() {
        _ambient.value = true
    }

    /** Playback stopped or the screen left: ambient mode ends without a key. */
    fun leaveAmbient() {
        _ambient.value = false
    }

    /** Call for every key event; true = it only woke the screen, swallow it. */
    fun onKey(keyCode: Int, isDown: Boolean): Boolean {
        if (isDown) _activity.update { it + 1 }
        if (isDown && _ambient.value) {
            _ambient.value = false
            swallowing.add(keyCode)
            return true
        }
        // the release of the key that woke it
        return !isDown && swallowing.remove(keyCode)
    }
}
