package com.perseusoft.homefi.player.dj

import android.content.Context
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import androidx.annotation.OptIn
import androidx.annotation.StringRes
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.PlaybackException
import androidx.media3.common.PlaybackParameters
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.audio.AudioSink
import androidx.media3.exoplayer.audio.DefaultAudioSink
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import com.perseusoft.homefi.HomefiApplication
import com.perseusoft.homefi.R
import com.perseusoft.homefi.data.DJ_PADS
import com.perseusoft.homefi.core.DjException
import com.perseusoft.homefi.core.DjGrid
import com.perseusoft.homefi.core.SfxKind
import com.perseusoft.homefi.core.djAutoGainDb
import com.perseusoft.homefi.core.djChannelEq
import com.perseusoft.homefi.core.djCrossfaderGains
import com.perseusoft.homefi.core.djNearestBeatMs
import com.perseusoft.homefi.core.djNextBeatMs
import com.perseusoft.homefi.core.djPhaseOffsetMs
import com.perseusoft.homefi.core.djSyncTempo
import com.perseusoft.homefi.player.EqAudioProcessor
import com.perseusoft.homefi.player.EqSlot
import com.perseusoft.homefi.player.TidalStreams
import com.perseusoft.homefi.player.fileMediaItem
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import java.io.File
import kotlin.math.pow

/** Gain of a killed EQ band (dB); the core's `EQ_KILL_DB`. */
const val DJ_EQ_KILL_DB = -48f

private const val TICK_MS = 20L
private const val PAD_EVENT_BUFFER = 4
private const val PUBLISH_EVERY_TICKS = 3
private const val FRAME_MS = 16L
private const val BASS_STEP_MS = 40L

/** Speed of the first beat of a song that has no grid: a plain 120 BPM. */
private const val FALLBACK_BPM = 120.0
private const val MS_PER_MINUTE = 60_000.0

/** The audio output takes about this long to turn a start into sound; a mix starts that much early. */
private const val PLAY_LATENCY_MS = 60L

/** Where in the mix (0..1) the bass changes hands, and how wide that change is. */
private const val BASS_SWAP_AT = 0.5f
private const val BASS_SWAP_WIDTH = 0.1f

/** A cue preview counts as "at the cue" within this many ms. */
private const val AT_CUE_MS = 80L
private const val DB_PER_LOG = 20f

/**
 * The simple DJ: two ExoPlayers ("decks") like [com.perseusoft.homefi.player.Crossfader]'s, each
 * with its own [EqSlot] for the channel EQ and the crossfade level, and Media3's Sonic for the
 * tempo. All the maths (analysis, EQ filters, crossfader curves, sync) is the core's; this class
 * only runs the players and keeps the state the screen shows.
 *
 * Everything runs on the main thread. [open] pauses the normal player and starts [DjService] so a
 * mix survives a locked screen; [close] gives it all back. The normal player and its service are
 * not touched while DJ mode is on.
 */
@OptIn(UnstableApi::class)
class DjSession(private val app: HomefiApplication) {
    private var scope = newScope()
    private var players: List<ExoPlayer> = emptyList()
    private val slots = List(DJ_DECKS) { EqSlot() }
    private var sampler: DjSampler? = null
    private var tickJob: Job? = null
    private var mixJob: Job? = null
    private val loadJobs = arrayOfNulls<Job>(DJ_DECKS)
    private val previewing = BooleanArray(DJ_DECKS)
    /** dB taken off each deck's bass by a mix in progress (0 or less). */
    private val bassCut = FloatArray(DJ_DECKS)
    private var focus: AudioFocusRequest? = null
    private var noticeSeq = 0L

