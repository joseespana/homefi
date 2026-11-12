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
import androidx.media3.session.LibraryResult
import androidx.media3.session.MediaLibraryService
import androidx.media3.session.MediaSession
import com.google.common.collect.ImmutableList
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import com.google.common.util.concurrent.SettableFuture
import kotlinx.coroutines.CoroutineScope
import com.perseusoft.homefi.data.AppEntry
import com.perseusoft.homefi.R
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.exercise.RideTvHost
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import java.io.File

/**
 * Background playback: ExoPlayer inside a media library session, so the system shows
 * the media notification, lock-screen / quick-settings controls and routes
 * headset, Bluetooth and keyboard media buttons here. Android Auto browses and searches
 * the music through the same session ([CarLibrary]).
 */
class PlaybackService : MediaLibraryService() {
    private var session: MediaLibrarySession? = null
    private val scope = MainScope()

    private var crossfader: Crossfader? = null
    /** Exercise mode's TV server: lives with this foreground service so it keeps serving with the screen off. */
    private var rideTv: RideTvHost? = null
    /** The decks as the session sees them: with the lyric line while a car is connected. */
    private var carDecks: List<CarLyricsPlayer> = emptyList()
    /** Android Auto controllers connected now. */
    private val cars = mutableSetOf<MediaSession.ControllerInfo>()

