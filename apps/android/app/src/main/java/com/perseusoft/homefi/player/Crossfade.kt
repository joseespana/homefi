package com.perseusoft.homefi.player

import android.app.ActivityManager
import android.content.Context
import android.os.SystemClock
import androidx.annotation.OptIn
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.ForwardingPlayer
import androidx.media3.common.MediaItem
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.audio.TeeAudioProcessor
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.CrossfadeMode
import com.perseusoft.homefi.core.CrossfadeSettings
import com.perseusoft.homefi.core.FadeCurve
import com.perseusoft.homefi.core.FadeTrack
import com.perseusoft.homefi.core.crossfadeFades
import com.perseusoft.homefi.core.crossfadeGains
import com.perseusoft.homefi.core.crossfadeSeconds
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import java.nio.ByteBuffer
import kotlin.math.abs
import kotlin.math.min

/** A crossfade being heard, for the now-playing screen. */
data class FadeUi(val fromTitle: String, val toTitle: String, val progress: Float, val seconds: Float, val manual: Boolean)

/** What the app shows about the crossfade (the service runs it; same process). */
object Crossfading {
    private val _state = MutableStateFlow<FadeUi?>(null)
    /** The fade playing now, or null. */
    val state: StateFlow<FadeUi?> = _state.asStateFlow()

    private val _unavailable = MutableStateFlow<Int?>(null)
    /** Why crossfade fell back to normal transitions on this phone (a string resource), or null. */
    val unavailable: StateFlow<Int?> = _unavailable.asStateFlow()

    internal fun publish(ui: FadeUi?) {
        val old = _state.value
        // the screen only needs a few steps a second
        if (ui == null || old == null || abs(ui.progress - old.progress) >= 0.02f || ui.progress >= 1f) _state.value = ui
    }

    internal fun unavailable(reason: Int?) {
        _unavailable.value = reason
    }
}

/**
 * The media session's player: one deck's ExoPlayer, with next / previous / picking a song
 * routed through the crossfade first ([skip] returns `true` when it fades there itself).
 */
@OptIn(UnstableApi::class)
class DeckPlayer(val exo: ExoPlayer, private val skip: (Int) -> Boolean) : ForwardingPlayer(exo) {
    override fun seekToNext() {
        val next = exo.nextMediaItemIndex
        if (next == C.INDEX_UNSET || !skip(next)) super.seekToNext()
    }

    override fun seekToNextMediaItem() {
        val next = exo.nextMediaItemIndex
        if (next == C.INDEX_UNSET || !skip(next)) super.seekToNextMediaItem()
    }

    override fun seekToPrevious() {
        // past the first seconds "previous" restarts the song: a fade into its own start
        val restart = exo.currentPosition > exo.maxSeekToPreviousPosition || !exo.hasPreviousMediaItem()
        val target = if (restart) exo.currentMediaItemIndex else exo.previousMediaItemIndex
        if (target == C.INDEX_UNSET || !skip(target)) super.seekToPrevious()
    }

    override fun seekToPreviousMediaItem() {
        val prev = exo.previousMediaItemIndex
        if (prev == C.INDEX_UNSET || !skip(prev)) super.seekToPreviousMediaItem()
    }

    override fun seekToDefaultPosition(mediaItemIndex: Int) {
        if (mediaItemIndex == exo.currentMediaItemIndex || !skip(mediaItemIndex)) super.seekToDefaultPosition(mediaItemIndex)
    }

    override fun seekTo(mediaItemIndex: Int, positionMs: Long) {
        val fromStart = positionMs == 0L || positionMs == C.TIME_UNSET
        if (mediaItemIndex == exo.currentMediaItemIndex || !fromStart || !skip(mediaItemIndex)) super.seekTo(mediaItemIndex, positionMs)
    }
}

/**
 * Feeds [AudioTap] (visualizer, VU meters, mascot) from one deck only while it is the
 * session's, so two songs overlapping don't mix in the meters.
 */
@OptIn(UnstableApi::class)
class DeckTap(private val deck: Int) : TeeAudioProcessor.AudioBufferSink {
    override fun flush(sampleRateHz: Int, channelCount: Int, encoding: Int) {
        if (EqRuntime.activeDeck == deck) AudioTap.flush(sampleRateHz, channelCount, encoding)
    }

    override fun handleBuffer(buffer: ByteBuffer) {
        if (EqRuntime.activeDeck == deck) AudioTap.handleBuffer(buffer)
    }
}