    private val _open = MutableStateFlow(false)
    val isOpen: StateFlow<Boolean> = _open.asStateFlow()
    private val _decks = MutableStateFlow(List(DJ_DECKS) { DjDeckUi() })
    val decks: StateFlow<List<DjDeckUi>> = _decks.asStateFlow()
    private val _mixer = MutableStateFlow(DjMixerUi())
    val mixer: StateFlow<DjMixerUi> = _mixer.asStateFlow()
    private val _padEvents = MutableSharedFlow<SfxKind>(extraBufferCapacity = PAD_EVENT_BUFFER)
    /** Every pad that fires (Homifito reacts to it); a slow collector drops the oldest. */
    val padEvents: SharedFlow<SfxKind> = _padEvents.asSharedFlow()
    private val _notice = MutableStateFlow<DjNotice?>(null)
    val notice: StateFlow<DjNotice?> = _notice.asStateFlow()

    private val dj get() = app.settings.dj.value
    private val prefs get() = app.settings.current

    private fun newScope() = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

    // ------------------------------------------------------------------ lifecycle

    fun open() {
        if (_open.value) return
        scope = newScope()
        app.player.pause() // the normal player makes way
        players = List(DJ_DECKS, ::buildPlayer)
        sampler = DjSampler(app, scope).also { it.prepare(prefs.djPads.take(DJ_PADS)) }
        _decks.value = List(DJ_DECKS) { DjDeckUi() }
        _mixer.value = DjMixerUi()
        DjService.onStopRequested = ::close
        DjService.start(app)
        scope.launch { app.outputs.preferredDevice.collect { device -> players.forEach { it.setPreferredAudioDevice(device) } } }
        // settings that change what is heard or shown while the screen is open
        scope.launch {
            app.settings.settings.map { it.djPads.take(DJ_PADS) }.distinctUntilChanged().collect { sampler?.prepare(it) }
        }
        scope.launch {
            app.settings.settings.map { it.djKeyLock }.distinctUntilChanged().collect { reapplySpeeds() }
        }
        scope.launch {
            app.settings.dj.collect {
                applyLevels()
                repeat(DJ_DECKS) { applyEq(it) }
            }
        }
        tickJob = scope.launch { tickLoop() }
        _open.value = true
    }

    fun close() {
        if (!_open.value) return
        _open.value = false
        mixJob?.cancel()
        scope.cancel()
        players.forEach { it.release() }
        players = emptyList()
        sampler?.release()
        sampler = null
        abandonFocus()
        DjService.onStopRequested = null
        DjService.stop(app)
        _decks.value = List(DJ_DECKS) { DjDeckUi() }
        _mixer.value = DjMixerUi()
    }

    private fun buildPlayer(deck: Int): ExoPlayer {
        val renderers = object : DefaultRenderersFactory(app) {
            override fun buildAudioSink(context: Context, enableFloatOutput: Boolean, enableAudioTrackPlaybackParams: Boolean): AudioSink =
                DefaultAudioSink.Builder(context)
                    .setEnableFloatOutput(enableFloatOutput)
                    .setEnableAudioTrackPlaybackParams(enableAudioTrackPlaybackParams)
                    // the channel EQ and level, then Media3's own Sonic for the tempo
                    .setAudioProcessorChain(DefaultAudioSink.DefaultAudioProcessorChain(EqAudioProcessor { slots[deck] }))
                    .build()
        }.setExtensionRendererMode(DefaultRenderersFactory.EXTENSION_RENDERER_MODE_ON)
        val sources = DefaultMediaSourceFactory(DefaultDataSource.Factory(app))
        val player = ExoPlayer.Builder(app, renderers)
            .setMediaSourceFactory(sources)
            // focus is asked once for both decks (see requestFocus): two players would fight over it
            .setAudioAttributes(
                AudioAttributes.Builder().setUsage(C.USAGE_MEDIA).setContentType(C.AUDIO_CONTENT_TYPE_MUSIC).build(),
                false,
            )
            .setHandleAudioBecomingNoisy(true)
            .setWakeMode(C.WAKE_MODE_LOCAL)
            .build()
        player.addListener(object : Player.Listener {
            override fun onIsPlayingChanged(isPlaying: Boolean) {
                updateDeck(deck) { it.copy(playing = isPlaying) }
                if (!isPlaying && players.none { it.isPlaying }) abandonFocus()
            }

            override fun onPlayerError(error: PlaybackException) {
                player.clearMediaItems()
                updateDeck(deck) { DjDeckUi(note = R.string.dj_err_open) }
                notify(R.string.dj_err_open)
            }
        })
        return player
    }

