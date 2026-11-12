package com.perseusoft.homefi.player.exercise

/** Fades the ride's next song in at a part's change (the playback service's [com.perseusoft.homefi.player.Crossfader] does it). Main thread. */
interface RideFader {
    /**
     * Turns the ride's own fade on ([seconds] > 0, builds the second deck when the phone can afford it) or off (0).
     * Returns whether fades are possible; false means every change is a cut.
     */
    fun setRideFade(seconds: Float): Boolean

    /** Buffers queue item [index] on the second deck, paused. True once it is ready to fade into. */
    fun prepare(index: Int): Boolean

    /** Starts fading into [index] over [lengthMs]. False when it can't (not prepared, not ready): the caller cuts. */
    fun fadeTo(index: Int, lengthMs: Long): Boolean
}

/** A fade shorter than this is a cut anyway. */
const val MIN_RIDE_FADE_MS = 500L

/** The levels reach the audio a buffer ahead of what is heard: the fade ends this much before the file does. */
const val RIDE_FADE_GUARD_MS = 500L

/** The next song is buffered on the second deck this long before the change. */
const val RIDE_FADE_PREPARE_MS = 6_000L

/**
 * How long the fade into the next song lasts: the setting ([seconds]), shortened so it ends before the
 * playing song's file does ([durationMs] unknown or <= 0 gives 0). Under [MIN_RIDE_FADE_MS] it is 0: cut.
 * [positionMs] is where the song is when its part ends.
 */
fun rideFadeMs(seconds: Float, durationMs: Long, positionMs: Long): Long {
    if (!seconds.isFinite() || seconds <= 0f || durationMs <= 0L) return 0L
    val wanted = (seconds * 1000f).toLong()
    val room = durationMs - positionMs.coerceAtLeast(0L) - RIDE_FADE_GUARD_MS
    val length = minOf(wanted, room)
    return if (length >= MIN_RIDE_FADE_MS) length else 0L
}
