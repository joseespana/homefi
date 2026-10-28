package com.perseusoft.homefi.share

import android.content.Context
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.util.Log
import androidx.annotation.OptIn
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.database.StandaloneDatabaseProvider
import androidx.media3.datasource.DataSpec
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.datasource.cache.CacheDataSource
import androidx.media3.datasource.cache.CacheWriter
import androidx.media3.datasource.cache.LeastRecentlyUsedCacheEvictor
import androidx.media3.datasource.cache.SimpleCache
import androidx.media3.exoplayer.DefaultLoadControl
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.audio.AudioSink
import androidx.media3.exoplayer.audio.DefaultAudioSink
import androidx.media3.exoplayer.audio.TeeAudioProcessor
import com.perseusoft.homefi.player.AudioTap
import com.perseusoft.homefi.player.EqAudioProcessor
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import com.perseusoft.homefi.core.ShareGuest
import com.perseusoft.homefi.core.ShareState
import com.perseusoft.homefi.core.ShareSyncAction
import com.perseusoft.homefi.core.shareSyncAction
import com.perseusoft.homefi.homefi
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import java.io.File
import kotlin.math.abs
import kotlin.math.roundToInt

/**
 * Party mode on a guest: plays the hub's song on this phone (its own speaker, Bluetooth...)
 * and keeps it on the hub's clock.
 *
 * - **Local first.** Everything goes through a cache on this phone: the song playing now is
 *   kept as it streams, and the next and previous songs are downloaded ahead, so a song
 *   change starts at once and a busy Wi-Fi doesn't cut the music. The cache is deleted on
 *   [release] (leaving the party).
 * - **Gentle sync.** The gap to the hub is the median of the last few measurements (one
 *   late clock sample or position report doesn't move it); small gaps are closed by playing
 *   up to 2 % faster or slower, changed at most every [RATE_HOLD_MS]; only a big, steady gap
 *   jumps. Right after starting or jumping, the position is left to settle first (Android's
 *   audio position is unreliable in the first few hundred ms).
 *
 * Separate from the media session player; main thread only.
 */
@OptIn(UnstableApi::class)
internal class PartyPlayer(private val context: Context) {
    private var exo: ExoPlayer? = null
    private var loaded: ULong? = null
    private var cache: SimpleCache? = null
    private var database: StandaloneDatabaseProvider? = null
    private var scope = newScope()

    /** Next / previous songs being downloaded, in that order. */
    private var prefetchUrls: List<String> = emptyList()
    private var prefetchJob: Job? = null
    @Volatile
    private var prefetchWriter: CacheWriter? = null

    /** How long a jump takes to be heard; learnt from the gap after each one. */
    private var seekLeadMs = 0L
    private var seeking = false
    private var settleUntil = 0L
    private val gaps = ArrayDeque<Long>()
    private var rateChangedAt = 0L

    val active: Boolean get() = exo != null