    // -------------------------------------------------------------- audio focus

    private fun requestFocus() {
        if (focus != null) return
        val manager = app.getSystemService(AudioManager::class.java) ?: return
        val request = AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
            .setAudioAttributes(
                android.media.AudioAttributes.Builder()
                    .setUsage(android.media.AudioAttributes.USAGE_MEDIA)
                    .setContentType(android.media.AudioAttributes.CONTENT_TYPE_MUSIC)
                    .build(),
            )
            .setOnAudioFocusChangeListener { change ->
                if (change == AudioManager.AUDIOFOCUS_LOSS || change == AudioManager.AUDIOFOCUS_LOSS_TRANSIENT) {
                    Handler(Looper.getMainLooper()).post { pauseAll() }
                }
            }
            .build()
        if (manager.requestAudioFocus(request) == AudioManager.AUDIOFOCUS_REQUEST_GRANTED) focus = request
    }

    private fun abandonFocus() {
        val request = focus ?: return
        focus = null
        app.getSystemService(AudioManager::class.java)?.abandonAudioFocusRequest(request)
    }

    private fun pauseAll() {
        mixJob?.cancel()
        players.forEach { it.pause() }
    }

    // ------------------------------------------------------------------- state

    private fun updateDeck(deck: Int, change: (DjDeckUi) -> DjDeckUi) =
        _decks.update { list -> list.mapIndexed { i, d -> if (i == deck) change(d) else d } }

    private fun notify(@StringRes res: Int, vararg args: String) {
        _notice.value = DjNotice(res, args.toList(), ++noticeSeq)
    }

    fun dismissNotice(id: Long) {
        _notice.update { if (it?.id == id) null else it }
    }

    private fun snap(deck: DjDeckUi, ms: Long): Long {
        val grid = deck.analysis?.grid
        return if (dj.quantize && grid != null) djNearestBeatMs(grid, ms.toDouble()).toLong().coerceAtLeast(0) else ms
    }

    private suspend fun tickLoop() {
        var ticks = 0
        while (scope.isActive) {
            players.forEachIndexed { i, player ->
                val deck = _decks.value[i]
                if (deck.loopBeats != null && player.isPlaying) {
                    val pos = player.currentPosition
                    if (pos >= deck.loopEndMs) player.seekTo(deck.loopStartMs + (pos - deck.loopEndMs).coerceAtLeast(0))
                }
            }
            if (++ticks % PUBLISH_EVERY_TICKS == 0) publishPositions()
            delay(TICK_MS)
        }
    }

    private fun publishPositions() {
        players.forEachIndexed { i, player ->
            val duration = player.duration.takeIf { it != C.TIME_UNSET } ?: 0L
            updateDeck(i) { it.copy(positionMs = player.currentPosition.coerceAtLeast(0), durationMs = duration) }
        }
    }

    // ----------------------------------------------------------------- loading