    override fun onCreate() {
        super.onCreate()
        // cheap: the TV server itself is only created when a ride sends to the TV
        rideTv = RideTvHost(this).also(homefi.ride::attachTv)
        val streams = TidalStreams(this, scope)
        val attributes = AudioAttributes.Builder()
            .setUsage(C.USAGE_MEDIA)
            .setContentType(C.AUDIO_CONTENT_TYPE_MUSIC)
            .build()
        // deck 0 now; deck 1 (the next song fading in) only once crossfade is on, a second ExoPlayer costs
        // ~100 ms of start-up and memory that gapless playback never uses
        val first = buildPlayer(0, streams, attributes, handleFocus = true)
        val fader = Crossfader(
            this, first, { buildPlayer(1, streams, attributes, handleFocus = false) }, { wire(it, 1, streams) },
            attributes, homefi.settings.crossfade, homefi.sound, streams, scope,
        ) { from, to ->
            // the incoming deck is the session's from now on: notification, lock screen, the app's queue
            carDecks.getOrNull(to)?.let { session?.player = it }
            crossfader?.players?.getOrNull(to)?.let { player ->
                streams.prefetch(player)
                homefi.sound.onTrack(player)
            }
        }
        crossfader = fader
        // exercise mode fades its songs through this crossfader (its own length, whatever the global crossfade says)
        homefi.ride.attachFader(fader)
        wire(first, 0, streams)
        carDecks = fader.decks.map { CarLyricsPlayer(it, homefi, scope) }
        // the second deck is built when crossfade is turned on (and at start when it already is)
        scope.launch {
            homefi.settings.crossfade.collect { if (fader.wantsSpare(it)) fader.ensureSpare() }
        }

        // route to the output picked in the app (USB DAC, jack, Bluetooth, ...)
        scope.launch {
            homefi.outputs.preferredDevice.collect { fader.setPreferredAudioDevice(it) }
        }

        val open = PendingIntent.getActivity(
            this,
            0,
            AppEntry.intent(this).putExtra(AppEntry.EXTRA_OPEN_PLAYER, true),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        // the lyrics in the car follow the setting
        scope.launch { homefi.settings.settings.collect { carDecks.forEach(CarLyricsPlayer::refresh) } }
        session = MediaLibrarySession.Builder(this, carDecks[0], Callback(this, CarLibrary(homefi), scope, ::carConnected))
            .setSessionActivity(open)
            .build()
    }

    /** The listeners every deck carries (the active one acts, the other stays quiet). */
    private fun wire(player: ExoPlayer, deck: Int, streams: TidalStreams) {
        val fader = crossfader ?: return
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
                if (!isActive()) return
                homefi.sound.onTrack(player)
                // a song swapped into the queue (the ride's "change this song") is fetched ahead like the others
                if (reason == Player.TIMELINE_CHANGE_REASON_PLAYLIST_CHANGED) streams.prefetch(player)
            }
        })
        // a deck built later joins the session wrappers (car lyric line, lyrics refresh)
        if (deck > 0) {
            val wrapper = fader.decks.getOrNull(deck) ?: return
            carDecks = carDecks + CarLyricsPlayer(wrapper, homefi, scope).also { it.carConnected = cars.isNotEmpty() }
        }
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

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaLibrarySession? = session

    /** Android Auto connected ([connected]) or left: the car shows the lyric line meanwhile. */
    private fun carConnected(controller: MediaSession.ControllerInfo, connected: Boolean) {
        if (controller.packageName != ANDROID_AUTO_PACKAGE) return
        if (connected) cars += controller else cars -= controller
        carDecks.forEach { it.carConnected = cars.isNotEmpty() }
    }

    override fun onTaskRemoved(rootIntent: Intent?) {
        val player = session?.player
        if (player == null || !player.playWhenReady || player.mediaItemCount == 0) stopSelf()
    }

    override fun onDestroy() {
        rideTv?.let { host ->
            homefi.ride.detachTv(host)
            homefi.appScope.launch { host.stop() }
        }
        rideTv = null
        crossfader?.let(homefi.ride::detachFader)
        scope.cancel()
        session?.release()
        carDecks.forEach(CarLyricsPlayer::stopLyrics)
        carDecks = emptyList()
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

    private class Callback(
        private val context: Context,
        private val car: CarLibrary,
        private val scope: CoroutineScope,
        private val onCar: (MediaSession.ControllerInfo, Boolean) -> Unit,
    ) : MediaLibrarySession.Callback {
        override fun onConnect(session: MediaSession, controller: MediaSession.ControllerInfo): MediaSession.ConnectionResult {
            onCar(controller, true)
            return super.onConnect(session, controller)
        }

        override fun onDisconnected(session: MediaSession, controller: MediaSession.ControllerInfo) {
            onCar(controller, false)
        }

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

        /**
         * A car picked something: a song inside a list plays the whole list from it; the app's
         * own controller sends full items, which play as they are.
         */
        override fun onSetMediaItems(
            mediaSession: MediaSession,
            controller: MediaSession.ControllerInfo,
            mediaItems: MutableList<MediaItem>,
            startIndex: Int,
            startPositionMs: Long,
        ): ListenableFuture<MediaSession.MediaItemsWithStartPosition> {
            if (mediaItems.all { it.localConfiguration != null || it.requestMetadata.mediaUri != null }) {
                return super.onSetMediaItems(mediaSession, controller, mediaItems, startIndex, startPositionMs)
            }
            return future {
                val (items, start) = car.expand(mediaItems) ?: error("nothing to play")
                MediaSession.MediaItemsWithStartPosition(items, start, startPositionMs)
            }
        }

        override fun onGetLibraryRoot(
            session: MediaLibrarySession,
            browser: MediaSession.ControllerInfo,
            params: LibraryParams?,
        ): ListenableFuture<LibraryResult<MediaItem>> = Futures.immediateFuture(LibraryResult.ofItem(car.root(), params))

        override fun onGetChildren(
            session: MediaLibrarySession,
            browser: MediaSession.ControllerInfo,
            parentId: String,
            page: Int,
            pageSize: Int,
            params: LibraryParams?,
        ): ListenableFuture<LibraryResult<ImmutableList<MediaItem>>> = future {
            val children = car.children(parentId)
            if (children == null) LibraryResult.ofError(LibraryResult.RESULT_ERROR_BAD_VALUE)
            else LibraryResult.ofItemList(children.page(page, pageSize), params)
        }

        override fun onGetItem(
            session: MediaLibrarySession,
            browser: MediaSession.ControllerInfo,
            mediaId: String,
        ): ListenableFuture<LibraryResult<MediaItem>> = future {
            val item = if (mediaId == CarLibrary.ROOT) car.root() else car.expand(listOf(MediaItem.Builder().setMediaId(mediaId).build()))?.let { (items, start) -> items[start] }
            if (item == null) LibraryResult.ofError(LibraryResult.RESULT_ERROR_BAD_VALUE) else LibraryResult.ofItem(item, null)
        }

        /** "Play … on HomeFi": the results are fetched by [onGetSearchResult]. */
        override fun onSearch(
            session: MediaLibrarySession,
            browser: MediaSession.ControllerInfo,
            query: String,
            params: LibraryParams?,
        ): ListenableFuture<LibraryResult<Void>> = future {
            session.notifySearchResultChanged(browser, query, car.search(query).size, params)
            LibraryResult.ofVoid()
        }

        override fun onGetSearchResult(
            session: MediaLibrarySession,
            browser: MediaSession.ControllerInfo,
            query: String,
            page: Int,
            pageSize: Int,
            params: LibraryParams?,
        ): ListenableFuture<LibraryResult<ImmutableList<MediaItem>>> = future {
            LibraryResult.ofItemList(car.search(query).page(page, pageSize), params)
        }

        /** Runs [block] on the service's scope and hands its result (or failure) to Media3. */
        private fun <T> future(block: suspend () -> T): ListenableFuture<T> {
            val result = SettableFuture.create<T>()
            scope.launch {
                runCatching { block() }.onSuccess { result.set(it) }.onFailure { result.setException(it) }
            }
            return result
        }

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

/** One page of a list a car asked for (Media3 pages from 0). */
private fun <T> List<T>.page(page: Int, pageSize: Int): List<T> {
    if (pageSize <= 0 || pageSize == Int.MAX_VALUE) return this
    val from = page.coerceAtLeast(0).toLong() * pageSize
    return if (from >= size) emptyList() else subList(from.toInt(), minOf(size, (from + pageSize).toInt()))
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