/**
 * Crossfade with two players ("decks"). The session plays the active deck; near the end of
 * a song the other deck is prepared with the same queue at the next song (paused, buffered
 * ahead), and when the fade starts it plays and becomes the session's player at once: the
 * notification, lock screen and the app's queue see a normal song change, and every queue
 * edit from then on goes to it. The old deck plays out its song alone (nothing queued after
 * it) while the levels cross in each deck's [EqAudioProcessor], then stops.
 *
 * The levels follow a ticker on the main thread (~60 Hz while fading) and are ramped across
 * each audio buffer, so the steps are inaudible. Each deck has its own [EqSlot]: the songs
 * keep their own taste and loudness while they overlap.
 *
 * Fallbacks (a normal, gapless transition): crossfade off, repeat-one, an album playing in
 * order (when kept gapless), songs too short, a Tidal song not fetched yet or the other deck
 * not ready when the fade should start. With "low-power fallback" on, phones that report low
 * memory never crossfade, and a phone that lags twice during fades (ticker stalls, the
 * incoming deck rebuffering) stops crossfading until the app restarts.
 */
@OptIn(UnstableApi::class)
class Crossfader(
    context: Context,
    val players: List<ExoPlayer>,
    private val attributes: AudioAttributes,
    private val settings: StateFlow<CrossfadeSettings>,
    private val sound: SoundLayers,
    private val streams: TidalStreams,
    private val scope: CoroutineScope,
    /** The session must now play deck [to] (it was [from]). */
    private val onSwap: (from: Int, to: Int) -> Unit,
) {
    val decks: List<DeckPlayer> = players.map { DeckPlayer(it, ::skipTo) }

    /** The deck the session plays. */
    var active = 0
        private set
    val player: ExoPlayer get() = players[active]
    private val spare get() = 1 - active

    private val lowRam: Boolean = runCatching {
        val am = context.getSystemService(ActivityManager::class.java)
        val info = ActivityManager.MemoryInfo().also { am.getMemoryInfo(it) }
        am.isLowRamDevice || info.totalMem in 1 until LOW_RAM_BYTES
    }.getOrDefault(false)
    private var lagging = false
    private var lagFades = 0

    private class Stage(val index: Int, val mediaId: String, val manual: Boolean, val queue: Int, val at: Long)
    private var stage: Stage? = null
    /** The song (media id) found not to fade: not asked again every tick. */
    private var noFadeFor: String? = null
    private var silentSince = 0L

    private class Fade(val from: Int, val to: Int, val lengthMs: Long, val curve: FadeCurve, val manual: Boolean, val fromTitle: String, val toTitle: String) {
        var elapsed = 0L
        var last = SystemClock.elapsedRealtime()
        var lagged = false
    }
    private var fade: Fade? = null

    init {
        players.forEachIndexed { i, p ->
            p.addListener(object : Player.Listener {
                override fun onPlaybackStateChanged(playbackState: Int) {
                    if (playbackState == Player.STATE_READY && i == spare) onSpareReady()
                    if (playbackState == Player.STATE_BUFFERING) fade?.takeIf { it.to == i }?.lagged = true
                    if (playbackState == Player.STATE_ENDED && fade?.from == i) finishFade()
                }

                override fun onPlayWhenReadyChanged(playWhenReady: Boolean, reason: Int) {
                    // paused during a fade: the outgoing song stops with it
                    if (!playWhenReady && fade?.to == i) finishFade()
                }
            })
        }
        if (lowRam) Crossfading.unavailable(R.string.crossfade_unavailable_memory)
        scope.launch {
            while (isActive) delay(tick())
        }
    }

    private fun enabled(s: CrossfadeSettings): Boolean =
        s.mode == CrossfadeMode.ON && !(s.lowPowerFallback && (lowRam || lagging))

    private fun now() = SystemClock.elapsedRealtime()

    /** One step; returns how long to wait for the next. */
    private fun tick(): Long {
        val s = settings.value
        fade?.let {
            step(it)
            return FAST_MS
        }
        stage?.takeIf { it.manual }?.let { st ->
            if (now() - st.at > MANUAL_WAIT_MS) {
                // the other deck didn't get ready in time: a plain skip
                cancelStage()
                player.seekTo(st.index, 0)
            }
            return FAST_MS
        }
        val p = player
        val item = p.currentMediaItem
        val slot = EqRuntime.decks[active]
        if (item == null || !enabled(s) || !p.isPlaying || p.repeatMode == Player.REPEAT_MODE_ONE) {
            slot.measure = false
            cancelAuto()
            return IDLE_MS
        }
        val duration = p.duration
        val next = p.nextMediaItemIndex
        if (duration == C.TIME_UNSET || duration <= 0 || next == C.INDEX_UNSET || noFadeFor == item.mediaId) {
            slot.measure = false
            return IDLE_MS
        }
        val remaining = duration - p.currentPosition
        val fadeMs = (crossfadeSeconds(s, false) * 1000).toLong()
        val window = fadeMs + if (s.smart) SMART_AHEAD_MS else AHEAD_MS
        slot.measure = s.smart && remaining <= window
        if (remaining > window) {
            cancelAuto() // seeked back
            return if (remaining > window + 3000) IDLE_MS else FAST_MS
        }
        val target = p.getMediaItemAt(next)
        val st = stage
        if (st == null) {
            if (!fades(p, item, target, manual = false)) {
                noFadeFor = item.mediaId
                return IDLE_MS
            }
            // a Tidal song still downloading: ask again (TidalStreams fetches ahead)
            if (!streams.ready(target.mediaId)) return IDLE_MS
            stageAt(next, manual = false)
            silentSince = 0
            return FAST_MS
        }
        if (st.index != next || st.mediaId != target.mediaId) {
            cancelAuto() // the queue changed under it
            return FAST_MS
        }
        val silent = s.smart && silence(slot) >= SILENCE_MS
        if (remaining <= fadeMs + GUARD_MS || silent) {
            val length = if (silent) fadeMs else min(fadeMs, remaining - GUARD_MS)
            val ready = players[spare].playbackState == Player.STATE_READY && queueHash(p) == st.queue
            if (ready && length >= MIN_FADE_MS) {
                startFade(length, manual = false)
            } else {
                // not ready in time: the song ends as usual (gapless)
                cancelAuto()
                noFadeFor = item.mediaId
            }
        }
        return FAST_MS
    }

    /** How long (ms) the active deck's music has been below -60 dBFS. */
    private fun silence(slot: EqSlot): Long {
        val peak = slot.takePeak()
        if (peak < 0f) return if (silentSince == 0L) 0 else now() - silentSince // no new audio
        if (peak >= SILENCE_LEVEL) {
            silentSince = 0
            return 0
        }
        if (silentSince == 0L) silentSince = now()
        return now() - silentSince
    }

    private fun fades(p: Player, from: MediaItem, to: MediaItem, manual: Boolean): Boolean {
        val s = settings.value
        val (fa, fr) = sound.fadeInfo(from)
        val (ta, tr) = sound.fadeInfo(to)
        val fromMs = p.duration.takeIf { it != C.TIME_UNSET && it > 0 }?.toULong()
        val toMs = to.mediaMetadata.durationMs?.takeIf { it > 0 }?.toULong()
        return runCatching {
            crossfadeFades(s, FadeTrack(fa, fr, fromMs), FadeTrack(ta, tr, toMs), p.shuffleModeEnabled, manual)
        }.getOrDefault(false)
    }

    private fun queueHash(p: Player): Int {
        var h = p.mediaItemCount
        for (i in 0 until p.mediaItemCount) h = h * 31 + p.getMediaItemAt(i).mediaId.hashCode()
        return h
    }

    /** The other deck: the same queue, paused at [index], buffering. */
    private fun stageAt(index: Int, manual: Boolean) {
        val src = player
        val dst = players[spare]
        val items = (0 until src.mediaItemCount).map(src::getMediaItemAt)
        dst.setMediaItems(items, index, 0)
        dst.shuffleModeEnabled = src.shuffleModeEnabled
        runCatching { dst.setShuffleOrder(src.shuffleOrder) }
        dst.repeatMode = src.repeatMode
        dst.playbackParameters = src.playbackParameters
        dst.volume = src.volume
        dst.setAudioAttributes(attributes, false)
        // smart: the next song starts where its music does
        dst.skipSilenceEnabled = settings.value.smart
        val slot = EqRuntime.decks[spare]
        slot.fadeGain = 0f
        sound.inputFor(items[index], src.shuffleModeEnabled)?.let(slot::configure)
        dst.playWhenReady = false
        dst.prepare()
        stage = Stage(index, items[index].mediaId, manual, queueHash(src), now())
    }

    private fun cancelStage() {
        stage = null
        val dst = players[spare]
        if (dst.mediaItemCount > 0 || dst.playbackState != Player.STATE_IDLE) {
            dst.stop()
            dst.clearMediaItems()
        }
        dst.skipSilenceEnabled = false
        EqRuntime.decks[spare].fadeGain = 1f
    }

    private fun cancelAuto() {
        if (stage?.manual == false) cancelStage()
    }

    /** Next / previous / a pick while playing: a short fade when allowed (`false`: seek as usual). */
    private fun skipTo(index: Int): Boolean {
        if (fade != null) {
            finishFade() // skipping again mid-fade: a plain skip
            return false
        }
        val s = settings.value
        val p = player
        if (!enabled(s) || !s.onManualSkip || !p.isPlaying || index !in 0 until p.mediaItemCount) return false
        val target = p.getMediaItemAt(index)
        val current = p.currentMediaItem ?: return false
        if (!streams.ready(target.mediaId) || !fades(p, current, target, manual = true)) return false
        val st = stage
        if (st != null && st.index == index && st.mediaId == target.mediaId && st.queue == queueHash(p)) {
            stage = Stage(st.index, st.mediaId, true, st.queue, now())
        } else {
            cancelStage()
            stageAt(index, manual = true)
        }
        if (players[spare].playbackState == Player.STATE_READY) onSpareReady()
        return true
    }

    private fun onSpareReady() {
        val st = stage ?: return
        if (!st.manual || fade != null) return
        val p = player
        val manualMs = (crossfadeSeconds(settings.value, true) * 1000).toLong()
        val left = if (p.duration == C.TIME_UNSET) manualMs else p.duration - p.currentPosition - GUARD_MS
        startFade(min(manualMs, left.coerceAtLeast(MIN_FADE_MS)), manual = true)
    }

    private fun startFade(lengthMs: Long, manual: Boolean) {
        val from = active
        val to = spare
        val out = players[from]
        val inc = players[to]
        val s = settings.value
        // one audio focus holder: the session's player
        out.setAudioAttributes(attributes, false)
        inc.setAudioAttributes(attributes, true)
        EqRuntime.decks[to].fadeGain = 0f
        inc.play()
        // the outgoing song plays out alone
        val i = out.currentMediaItemIndex
        if (i + 1 < out.mediaItemCount) out.removeMediaItems(i + 1, out.mediaItemCount)
        out.repeatMode = Player.REPEAT_MODE_OFF
        out.skipSilenceEnabled = s.smart && !manual
        fade = Fade(
            from, to, lengthMs, s.curve, manual,
            out.mediaMetadata.title?.toString().orEmpty(),
            inc.mediaMetadata.title?.toString().orEmpty(),
        )
        stage = null
        noFadeFor = null
        silentSince = 0
        active = to
        EqRuntime.activeDeck = to
        onSwap(from, to)
        step(fade!!)
    }

    private fun step(f: Fade) {
        val t0 = now()
        val dt = t0 - f.last
        f.last = t0
        val inc = players[f.to]
        if (inc.isPlaying) {
            f.elapsed += dt
            if (dt > LAG_MS) f.lagged = true
        }
        val t = (f.elapsed.toFloat() / f.lengthMs).coerceIn(0f, 1f)
        val g = runCatching { crossfadeGains(f.curve, t) }.getOrNull() ?: listOf(1f - t, t)
        EqRuntime.decks[f.from].fadeGain = g[0]
        EqRuntime.decks[f.to].fadeGain = g[1]
        Crossfading.publish(FadeUi(f.fromTitle, f.toTitle, t, f.lengthMs / 1000f, f.manual))
        val out = players[f.from]
        if (t >= 1f || out.playbackState == Player.STATE_IDLE || out.playbackState == Player.STATE_ENDED) finishFade()
    }

    private fun finishFade() {
        val f = fade ?: return
        fade = null
        val out = players[f.from]
        out.stop()
        out.clearMediaItems()
        out.skipSilenceEnabled = false
        EqRuntime.decks[f.from].apply {
            fadeGain = 1f
            measure = false
        }
        EqRuntime.decks[f.to].fadeGain = 1f
        players[f.to].skipSilenceEnabled = false
        Crossfading.publish(null)
        if (f.lagged && ++lagFades >= LAG_FADES) {
            lagging = true
            if (settings.value.lowPowerFallback) Crossfading.unavailable(R.string.crossfade_unavailable_lag)
        }
    }

    /** The output picked in the app, for both decks. */
    fun setPreferredAudioDevice(device: android.media.AudioDeviceInfo?) = players.forEach { it.setPreferredAudioDevice(device) }

    fun release() {
        Crossfading.publish(null)
        players.forEach { it.release() }
    }

    private companion object {
        const val IDLE_MS = 250L
        const val FAST_MS = 16L
        /** Buffer the next song this long before the fade (Tidal: fetched long before by TidalStreams). */
        const val AHEAD_MS = 5_000L
        /** With the smart trim, earlier: the music may end up to ~10 s before the file. */
        const val SMART_AHEAD_MS = 12_000L
        /** The levels reach the audio a buffer ahead of what is heard: end the fade this much before the file does. */
        const val GUARD_MS = 500L
        const val MIN_FADE_MS = 500L
        const val MANUAL_WAIT_MS = 2_000L
        /** Silence this long near the end means the music is over (smart). */
        const val SILENCE_MS = 1_000L
        const val SILENCE_LEVEL = 0.001f // -60 dBFS
        /** A ticker step this late during a fade counts as lag. */
        const val LAG_MS = 150L
        const val LAG_FADES = 2
        /** Phones below this (or reporting low RAM) skip the second player (low-power fallback). */
        const val LOW_RAM_BYTES = 2L * 1024 * 1024 * 1024
    }
}