    /** Puts a song on [deck]: a local file at once, a TIDAL track after it is fetched. */
    fun load(deck: Int, source: DjSource) {
        if (!_open.value) return
        loadJobs[deck]?.cancel()
        if (_mixer.value.mix != null) cancelMix()
        players[deck].apply {
            stop()
            clearMediaItems()
            playbackParameters = PlaybackParameters.DEFAULT
        }
        bassCut[deck] = 0f
        updateDeck(deck) { DjDeckUi(loading = true, title = source.title, artist = source.artist) }
        applyEq(deck)
        loadJobs[deck] = scope.launch {
            val path = when (source) {
                is DjSource.Local -> source.path
                is DjSource.Tidal -> try {
                    app.engine.fetchStream(source.id, File(app.cacheDir, TidalStreams.STREAM_DIR).absolutePath)
                } catch (e: CancellationException) {
                    throw e
                } catch (e: Exception) {
                    updateDeck(deck) { DjDeckUi(note = R.string.dj_err_tidal) }
                    notify(R.string.dj_err_tidal)
                    return@launch
                }
            }
            players[deck].apply {
                setMediaItem(fileMediaItem(path))
                prepare()
                playWhenReady = false
            }
            updateDeck(deck) { it.copy(loaded = true, loading = false, analyzing = true, path = path) }
            applyLevels()
            notify(R.string.dj_loaded, deckLetter(deck).toString(), source.title)
            analyze(deck, path)
        }
    }

    private suspend fun analyze(deck: Int, path: String) {
        val analysis = try {
            app.engine.djAnalyzeFile(path, dj.memoryMb)
        } catch (e: CancellationException) {
            throw e
        } catch (e: DjException.Memory) {
            updateDeck(deck) { it.copy(analyzing = false, note = R.string.dj_err_memory) }
            return
        } catch (e: Exception) {
            // ExoPlayer can still play it: only the beat is missing
            updateDeck(deck) { it.copy(analyzing = false, note = R.string.dj_err_beat) }
            return
        }
        val grid = analysis.grid
        updateDeck(deck) {
            it.copy(
                analyzing = false,
                analysis = analysis,
                overview = DjOverview.from(analysis),
                cueMs = grid?.firstBeatMs?.toLong() ?: 0L,
                note = if (grid == null) R.string.dj_err_beat else null,
            )
        }
        applyLevels()
        val other = _decks.value[1 - deck]
        if (dj.syncOnLoad && grid != null && other.playing && other.hasGrid) sync(deck)
    }

    // ------------------------------------------------------------- transport

    fun togglePlay(deck: Int) {
        val player = players.getOrNull(deck) ?: return
        if (!_decks.value[deck].loaded) return
        if (player.playWhenReady) player.pause() else startDeck(deck)
    }

    private fun startDeck(deck: Int) {
        requestFocus()
        val d = _decks.value[deck]
        val other = _decks.value[1 - deck]
        if (d.synced && other.playing) alignPhase(deck)
        players[deck].play()
    }

    /** Cue pressed: while playing, back to the cue point and stop; paused elsewhere, set it here; paused at it, preview. */
    fun cueDown(deck: Int) {
        val player = players.getOrNull(deck) ?: return
        val d = _decks.value[deck]
        if (!d.loaded) return
        if (player.playWhenReady) {
            player.pause()
            player.seekTo(d.cueMs)
            return
        }
        if (kotlin.math.abs(player.currentPosition - d.cueMs) <= AT_CUE_MS) {
            previewing[deck] = true
            startDeck(deck)
        } else {
            val here = snap(d, player.currentPosition)
            updateDeck(deck) { it.copy(cueMs = here) }
        }
    }

    /** Cue released: a preview goes back to the cue point. */
    fun cueUp(deck: Int) {
        if (!previewing[deck]) return
        previewing[deck] = false
        val player = players.getOrNull(deck) ?: return
        player.pause()
        player.seekTo(_decks.value[deck].cueMs)
    }

    fun seekTo(deck: Int, fraction: Float) {
        val player = players.getOrNull(deck) ?: return
        val duration = _decks.value[deck].durationMs
        if (duration <= 0) return
        player.seekTo((duration * fraction.coerceIn(0f, 1f)).toLong())
    }

    // ------------------------------------------------------- hot cues and loops