    /**
     * One sync step (call every few hundred ms). [delayMs] is this phone's speaker latency;
     * [syncBySpeed] off only jumps; [preload] downloads the next and previous songs.
     * Returns `true` when playback just started here.
     */
    fun tick(guest: ShareGuest, state: ShareState, delayMs: Int, syncBySpeed: Boolean = true, preload: Boolean = true): Boolean {
        val audio = state.audio ?: run {
            pause()
            return false
        }
        val created = exo == null
        val player = exo ?: build().also { exo = it }
        prefetch(if (preload) state.prefetch.map { guest.resolveUrl(it.url) } else emptyList(), keep = guest.resolveUrl(audio.url))
        val target = (guest.expectedPositionMs(state.positionMs, state.clockMs, state.playing).toLong() + delayMs).coerceAtLeast(0)

        if (loaded != audio.queueId) {
            cancelStart()
            loaded = audio.queueId
            val item = MediaItem.Builder().setUri(guest.resolveUrl(audio.url)).setMimeType(audio.mime).build()
            player.setPlaybackSpeed(1f)
            player.setMediaItem(item, target + seekLeadMs)
            player.prepare()
            jumped()
        }
        // a new song scheduled to start for everyone at once: wait at the start, say we're ready
        val hubNow = guest.hubNowMs()?.toLong()
        val startsIn = if (state.playing && hubNow != null) state.clockMs.toLong() - hubNow else 0L
        if (startsIn > 0) {
            player.playWhenReady = false
            player.setPlaybackSpeed(1f)
            if (player.playbackState == Player.STATE_READY) {
                if (abs(player.currentPosition - target) > LEAD_TOLERANCE_MS) player.seekTo(target)
                if (readyFor != audio.queueId) {
                    readyFor = audio.queueId
                    runCatching { guest.ready(audio.queueId) }
                }
                if (startsIn <= START_AHEAD_MS) startAt(startsIn, audio.queueId)
            }
            return created
        }
        if (!state.playing) {
            player.playWhenReady = false
            player.setPlaybackSpeed(1f)
            if (player.playbackState == Player.STATE_READY && abs(player.currentPosition - target) > PAUSED_TOLERANCE_MS) {
                player.seekTo(target)
                jumped()
            }
            return false
        }
        player.playWhenReady = true
        // buffering: nothing to measure yet
        if (player.playbackState != Player.STATE_READY || !player.isPlaying) return created

        val now = SystemClock.elapsedRealtime()
        if (seeking) {
            if (settleUntil == 0L) settleUntil = now + SETTLE_MS
            if (now < settleUntil) return created
            seeking = false
            // how late the jump landed, so the next one aims further ahead
            val gap = target - player.currentPosition
            if (abs(gap) > LEAD_TOLERANCE_MS) seekLeadMs = (seekLeadMs + gap).coerceIn(0, MAX_LEAD_MS)
        }

        gaps.addLast(target - player.currentPosition.coerceAtLeast(0))
        while (gaps.size > GAP_SAMPLES) gaps.removeFirst()
        val gap = gaps.sorted()[gaps.size / 2]
        val steady = gaps.size >= MIN_SAMPLES_TO_JUMP

        when (val action = shareSyncAction(target.toULong(), (target - gap).coerceAtLeast(0).toULong())) {
            ShareSyncAction.Keep -> setSpeed(player, 1f, now, force = true)
            is ShareSyncAction.Rate -> if (syncBySpeed) setSpeed(player, action.rate, now, force = false)
            is ShareSyncAction.Seek -> if (steady) {
                player.setPlaybackSpeed(1f)
                player.seekTo(action.positionMs.toLong() + seekLeadMs)
                jumped()
            }
        }
        return created
    }

    fun pause() {
        cancelStart()
        exo?.playWhenReady = false
    }

    private val main = Handler(Looper.getMainLooper())
    private var readyFor: ULong? = null
    private var startRunnable: Runnable? = null

    /** Starts [queueId] in [inMs], to the millisecond rather than on the next tick. */
    private fun startAt(inMs: Long, queueId: ULong) {
        if (startRunnable != null) return
        val run = Runnable {
            startRunnable = null
            val player = exo ?: return@Runnable
            if (loaded != queueId) return@Runnable
            player.playWhenReady = true
            jumped()
        }
        startRunnable = run
        main.postDelayed(run, inMs)
    }

    private fun cancelStart() {
        startRunnable?.let(main::removeCallbacks)
        startRunnable = null
    }

    /** Stops and deletes the songs kept on this phone (leaving the party). */
    fun release() {
        cancelStart()
        readyFor = null
        stopPrefetch()
        scope.cancel()
        scope = newScope()
        exo?.release()
        exo = null
        loaded = null
        seeking = false
        gaps.clear()
        val dir = cacheDir()
        val db = database
        cache?.let { c ->
            runCatching { c.release() }
            runCatching { if (db != null) SimpleCache.delete(dir, db) }
        }
        cache = null
        runCatching { dir.deleteRecursively() }
    }

    private fun jumped() {
        seeking = true
        settleUntil = 0L
        gaps.clear()
    }

    /** Speed in steps of 0.5 %, changed at most every [RATE_HOLD_MS] (each change is a tiny glitch). */
    private fun setSpeed(player: ExoPlayer, rate: Float, now: Long, force: Boolean) {
        val stepped = (rate / RATE_STEP).roundToInt() * RATE_STEP
        val current = player.playbackParameters.speed
        if (abs(stepped - current) < RATE_STEP / 2) return
        if (!force && now - rateChangedAt < RATE_HOLD_MS) return
        player.setPlaybackSpeed(stepped)
        rateChangedAt = now
    }

    // --- songs kept on this phone -------------------------------------------------------

    private fun newScope() = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

    private fun cacheDir() = File(context.cacheDir, "party")

