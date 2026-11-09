package com.perseusoft.homefi.player.exercise

import android.content.Context
import android.net.wifi.WifiManager
import android.util.Log
import com.perseusoft.homefi.core.ExerciseRide
import com.perseusoft.homefi.core.ExerciseTv
import com.perseusoft.homefi.core.SongLyrics
import com.perseusoft.homefi.core.TvSession
import com.perseusoft.homefi.data.userMessage

/**
 * The ride's TV server, living inside the foreground playback service so it keeps serving with the
 * screen off: the page and `/ride.json` over HTTP, plus the mDNS announcement for HomeFi's TV app.
 * Announcing and answering mDNS needs a Wi-Fi multicast lock for as long as the server runs.
 */
class RideTvHost(context: Context) {
    private val tv = ExerciseTv()
    private val lock: WifiManager.MulticastLock? = (context.applicationContext.getSystemService(Context.WIFI_SERVICE) as? WifiManager)
        ?.createMulticastLock(LOCK_TAG)
        ?.apply { setReferenceCounted(false) }

    /** Starts serving on [port] (the next free one if it is busy). Returns the URL, or the reason it failed. */
    suspend fun start(port: Int, name: String, language: String): Result<String> {
        runCatching { lock?.acquire() }.onFailure { Log.w(TAG, "no multicast lock", it) }
        return runCatching { tv.start(port.toUShort(), name, true, language) }.also {
            if (it.isFailure) releaseLock()
        }.recoverCatching { throw IllegalStateException(it.userMessage(), it) }
    }

    fun begin(session: TvSession) = tv.begin(session)

    val running: Boolean get() = tv.isRunning()

    /** Lyrics of the song [songKey] for the TV (null clears them); [offsetMs] positive = later. */
    fun setLyrics(songKey: String, lyrics: SongLyrics?, offsetMs: Int, durationMs: Long) {
        if (tv.isRunning()) tv.setLyrics(songKey, lyrics, offsetMs, durationMs.coerceAtLeast(0).toULong())
    }

    fun publish(ride: ExerciseRide, queueIndex: Int, positionMs: Long, nowMs: Long, playing: Boolean) {
        if (tv.isRunning()) tv.publish(ride, queueIndex.toUInt(), positionMs.toULong(), nowMs.toULong(), playing)
    }

    /** The ride is over: the page goes back to waiting, the server stays up for the next one. */
    fun clear() = tv.clear()

    suspend fun stop() {
        runCatching { tv.stop() }
        releaseLock()
    }

    private fun releaseLock() {
        runCatching { if (lock?.isHeld == true) lock.release() }
    }

    private companion object {
        const val TAG = "HomeFiRideTv"
        const val LOCK_TAG = "homefi-ride-tv"
    }
}