    /** An empty hot cue saves the current spot; a saved one jumps there and plays. */
    fun hotCue(deck: Int, index: Int) {
        val player = players.getOrNull(deck) ?: return
        val d = _decks.value[deck]
        if (!d.loaded) return
        val saved = d.hotCues.getOrNull(index)
        if (saved == null) {
            val here = snap(d, player.currentPosition)
            updateDeck(deck) { it.copy(hotCues = it.hotCues.toMutableList().also { cues -> cues[index] = here }) }
        } else {
            player.seekTo(saved)
            if (!player.playWhenReady) startDeck(deck)
        }
    }

    fun clearHotCue(deck: Int, index: Int) =
        updateDeck(deck) { it.copy(hotCues = it.hotCues.toMutableList().also { cues -> cues[index] = null }) }

    /** Repeats the next [beats] beats from the nearest beat; the same button again leaves the loop. */
    fun toggleLoop(deck: Int, beats: Int) {
        val player = players.getOrNull(deck) ?: return
        val d = _decks.value[deck]
        val grid = d.analysis?.grid ?: return
        if (d.loopBeats == beats) {
            exitLoop(deck)
            return
        }
        val start = snap(d, player.currentPosition)
        val end = start + (grid.beatMs * beats).toLong()
        updateDeck(deck) { it.copy(loopBeats = beats, loopStartMs = start, loopEndMs = end) }
    }

    fun exitLoop(deck: Int) = updateDeck(deck) { it.copy(loopBeats = null) }

    // ------------------------------------------------------------ tempo and sync

    private fun speedRange(): Float = dj.tempoRangePct / 100f

    private fun applySpeed(deck: Int, speed: Float) {
        val range = speedRange()
        val s = speed.coerceIn(1f - range, 1f + range)
        val pitch = if (prefs.djKeyLock) 1f else s
        players.getOrNull(deck)?.playbackParameters = PlaybackParameters(s, pitch)
        updateDeck(deck) { it.copy(speed = s) }
    }

    private fun reapplySpeeds() = _decks.value.forEachIndexed { i, d -> if (d.loaded) applySpeed(i, d.speed) }

    /** The tempo slider (percent). A synced pair keeps one tempo: the other deck follows. */
    fun setTempoPct(deck: Int, pct: Float) {
        applySpeed(deck, 1f + pct / 100f)
        val d = _decks.value
        if (d[0].synced || d[1].synced) followTempo(from = deck)
    }

    fun resetTempo(deck: Int) = setTempoPct(deck, 0f)

    private fun followTempo(from: Int) {
        val other = 1 - from
        val o = _decks.value[other]
        val ownBpm = o.analysis?.bpm ?: return
        val ratio = djSyncTempo(ownBpm, _decks.value[from].bpm, dj.tempoRangePct.toDouble())
        if (ratio == null) {
            updateDeck(0) { it.copy(synced = false) }
            updateDeck(1) { it.copy(synced = false) }
            notify(R.string.dj_err_far)
        } else {
            applySpeed(other, ratio.toFloat())
        }
    }

    /** Sync: this deck takes the other one's tempo and, if both play, its beat. Again = off. */
    fun toggleSync(deck: Int) {
        if (_decks.value[deck].synced) updateDeck(deck) { it.copy(synced = false) } else sync(deck)
    }

    private fun sync(deck: Int): Boolean {
        val d = _decks.value[deck]
        val other = _decks.value[1 - deck]
        if (!d.loaded || !d.hasGrid || !other.loaded || !other.hasGrid) {
            notify(R.string.dj_err_beat)
            return false
        }
        if (!syncTempo(deck)) return false
        updateDeck(deck) { it.copy(synced = true) }
        if (d.playing && other.playing) alignPhase(deck)
        return true
    }

    /** Tempo half of Sync; tells the user when the songs are too far apart. */
    private fun syncTempo(deck: Int): Boolean {
        val own = _decks.value[deck].analysis?.bpm ?: return false
        val ratio = djSyncTempo(own, _decks.value[1 - deck].bpm, dj.tempoRangePct.toDouble())
        if (ratio == null) {
            notify(R.string.dj_err_far)
            return false
        }
        applySpeed(deck, ratio.toFloat())
        return true
    }

