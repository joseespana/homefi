package com.perseusoft.homefi.player

import androidx.media3.common.FlagSet
import androidx.media3.common.ForwardingPlayer
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import com.perseusoft.homefi.HomefiApplication
import com.perseusoft.homefi.lyrics.LyricsState
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import java.util.concurrent.CopyOnWriteArraySet

/**
 * Lyrics in the car. A car only shows its own player, so while Android Auto is connected
 * (and Settings → Playback → Android Auto → Lyrics is on) the line being sung replaces the
 * artist in what the session reports: the car's player shows it under the title, line by
 * line. Off the car, the metadata is the song's own.
 *
 * One per deck: the session follows the deck that plays (crossfade).
 */
class CarLyricsPlayer(
    base: Player,
    private val app: HomefiApplication,
    private val scope: CoroutineScope,
) : ForwardingPlayer(base) {
    private val listeners = CopyOnWriteArraySet<Player.Listener>()
    private var line: String? = null
    private var ticker: Job? = null

    /** Android Auto is connected: show the lyrics (when the setting allows). */
    var carConnected: Boolean = false
        set(value) {
            field = value
            refresh()
        }

    override fun addListener(listener: Player.Listener) {
        listeners += listener
        super.addListener(listener)
    }

    override fun removeListener(listener: Player.Listener) {
        listeners -= listener
        super.removeListener(listener)
    }

    override fun getMediaMetadata(): MediaMetadata {
        val own = super.getMediaMetadata()
        val sung = line ?: return own
        val artist = own.artist?.toString().orEmpty()
        // the line first; the artist stays in the subtitle for cars that show both
        return own.buildUpon().setArtist("♪ $sung").setSubtitle(artist.ifBlank { null }).build()
    }

    /** Starts or stops following the lyrics as the car connects or the setting changes. */
    fun refresh() {
        val on = carConnected && app.settings.current.let { it.carEnabled && it.carLyrics }
        if (on && ticker == null) {
            ticker = scope.launch {
                while (isActive) {
                    update(currentLine())
                    delay(TICK_MS)
                }
            }
        } else if (!on) {
            ticker?.cancel()
            ticker = null
            update(null)
        }
    }

    /** The line playing now, or null (no synced lyrics, an instrumental part, nothing playing). */
    private fun currentLine(): String? {
        val id = currentMediaItem?.mediaId?.takeIf { it.isNotBlank() } ?: return null
        val state = app.lyrics.states.value[id]
        if (state == null) {
            app.lyrics.request(id)
            return null
        }
        val lyrics = (state as? LyricsState.Ready)?.lyrics ?: return null
        val index = lyrics.lineAt(currentPosition + app.settings.current.lyricsOffsetMs)
        return lyrics.lines.getOrNull(index)?.text?.takeIf { it.isNotBlank() }
    }

    private fun update(next: String?) {
        if (next == line) return
        line = next
        val metadata = mediaMetadata
        val events = Player.Events(FlagSet.Builder().add(Player.EVENT_MEDIA_METADATA_CHANGED).build())
        listeners.forEach {
            it.onMediaMetadataChanged(metadata)
            it.onEvents(this, events)
        }
    }

    /** Stops following the lyrics (the service is going away); the player itself is released by its owner. */
    fun stopLyrics() {
        ticker?.cancel()
        ticker = null
    }

    private companion object {
        /** Often enough that a line changes on time, rarely enough to cost nothing. */
        const val TICK_MS = 250L
    }
}

/** Android Auto's package: the controller that is the car. */
const val ANDROID_AUTO_PACKAGE = "com.google.android.projection.gearhead"