    private fun cache(): SimpleCache = cache ?: run {
        val db = database ?: StandaloneDatabaseProvider(context).also { database = it }
        SimpleCache(cacheDir(), LeastRecentlyUsedCacheEvictor(MAX_CACHE_BYTES), db).also { cache = it }
    }

    private fun dataSource(): CacheDataSource.Factory = CacheDataSource.Factory()
        .setCache(cache())
        .setUpstreamDataSourceFactory(
            DefaultHttpDataSource.Factory().setConnectTimeoutMs(HTTP_TIMEOUT_MS).setReadTimeoutMs(HTTP_TIMEOUT_MS),
        )
        // a broken cache file plays from the hub instead of failing
        .setFlags(CacheDataSource.FLAG_IGNORE_CACHE_ON_ERROR)

    /**
     * Downloads [urls] (next, then previous) one after the other, and drops the songs no
     * longer wanted ([keep] is the one playing).
     */
    private fun prefetch(urls: List<String>, keep: String) {
        if (urls == prefetchUrls) return
        prefetchUrls = urls
        stopPrefetch()
        val wanted = urls.toSet() + keep
        val c = cache()
        prefetchJob = scope.launch(Dispatchers.IO) {
            runCatching { c.keys.filter { it !in wanted }.forEach(c::removeResource) }
            for (url in urls) {
                if (!isActive) break
                val writer = CacheWriter(dataSource().createDataSource(), DataSpec(Uri.parse(url)), null, null)
                prefetchWriter = writer
                runCatching { writer.cache() }.onFailure { if (isActive) Log.w(TAG, "preload failed: ${it.message}") }
            }
            prefetchWriter = null
        }
    }

    private fun stopPrefetch() {
        prefetchWriter?.cancel()
        prefetchWriter = null
        prefetchJob?.cancel()
        prefetchJob = null
    }

    private fun build(): ExoPlayer {
        // FFmpeg too, for hubs that stream ALAC and friends
        // the tee feeds the visualizer, VU meters and mascot, as in the app's own player;
        // the equalizer runs first, with the same settings as the app's player
        val renderers = object : DefaultRenderersFactory(context) {
            override fun buildAudioSink(context: Context, enableFloatOutput: Boolean, enableAudioTrackPlaybackParams: Boolean): AudioSink =
                DefaultAudioSink.Builder(context)
                    .setEnableFloatOutput(enableFloatOutput)
                    .setEnableAudioTrackPlaybackParams(enableAudioTrackPlaybackParams)
                    .setAudioProcessors(arrayOf(EqAudioProcessor(), TeeAudioProcessor(AudioTap)))
                    .build()
        }.setExtensionRendererMode(DefaultRenderersFactory.EXTENSION_RENDERER_MODE_ON)
        // starts after 1 s of audio, then keeps reading well ahead (the rest lands in the cache)
        val load = DefaultLoadControl.Builder()
            .setBufferDurationsMs(60_000, 300_000, 1_000, 3_000)
            .setPrioritizeTimeOverSizeThresholds(true)
            .build()
        return ExoPlayer.Builder(context, renderers)
            .setMediaSourceFactory(DefaultMediaSourceFactory(dataSource()))
            .setLoadControl(load)
            .setAudioAttributes(
                AudioAttributes.Builder().setUsage(C.USAGE_MEDIA).setContentType(C.AUDIO_CONTENT_TYPE_MUSIC).build(),
                // no focus handling: it would fight with the app's own player every tick
                /* handleAudioFocus = */ false,
            )
            .setWakeMode(C.WAKE_MODE_NETWORK)
            .build()
            .also { it.setPreferredAudioDevice(context.homefi.outputs.preferredDevice.value) }
    }

    private companion object {
        const val TAG = "PartyPlayer"
        const val PAUSED_TOLERANCE_MS = 250L
        const val LEAD_TOLERANCE_MS = 40L
        const val MAX_LEAD_MS = 3_000L
        const val SETTLE_MS = 700L
        /** Closer than this to a scheduled start, it is timed exactly instead of on a tick. */
        const val START_AHEAD_MS = 400L
        const val GAP_SAMPLES = 7
        const val MIN_SAMPLES_TO_JUMP = 4
        const val RATE_STEP = 0.005f
        const val RATE_HOLD_MS = 1_500L
        const val HTTP_TIMEOUT_MS = 8_000
        const val MAX_CACHE_BYTES = 768L * 1024 * 1024
    }
}