    /** Nudges [deck] to the nearest spot where its beat falls with the other deck's. */
    private fun alignPhase(deck: Int) {
        val leader = _decks.value[1 - deck]
        val follower = _decks.value[deck]
        val leaderGrid = leader.analysis?.grid ?: return
        val followerGrid = follower.analysis?.grid ?: return
        val offset = djPhaseOffsetMs(
            leaderGrid, players[1 - deck].currentPosition.toDouble(),
            followerGrid, players[deck].currentPosition.toDouble(),
        )
        players[deck].seekTo((players[deck].currentPosition + offset).toLong().coerceAtLeast(0))
    }

    // ------------------------------------------------------------ mixer

    /** The user moves the crossfader: a mix in progress stops. */
    fun setCrossfader(position: Float) {
        if (_mixer.value.mix != null) cancelMix()
        _mixer.update { it.copy(crossfader = position.coerceIn(0f, 1f)) }
        applyLevels()
    }

    fun setMaster(volume: Float) {
        _mixer.update { it.copy(master = volume.coerceIn(0f, 1f)) }
        applyLevels()
    }

    fun setVolume(deck: Int, volume: Float) {
        updateDeck(deck) { it.copy(volume = volume.coerceIn(0f, 1f)) }
        applyLevels()
    }

    fun setTrimDb(deck: Int, db: Float) {
        updateDeck(deck) { it.copy(trimDb = db) }
        applyLevels()
    }

    fun setEq(deck: Int, change: (DjEq) -> DjEq) {
        updateDeck(deck) { it.copy(eq = change(it.eq)) }
        applyEq(deck)
    }

    private fun applyEq(deck: Int) {
        val eq = _decks.value[deck].eq
        val boost = dj.eqMaxBoostDb
        val low = ((if (eq.killLow) DJ_EQ_KILL_DB else eq.lowDb) + bassCut[deck]).coerceAtLeast(DJ_EQ_KILL_DB)
        val mid = if (eq.killMid) DJ_EQ_KILL_DB else eq.midDb
        val high = if (eq.killHigh) DJ_EQ_KILL_DB else eq.highDb
        val filter = eq.filter
        slots[deck].configureDirect { rate -> djChannelEq(low, mid, high, filter, boost, rate.toUInt()) }
    }

    /**
     * Each deck's level: crossfader curve x channel volume x master x trim. Auto-gain trims are
     * brought to at most 0 dB together (the processor's level cannot exceed 1), so matching two
     * songs lowers the louder one and, if the quieter needs a boost, lowers both.
     */
    private fun applyLevels() {
        val mixer = _mixer.value
        val gains = djCrossfaderGains(mixer.crossfader, dj.crossfaderCurve)
        val cross = floatArrayOf(gains.deckA, gains.deckB)
        val trims = _decks.value.map { d ->
            val auto = if (dj.autoGain) d.analysis?.let { djAutoGainDb(it.loudnessDb) } ?: 0f else 0f
            auto + d.trimDb
        }
        val ceiling = maxOf(0f, trims.max())
        _decks.value.forEachIndexed { i, d ->
            val trim = 10f.pow((trims[i] - ceiling) / DB_PER_LOG)
            slots[i].fadeGain = cross[i] * d.volume * mixer.master * trim
        }
    }

    // ------------------------------------------------------------------- mix

    /** Mix to deck [to]: a crossfade over the chosen number of beats, started on a beat of the other deck. */
    fun mixTo(to: Int) {
        if (!_open.value) return
        val d = _decks.value
        if (!d[0].loaded || !d[1].loaded) {
            notify(R.string.dj_both_decks)
            return
        }
        // the previous mix puts its EQ back before this one starts
        val previous = mixJob
        mixJob = scope.launch {
            previous?.cancelAndJoin()
            runMix(from = 1 - to, to = to)
        }
    }

