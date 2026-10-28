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

    override fun onCreate() {
        super.onCreate()
        // platform decoders first; FFmpeg covers what Android lacks (ALAC, AC-3, DTS, TrueHD)
        val renderers = object : DefaultRenderersFactory(this) {
            // the tee feeds the player's visualizer, VU meters and mascot
            override fun buildAudioSink(context: Context, enableFloatOutput: Boolean, enableAudioTrackPlaybackParams: Boolean): AudioSink =
                DefaultAudioSink.Builder(context)
                    .setEnableFloatOutput(enableFloatOutput)
                    .setEnableAudioTrackPlaybackParams(enableAudioTrackPlaybackParams)
                    .setAudioProcessors(arrayOf(TeeAudioProcessor(AudioTap)))
                    .build()
        }.setExtensionRendererMode(DefaultRenderersFactory.EXTENSION_RENDERER_MODE_ON)
        val streams = TidalStreams(this, scope)
        val sources = DefaultMediaSourceFactory(ResolvingDataSource.Factory(DefaultDataSource.Factory(this), streams))
        val player = ExoPlayer.Builder(this, renderers)
            .setMediaSourceFactory(sources)
            .setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(C.USAGE_MEDIA)
                    .setContentType(C.AUDIO_CONTENT_TYPE_MUSIC)
                    .build(),
                /* handleAudioFocus = */ true,
            )
            .setHandleAudioBecomingNoisy(true) // pause when headphones are unplugged
            .setWakeMode(C.WAKE_MODE_LOCAL)
            .build()
        player.addListener(SkipUndecodable(player))
        player.addListener(LastQueue.Saver(this, player))
        player.addListener(object : Player.Listener {
            override fun onIsPlayingChanged(isPlaying: Boolean) {
                if (!isPlaying) AudioTap.reset()
            }

            override fun onMediaItemTransition(mediaItem: MediaItem?, reason: Int) {
                streams.prefetch(player)
            }
        })

        // route to the output picked in the app (USB DAC, jack, Bluetooth, ...)
        scope.launch {
            homefi.outputs.preferredDevice.collect { player.setPreferredAudioDevice(it) }
        }

        val open = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java).putExtra(MainActivity.EXTRA_OPEN_PLAYER, true),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        session = MediaSession.Builder(this, player)
            .setSessionActivity(open)
            .setCallback(Callback(this))
            .build()
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    override fun onTaskRemoved(rootIntent: Intent?) {
        val player = session?.player
        if (player == null || !player.playWhenReady || player.mediaItemCount == 0) stopSelf()
    }

    override fun onDestroy() {
        scope.cancel()
        session?.run {
            player.release()
            release()
        }
        session = null
        super.onDestroy()
    }

    /**
     * The file has audio but no decoder can handle it, not even FFmpeg
     * (very exotic codecs). ExoPlayer would "play" silence, so tell the user
     * and move on. Checked here, on the real player, because controllers can
     * briefly see the previous item's tracks after a transition.
     */
    private inner class SkipUndecodable(private val player: Player) : Player.Listener {
        override fun onTracksChanged(tracks: Tracks) {
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

/** Persists the queue (local files only) so playback can be resumed later. */
private object LastQueue {
    private const val PREFS = "last_queue"
    private const val MAX_ITEMS = 500

    class Saver(private val context: Context, private val player: Player) : Player.Listener {
        override fun onEvents(player: Player, events: Player.Events) {
            if (events.containsAny(Player.EVENT_MEDIA_ITEM_TRANSITION, Player.EVENT_TIMELINE_CHANGED, Player.EVENT_IS_PLAYING_CHANGED)) {
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
