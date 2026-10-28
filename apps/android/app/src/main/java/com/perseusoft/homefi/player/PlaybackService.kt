package com.perseusoft.homefi.player

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.widget.Toast
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.Player
import androidx.media3.common.Tracks
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.datasource.ResolvingDataSource
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.exoplayer.audio.TeeAudioProcessor
import androidx.media3.exoplayer.audio.DefaultAudioSink
import androidx.media3.exoplayer.audio.AudioSink
import androidx.media3.exoplayer.audio.SilenceSkippingAudioProcessor
import androidx.media3.common.audio.SonicAudioProcessor
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import com.perseusoft.homefi.MainActivity
import com.perseusoft.homefi.R
import com.perseusoft.homefi.homefi
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import java.io.File

/**
 * Background playback: ExoPlayer inside a MediaSession, so the system shows
 * the media notification, lock-screen / quick-settings controls and routes
 * headset, Bluetooth and keyboard media buttons here.
 */
class PlaybackService : MediaSessionService() {
    private var session: MediaSession? = null
    private val scope = MainScope()

    private var crossfader: Crossfader? = null

    override fun onCreate() {
        super.onCreate()
        val streams = TidalStreams(this, scope)
        val attributes = AudioAttributes.Builder()
            .setUsage(C.USAGE_MEDIA)
            .setContentType(C.AUDIO_CONTENT_TYPE_MUSIC)
            .build()
        // two decks, so the next song can start while this one fades out (crossfade)
        val players = listOf(0, 1).map { deck -> buildPlayer(deck, streams, attributes, handleFocus = deck == 0) }
        val fader = Crossfader(this, players, attributes, homefi.settings.crossfade, homefi.sound, streams, scope) { from, to ->
            // the incoming deck is the session's from now on: notification, lock screen, the app's queue
            crossfader?.decks?.get(to)?.let { session?.player = it }
            val player = players[to]
            streams.prefetch(player)
            homefi.sound.onTrack(player)
        }
        crossfader = fader
        players.forEach { player ->
            val isActive = { fader.player === player }
            player.addListener(SkipUndecodable(player, isActive))
            player.addListener(LastQueue.Saver(this, player, isActive))
            player.addListener(object : Player.Listener {
                override fun onIsPlayingChanged(isPlaying: Boolean) {
                    if (isActive() && !isPlaying) AudioTap.reset()
                }

                override fun onMediaItemTransition(mediaItem: MediaItem?, reason: Int) {
                    if (!isActive()) return
                    streams.prefetch(player)
                    // taste layers and loudness follow the track at once (no round trip through a controller)
                    homefi.sound.onTrack(player)
                }

                override fun onShuffleModeEnabledChanged(shuffleModeEnabled: Boolean) {
                    if (isActive()) homefi.sound.onTrack(player)
                }

                override fun onTimelineChanged(timeline: androidx.media3.common.Timeline, reason: Int) {
                    if (isActive()) homefi.sound.onTrack(player)
                }
            })
        }

        // route to the output picked in the app (USB DAC, jack, Bluetooth, ...)
        scope.launch {
            homefi.outputs.preferredDevice.collect { fader.setPreferredAudioDevice(it) }
        }

        val open = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java).putExtra(MainActivity.EXTRA_OPEN_PLAYER, true),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        session = MediaSession.Builder(this, fader.decks[0])
            .setSessionActivity(open)
            .setCallback(Callback(this))
            .build()
    }

    /**
     * One deck: platform decoders first, FFmpeg for what Android lacks (ALAC, AC-3, DTS,
     * TrueHD); the equalizer (this deck's slot), then the tee that feeds the visualizer, VU
     * meters and mascot. Silence skipping (crossfade's smart trim, only switched on around a
     * fade) listens for -60 dBFS, not Media3's default -30 that would cut quiet intros.
     */
    private fun buildPlayer(deck: Int, streams: TidalStreams, attributes: AudioAttributes, handleFocus: Boolean): ExoPlayer {
        val renderers = object : DefaultRenderersFactory(this) {
            override fun buildAudioSink(context: Context, enableFloatOutput: Boolean, enableAudioTrackPlaybackParams: Boolean): AudioSink =
                DefaultAudioSink.Builder(context)
                    .setEnableFloatOutput(enableFloatOutput)
                    .setEnableAudioTrackPlaybackParams(enableAudioTrackPlaybackParams)
                    .setAudioProcessorChain(
                        DefaultAudioSink.DefaultAudioProcessorChain(
                            arrayOf(EqAudioProcessor { EqRuntime.decks[deck] }, TeeAudioProcessor(DeckTap(deck))),
                            SilenceSkippingAudioProcessor(SILENCE_MIN_US, 0f, SILENCE_KEEP_US, 0, SILENCE_LEVEL),
                            SonicAudioProcessor(),
                        ),
                    )
                    .build()
        }.setExtensionRendererMode(DefaultRenderersFactory.EXTENSION_RENDERER_MODE_ON)
        val sources = DefaultMediaSourceFactory(ResolvingDataSource.Factory(DefaultDataSource.Factory(this), streams))
        return ExoPlayer.Builder(this, renderers)
            .setMediaSourceFactory(sources)
            .setAudioAttributes(attributes, handleFocus)
            .setHandleAudioBecomingNoisy(true) // pause when headphones are unplugged
            .setWakeMode(C.WAKE_MODE_LOCAL)
            .build()
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    override fun onTaskRemoved(rootIntent: Intent?) {
        val player = session?.player
        if (player == null || !player.playWhenReady || player.mediaItemCount == 0) stopSelf()
    }

    override fun onDestroy() {
        scope.cancel()
        session?.release()
        crossfader?.release()
        crossfader = null
        session = null
        super.onDestroy()
    }

    /**
     * The file has audio but no decoder can handle it, not even FFmpeg
     * (very exotic codecs). ExoPlayer would "play" silence, so tell the user
     * and move on. Checked here, on the real player, because controllers can
     * briefly see the previous item's tracks after a transition.
     */
    private inner class SkipUndecodable(private val player: Player, private val isActive: () -> Boolean) : Player.Listener {
        override fun onTracksChanged(tracks: Tracks) {
            if (!isActive()) return
            val audio = tracks.groups.filter { it.type == C.TRACK_TYPE_AUDIO }
            if (audio.isEmpty() || audio.any { it.isSupported }) return
            val title = player.mediaMetadata.title ?: player.currentMediaItem?.mediaId?.substringAfterLast('/')
            Toast.makeText(this@PlaybackService, getString(R.string.player_skipped, title), Toast.LENGTH_LONG).show()
            if (player.hasNextMediaItem()) player.seekToNextMediaItem() else player.pause()
        }
    }

    private class Callback(private val context: Context) : MediaSession.Callback {
        /** Controllers send items without their local URI; restore it from the request metadata. */
        override fun onAddMediaItems(
            mediaSession: MediaSession,
            controller: MediaSession.ControllerInfo,
            mediaItems: MutableList<MediaItem>,
        ): ListenableFuture<MutableList<MediaItem>> = Futures.immediateFuture(
            mediaItems.map { item ->
                val uri = item.requestMetadata.mediaUri ?: Uri.parse(item.mediaId)
                item.buildUpon().setUri(uri).build()
            }.toMutableList()
        )

        /** "Play" on headphones / Bluetooth / the lock screen after the app was closed. */
        override fun onPlaybackResumption(
            mediaSession: MediaSession,
            controller: MediaSession.ControllerInfo,
        ): ListenableFuture<MediaSession.MediaItemsWithStartPosition> {
            val last = LastQueue.load(context)
                ?: return Futures.immediateFailedFuture(UnsupportedOperationException("nothing to resume"))
            return Futures.immediateFuture(last)
        }
    }
}