    fun cancelMix() {
        mixJob?.cancel()
    }

    private suspend fun runMix(from: Int, to: Int) {
        val beats = dj.autoMixBeats.toInt()
        val bassSwap = prefs.djBassSwap
        var completed = false
        try {
            if (dj.syncOnLoad && _decks.value[from].hasGrid && _decks.value[to].hasGrid) {
                if (syncTempo(to)) updateDeck(to) { it.copy(synced = true) }
            }
            val incoming = _decks.value[to]
            if (!incoming.playing && incoming.hasGrid && dj.quantize) players[to].seekTo(snap(incoming, players[to].currentPosition))
            // the bass of the new song stays out until the swap
            if (bassSwap) setBassCuts(from, to, 0f)
            _mixer.update { it.copy(mix = DjMixUi(to, 0f, beats)) }
            notify(R.string.dj_mix_started, beats.toString())
            // start on a beat of the song that plays now
            val out = _decks.value[from]
            val outGrid: DjGrid? = out.analysis?.grid
            if (out.playing && outGrid != null) {
                val pos = players[from].currentPosition.toDouble()
                val wait = ((djNextBeatMs(outGrid, pos) - pos) / out.speed).toLong() - PLAY_LATENCY_MS
                if (wait > 0) delay(wait)
            }
            if (!players[to].playWhenReady) {
                requestFocus()
                players[to].play()
            }
            val total = (beats * beatLengthMs(from, to)).toLong().coerceAtLeast(1L)
            val startAt = _mixer.value.crossfader
            val endAt = if (to == 1) 1f else 0f
            val started = SystemClock.elapsedRealtime()
            var lastBass = Long.MIN_VALUE
            while (true) {
                val elapsed = SystemClock.elapsedRealtime() - started
                val t = (elapsed.toFloat() / total).coerceIn(0f, 1f)
                _mixer.update { it.copy(crossfader = startAt + (endAt - startAt) * t, mix = DjMixUi(to, t, beats)) }
                applyLevels()
                if (bassSwap && elapsed - lastBass >= BASS_STEP_MS) {
                    lastBass = elapsed
                    setBassCuts(from, to, t)
                }
                if (t >= 1f) break
                delay(FRAME_MS)
            }
            completed = true
        } finally {
            bassCut.fill(0f)
            repeat(DJ_DECKS) { applyEq(it) }
            _mixer.update { it.copy(mix = null) }
            if (completed) {
                players[from].pause()
                notify(R.string.dj_mix_done)
            }
        }
    }

    /** Length of a beat in real time (ms) at the speed the mix runs at. */
    private fun beatLengthMs(from: Int, to: Int): Double {
        val bpm = listOf(_decks.value[from], _decks.value[to]).firstOrNull { it.hasGrid }?.bpm ?: FALLBACK_BPM
        return MS_PER_MINUTE / bpm
    }

    /** The outgoing bass leaves and the incoming one arrives around [BASS_SWAP_AT]. */
    private fun setBassCuts(from: Int, to: Int, t: Float) {
        val s = ((t - (BASS_SWAP_AT - BASS_SWAP_WIDTH / 2)) / BASS_SWAP_WIDTH).coerceIn(0f, 1f)
        val smooth = s * s * (3f - 2f * s)
        bassCut[from] = DJ_EQ_KILL_DB * smooth
        bassCut[to] = DJ_EQ_KILL_DB * (1f - smooth)
        applyEq(from)
        applyEq(to)
    }

    // ------------------------------------------------------------------ pads

    fun pad(kind: SfxKind) {
        sampler?.play(kind, dj.samplerVolume)
        _padEvents.tryEmit(kind)
    }

    /** Deck that is not playing (deck A when both are idle): where a song goes by default. */
    fun freeDeck(): Int = if (_decks.value[0].playing && !_decks.value[1].playing) 1 else 0
}