/** Silence skipping for the crossfade's smart trim: 200 ms below -60 dBFS (16-bit), all but 20 ms removed. */
private const val SILENCE_MIN_US = 200_000L
private const val SILENCE_KEEP_US = 20_000L
private const val SILENCE_LEVEL: Short = 33

/** Persists the queue (local files only) so playback can be resumed later. */
private object LastQueue {
    private const val PREFS = "last_queue"
    private const val MAX_ITEMS = 500

    class Saver(private val context: Context, private val player: Player, private val isActive: () -> Boolean) : Player.Listener {
        override fun onEvents(player: Player, events: Player.Events) {
            if (isActive() && events.containsAny(Player.EVENT_MEDIA_ITEM_TRANSITION, Player.EVENT_TIMELINE_CHANGED, Player.EVENT_IS_PLAYING_CHANGED)) {
                save()
            }
        }

        private fun save() {
            val ids = (0 until player.mediaItemCount).map { player.getMediaItemAt(it).mediaId }
            val files = ids.filter(::isFilePath).take(MAX_ITEMS)
            val index = files.indexOf(player.currentMediaItem?.mediaId).coerceAtLeast(0)
            context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit()
                .putString("ids", files.joinToString("\n"))
                .putInt("index", index)
                .putLong("position", player.currentPosition.coerceAtLeast(0))
                .apply()
        }
    }

    fun load(context: Context): MediaSession.MediaItemsWithStartPosition? {
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val ids = prefs.getString("ids", null)?.split("\n")?.filter { it.isNotBlank() && File(it).exists() }
        if (ids.isNullOrEmpty()) return null
        val library = context.homefi.library
        val items = ids.map { path -> library.find(path)?.toMediaItem() ?: fileMediaItem(path) }
        val index = prefs.getInt("index", 0).coerceIn(0, items.lastIndex)
        return MediaSession.MediaItemsWithStartPosition(items, index, prefs.getLong("position", 0))
    }
}
