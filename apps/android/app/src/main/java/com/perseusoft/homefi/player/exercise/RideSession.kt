package com.perseusoft.homefi.player.exercise

import android.os.SystemClock
import androidx.media3.common.Player
import com.perseusoft.homefi.HomefiApplication
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.ExerciseRide
import com.perseusoft.homefi.core.ExerciseSession
import com.perseusoft.homefi.core.ExerciseSettings
import com.perseusoft.homefi.core.HomePlaylist
import com.perseusoft.homefi.core.ExerciseTurn
import com.perseusoft.homefi.core.LibraryRef
import com.perseusoft.homefi.core.MusicWish
import com.perseusoft.homefi.core.PlannedSong
import com.perseusoft.homefi.core.PresetId
import com.perseusoft.homefi.core.RideBuilt
import com.perseusoft.homefi.core.RideListen
import com.perseusoft.homefi.core.RideListener
import com.perseusoft.homefi.core.RidePlan
import com.perseusoft.homefi.core.RideProgress
import com.perseusoft.homefi.core.RideRequest
import com.perseusoft.homefi.core.RideTaste
import com.perseusoft.homefi.core.RideView
import com.perseusoft.homefi.core.RideWorkout
import com.perseusoft.homefi.core.LyricLine
import com.perseusoft.homefi.core.SongLyrics
import com.perseusoft.homefi.lyrics.LyricsState
import com.perseusoft.homefi.core.SavedRideSummary
import com.perseusoft.homefi.core.TvAudio
import com.perseusoft.homefi.core.TvSession
import com.perseusoft.homefi.core.exerciseArtistSuggestions
import com.perseusoft.homefi.core.exerciseDefaultRequest
import com.perseusoft.homefi.core.exerciseListenPermissions
import com.perseusoft.homefi.core.exerciseRideToken
import com.perseusoft.homefi.data.DeviceForm
import com.perseusoft.homefi.data.refs
import com.perseusoft.homefi.data.userMessage
import com.perseusoft.homefi.share.RideHubTerms
import com.perseusoft.homefi.share.net.ShareNetworkMode
import com.perseusoft.homefi.ui.eq.ChatLine
import com.perseusoft.homefi.player.withSource
import com.perseusoft.homefi.ui.playlists.entryMediaItem
import com.perseusoft.homefi.ui.playlists.languageTag
import com.perseusoft.homefi.ui.playlists.queue
import com.perseusoft.homefi.ui.playlists.source
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import kotlin.math.ceil

/** What the Exercise screens show. */
sealed interface RideUi {
    /** Choosing the ride (the form is [RideSession.form]). */
    data object Setup : RideUi

    /** [progress] is null until the core reports the first step; [aiName] is the provider picking songs. */
    data class Building(val progress: RideProgress?, val aiName: String?) : RideUi

    /**
     * The plan, before riding. [saved] is its entry in "My rides" (saved by itself, or the one being ridden again).
     * [rebuilding]: "new songs" runs and the plan stays on screen until the new one replaces it;
     * [rebuildError]: the last "new songs" failed (this plan is still ready).
     */
    data class Preview(
        val built: RideBuilt,
        val saved: SavedRideSummary? = null,
        val rebuilding: Building? = null,
        val rebuildError: String? = null,
    ) : RideUi

    /** Riding (or paused): the clock is [RideSession.frame]. */
    data class Riding(val plan: RidePlan) : RideUi

    data class Finished(val plan: RidePlan, val elapsedS: Int, val completed: Boolean) : RideUi

    data class Failed(val message: String) : RideUi
}

/** The choices of the setup screen, kept while the ride is built so a rotation or "back" loses nothing. */
data class RideForm(
    val minutes: Int,
    val style: PresetId,
    val genres: Set<String> = emptySet(),
    val mix: Boolean = false,
    val sentence: String = "",
    val useAi: Boolean = false,
    /** The workout the AI chat designed; null builds [style] at [minutes]. */
    val workout: RideWorkout? = null,
) {
    /** [taste] is the saved setting (the setup's taste chips edit the settings, not the form). */
    fun toRequest(taste: RideTaste) = RideRequest(
        minutes = minutes.toUShort(),
        style = style,
        wish = MusicWish(genres.toList(), mix, sentence.trim(), taste),
        workout = workout,
    )
}

/** The optional chat with the user's AI on the setup screen. */
data class RideChat(
    val lines: List<ChatLine> = emptyList(),
    val replies: List<String> = emptyList(),
    val thinking: Boolean = false,
    val error: String? = null,
) {
    val started: Boolean get() = lines.isNotEmpty() || thinking
}

/** One look at the ride: the core's view plus when it was taken, so the screen can animate between ticks. */
data class RideFrame(
    val view: RideView,
    val playing: Boolean,
    /** `SystemClock.elapsedRealtime()` when [view] was taken. */
    val takenAtMs: Long,
    val song: PlannedSong?,
    val nextSong: PlannedSong?,
)

/** The TV of the ride, as the ride screen shows it. */
data class RideTvStatus(
    val url: String? = null,
    /** The music plays on the TV (listen-only hub); false: pictures only. */
    val audioOnTv: Boolean = false,
    /** The ride waits for the TV to join before the music starts. */
    val waitingForTv: Boolean = false,
    val error: String? = null,
) {
    val active: Boolean get() = url != null
}

/**
 * Exercise mode on the phone: builds the ride through the core, plays it in the normal player (the ride
 * is saved as a HomeFi playlist), keeps the ride's clock at about 10 Hz, sounds the cues, and publishes
 * to the TV through [RideTvHost] (inside the playback service). All rules (segments, cadence, cues) are the
 * core's; this class only moves data between the player, the core and the screens. Main thread.
 */
class RideSession(private val app: HomefiApplication) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val settings: ExerciseSettings get() = app.settings.exercise.value

    private val _open = MutableStateFlow(false)
    /** The ride screen (or the summary) shows over the app; a ride keeps going while it is hidden. */
    val isOpen: StateFlow<Boolean> = _open.asStateFlow()

    private val _ui = MutableStateFlow<RideUi>(RideUi.Setup)
    val ui: StateFlow<RideUi> = _ui.asStateFlow()

    private val _form = MutableStateFlow(newForm())
    val form: StateFlow<RideForm> = _form.asStateFlow()

    private val _chat = MutableStateFlow(RideChat())
    val chat: StateFlow<RideChat> = _chat.asStateFlow()

    private val _frame = MutableStateFlow<RideFrame?>(null)
    val frame: StateFlow<RideFrame?> = _frame.asStateFlow()

    private val _tv = MutableStateFlow(RideTvStatus())
    val tv: StateFlow<RideTvStatus> = _tv.asStateFlow()

    /** A line about the ride a snackbar can show once (a fallback, a refusal). */
    private val _notice = MutableStateFlow<Int?>(null)
    val notice: StateFlow<Int?> = _notice.asStateFlow()

    private val _swapping = MutableStateFlow<Int?>(null)
    /** The segment (song index of the plan) being swapped for another song, or null when idle. */
    val swapping: StateFlow<Int?> = _swapping.asStateFlow()

    private val _calibrating = MutableStateFlow(false)
    /** The bike calibration's clicks are sounding. */
    val calibrating: StateFlow<Boolean> = _calibrating.asStateFlow()

    private val tvHost = MutableStateFlow<RideTvHost?>(null)
    private var fader: RideFader? = null
    private val cues by lazy { RideCues(app, scope).also(RideCues::prepare) }

    private var buildJob: Job? = null
    private var loop: Job? = null
    private var ride: ExerciseRide? = null
    /** For each queue item, the plan's song it plays (an entry that can't play has no item). Fixed for the ride: a swap replaces in place. */
    private var itemToSong: IntArray = IntArray(0)
    /** The ride's songs as queued, to notice when the rider put on something else. */
    private val rideMediaIds = ArrayList<String>()
    /** Items just replaced under the playing song: the player still reports the old media id for a moment. */
    private val retiredMediaIds = HashMap<Int, String>()
    private var plan: RidePlan? = null
    /** The playlist being ridden (a swap changes one entry of it). */
    private var ridePlaylist: HomePlaylist? = null
    /** What the ride was built from: the core finds its memory of found songs with it. */
    private var rideRequest: RideRequest? = null
    /** The "My rides" entry this ride came from (a swap keeps it up to date). */
    private var ridingSavedId: String? = null
    private var swapJob: Job? = null
    /** The ride's fade between songs, in seconds; 0 when off or the phone can't (one deck). */
    private var fadeSeconds = 0f
    /** The queue item whose next song was already buffered for the fade: one per item. */
    private var preparedItem = NO_ITEM
    private var chatSession: ExerciseSession? = null
    private var nextLineId = 0
    private var ownsHub = false
    private var lastClick = NO_CLICK
    /** The queue item already told to move on (the core said its part is over): one request per item. */
    private var skippedItem = NO_ITEM
    private var ticks = 0
    /** What the TV's lyrics were last set for (song, setting, TV running), to act once per change. */
    private var lyricsSig: LyricsSig? = null
    private var lyricsJob: Job? = null

    /** Artists the library has the most songs of in the genres picked (for "artists you like"). */
    private val _suggestions = MutableStateFlow<List<String>>(emptyList())
    val suggestions: StateFlow<List<String>> = _suggestions.asStateFlow()
    private var suggestionsJob: Job? = null

    private val listener = object : RideListener {
        override fun onProgress(progress: RideProgress) {
            _ui.update {
                when {
                    it is RideUi.Building -> it.copy(progress = progress)
                    it is RideUi.Preview && it.rebuilding != null -> it.copy(rebuilding = it.rebuilding.copy(progress = progress))
                    else -> it
                }
            }
        }
    }

    // ------------------------------------------------------------ screens

    /** The playback service hands over the TV server while it lives. */
    fun attachTv(host: RideTvHost) {
        tvHost.value = host
    }

    fun detachTv(host: RideTvHost) {
        if (tvHost.value === host) tvHost.value = null
    }

    /** The playback service hands over its crossfader while it lives (the ride's fade between songs). */
    fun attachFader(fader: RideFader) {
        this.fader = fader
    }

    fun detachFader(fader: RideFader) {
        if (this.fader === fader) this.fader = null
    }

    /** Shows the ride (or its summary) over the app; the ride keeps going when it is hidden again. */
    fun open() {
        _open.value = true
    }

    fun close() {
        _open.value = false
    }

    fun dismissNotice() {
        _notice.value = null
    }

    // -------------------------------------------------------------- setup

    fun updateForm(transform: (RideForm) -> RideForm) = _form.update(transform)

    /** The setup again (after a failed build, a preview or a summary). */
    fun backToSetup() {
        buildJob?.cancel()
        _ui.value = RideUi.Setup
    }

    private fun resetSetup() {
        buildJob?.cancel()
        chatSession = null
        _chat.value = RideChat()
        _form.value = newForm()
        _ui.value = RideUi.Setup
    }

    private fun newForm(): RideForm {
        val request = exerciseDefaultRequest(settings)
        return RideForm(minutes = request.minutes.toInt(), style = request.style, useAi = app.ai.hasProvider())
    }

    // --------------------------------------------------------------- chat

    /** Opens the chat with the user's AI (HomeFi's fixed opener, no network). */
    fun startChat() {
        if (_chat.value.started) return
        val ready = app.ai.ready() ?: return
        val session = ExerciseSession(ready.config, ready.key, _form.value.toRequest(settings.taste), languageTag())
        chatSession = session
        _chat.value = RideChat(thinking = true)
        scope.launch {
            val turn = session.start()
            showTurn(turn, applyRequest = false)
        }
    }

    fun say(text: String) {
        val session = chatSession ?: return
        val clean = text.trim()
        if (clean.isEmpty() || _chat.value.thinking) return
        _chat.update { it.copy(lines = it.lines + ChatLine(nextLineId++, true, clean), replies = emptyList(), thinking = true, error = null) }
        scope.launch {
            try {
                showTurn(session.reply(clean), applyRequest = true)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                _chat.update { it.copy(thinking = false, error = e.userMessage()) }
            }
        }
    }

    private fun showTurn(turn: ExerciseTurn, applyRequest: Boolean) {
        _chat.update {
            it.copy(lines = it.lines + ChatLine(nextLineId++, false, turn.text), replies = turn.quickReplies, thinking = false)
        }
        if (!applyRequest) return
        val request = turn.request
        _form.update {
            it.copy(
                minutes = request.minutes.toInt(),
                style = request.style,
                genres = request.wish.genres.toSet(),
                mix = request.wish.mix,
                sentence = request.wish.sentence,
                workout = request.workout,
            )
        }
    }

    // -------------------------------------------------------------- build

    /** Builds the ride of the form from the setup (the build screen shows the steps). */
    fun build() = launchBuild(inPlace = false)

    /**
     * "New songs" from the preview: the same ride built again while the plan stays on screen with the progress
     * in it. Everything is searched again and the AI is asked for other songs; a failure keeps this plan.
     */
    fun rebuild() = launchBuild(inPlace = _ui.value is RideUi.Preview)

    private fun launchBuild(inPlace: Boolean) {
        buildJob?.cancel()
        buildJob = scope.launch {
            val form = _form.value
            val request = form.toRequest(settings.taste)
            val ready = if (form.useAi) withContext(Dispatchers.IO) { app.ai.ready() } else null
            val building = RideUi.Building(null, ready?.info?.name)
            val previous = _ui.value as? RideUi.Preview
            _ui.value = if (inPlace && previous != null) previous.copy(rebuilding = building, rebuildError = null) else building
            try {
                val built = app.engine.exerciseBuild(
                    request, settings, ready?.config, ready?.key, languageTag(), libraryRefs(), listener,
                )
                // every successful build is kept in "My rides" so a good one can be ridden again without the AI
                val saved = app.rides.autoSave(request, built, languageTag(), settings.rideHistoryMax)
                _ui.value = RideUi.Preview(built, saved)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                val shown = _ui.value
                _ui.value = if (shown is RideUi.Preview) shown.copy(rebuilding = null, rebuildError = e.userMessage())
                else RideUi.Failed(e.userMessage())
            }
        }
    }

    /** Stops the build: back to the setup, or (for "new songs") to the plan that was on screen. */
    fun cancelBuild() {
        buildJob?.cancel()
        _ui.update { if (it is RideUi.Preview) it.copy(rebuilding = null) else RideUi.Setup }
    }

    // -------------------------------------------------------------- taste

    /** The taste answers are settings: every change is saved and the next ride starts from it. */
    fun updateTaste(transform: (RideTaste) -> RideTaste) {
        val current = settings
        app.settings.setExercise(current.copy(taste = transform(current.taste)))
    }

    /** Re-reads the artists to suggest for [genres] (off the main thread; the newest call wins). */
    fun loadSuggestions(genres: Set<String>) {
        suggestionsJob?.cancel()
        suggestionsJob = scope.launch {
            _suggestions.value = withContext(Dispatchers.Default) {
                runCatching { exerciseArtistSuggestions(app.library.refs(), genres.toList()) }.getOrDefault(emptyList())
            }
        }
    }

    private suspend fun libraryRefs(): List<LibraryRef> = withContext(Dispatchers.Default) { app.library.refs() }

    // ------------------------------------------------------------ saved rides

    /** Puts a saved ride's plan on the screen, ready to start: no build, no AI call. Only from the setup. */
    fun rideAgain(id: String) {
        if (_ui.value != RideUi.Setup) return
        scope.launch {
            val loaded = app.rides.load(id) ?: return@launch
            val request = loaded.ride.request
            _form.update {
                it.copy(
                    minutes = request.minutes.toInt(),
                    style = request.style,
                    genres = request.wish.genres.toSet(),
                    mix = request.wish.mix,
                    sentence = request.wish.sentence,
                    workout = request.workout,
                )
            }
            _ui.value = RideUi.Preview(loaded.built, loaded.ride.summary)
        }
    }

    /** "Keep" on the preview: pruning will never remove this ride. */
    fun keepSaved() = updateSaved { id -> app.rides.keep(id) }

    /** "Rename" on the preview (also keeps the ride). */
    fun renameSaved(name: String) = updateSaved { id -> app.rides.rename(id, name) }

    private fun updateSaved(change: suspend (String) -> SavedRideSummary?) {
        val preview = _ui.value as? RideUi.Preview ?: return
        val id = preview.saved?.id ?: return
        scope.launch {
            val saved = change(id) ?: return@launch
            _ui.update { if (it is RideUi.Preview && it.saved?.id == id) it.copy(saved = saved) else it }
        }
    }

    // --------------------------------------------------------------- ride

    /** Saves the ride as a HomeFi playlist, plays it and starts the clock (and the TV when the settings say so). */
    fun start(built: RideBuilt) {
        scope.launch {
            if (app.share.isHub || app.share.isGuest) {
                _ui.value = RideUi.Failed(app.getString(R.string.ex_busy_sharing))
                return@launch
            }
            val items = built.playlist.queue(app.library::find)
            if (items.items.isEmpty()) {
                _ui.value = RideUi.Failed(app.getString(R.string.ex_no_playable))
                return@launch
            }
            withContext(Dispatchers.IO) { app.playlists.save(built.playlist) }
            ridingSavedId = (_ui.value as? RideUi.Preview)?.saved?.id
            ridingSavedId?.let { app.rides.markRidden(it) }
            if (items.items.size < built.plan.songs.size) _notice.value = R.string.ex_some_skipped
            stopTv()
            plan = built.plan
            ridePlaylist = built.playlist
            rideRequest = _form.value.toRequest(settings.taste)
            itemToSong = IntArray(items.items.size) { items.entryOfItem(it) ?: it }
            rideMediaIds.clear()
            items.items.mapTo(rideMediaIds) { it.mediaId }
            retiredMediaIds.clear()
            swapJob?.cancel()
            _swapping.value = null
            ride = ExerciseRide(built.plan, settings.bike)
            fadeSeconds = settings.crossfadeSeconds.takeIf { fader?.setRideFade(it) == true } ?: 0f
            preparedItem = NO_ITEM
            cues.reset()
            lastClick = NO_CLICK
            skippedItem = NO_ITEM
            ticks = 0
            lyricsJob?.cancel()
            lyricsSig = null
            // on a TV the ride is drawn by this TV's own WebView, from the local server, and its music plays here
            val onTv = app.deviceForm == DeviceForm.TV
            val audioToTv = !onTv && settings.tvEnabled && settings.tvAudio == TvAudio.TV
            app.player.playItems(
                items.items, 0, false, built.playlist.source(), autoplay = !audioToTv, repeatMode = Player.REPEAT_MODE_OFF,
            )
            _ui.value = RideUi.Riding(built.plan)
            _open.value = true
            startLoop()
            if (settings.tvEnabled || onTv) startTv(built.plan, audioToTv, built.playlist.name)
        }
    }

    fun togglePause() {
        app.player.togglePlayPause()
    }

    /**
     * Puts the running ride on a TV when it was built without one: turns the TV setting on and starts the TV page
     * (pictures only, the music stays on the phone, pause or play as it is). A paired TV opens it by itself.
     */
    fun showOnTv() {
        val plan = plan ?: return
        if (_ui.value !is RideUi.Riding || _tv.value.active || _tv.value.waitingForTv) return
        if (!settings.tvEnabled) app.settings.setExercise(settings.copy(tvEnabled = true))
        scope.launch { startTv(plan, audioToTv = false, rideName = ridePlaylist?.name.orEmpty(), midRide = true) }
    }

    fun skipSegment() = app.player.next()

    /** The rider ended the ride: the music stops and the summary shows what was done. */
    fun end() {
        val elapsed = _frame.value?.view?.elapsedS?.toInt() ?: 0
        val ended = plan ?: return
        app.player.pause()
        finish(ended, elapsed, completed = false)
    }

    /** From the summary: the same setup again, ready for another ride. */
    fun again() {
        _open.value = false
        _ui.value = RideUi.Setup
    }

    /** From the summary: back to the setup and out. */
    fun leave() {
        _open.value = false
        resetSetup()
    }

    private fun finish(plan: RidePlan, elapsedS: Int, completed: Boolean) {
        loop?.cancel()
        loop = null
        swapJob?.cancel()
        _swapping.value = null
        fader?.setRideFade(0f)
        fadeSeconds = 0f
        cues.hold()
        _frame.value = null
        _ui.value = RideUi.Finished(plan, elapsedS, completed)
        _open.value = true
        scope.launch { stopTv() }
    }

    private fun startLoop() {
        loop?.cancel()
        loop = scope.launch {
            while (isActive) {
                tick()
                delay(TICK_MS)
            }
        }
    }

    private fun tick() {
        val player = app.player.player ?: return
        val ride = ride ?: return
        val plan = plan ?: return
        val itemIndex = player.currentMediaItemIndex.coerceAtLeast(0)
        // the rider started other music: the ride is over (the music is left alone)
        val mediaId = player.currentMediaItem?.mediaId
        if (mediaId != rideMediaIds.getOrNull(itemIndex)) {
            // a song was just swapped in under the player: wait for it to show up (the old one is not the ride's clock)
            if (mediaId != null && mediaId == retiredMediaIds[itemIndex]) return
            finish(plan, _frame.value?.view?.elapsedS?.toInt() ?: 0, completed = false)
            return
        }
        retiredMediaIds.remove(itemIndex)
        val songIndex = itemToSong.getOrNull(itemIndex) ?: itemIndex
        val positionMs = player.currentPosition.coerceAtLeast(0)
        val now = SystemClock.elapsedRealtime()
        val playing = player.isPlaying
        val view = ride.view(songIndex.toUInt(), positionMs.toULong(), now.toULong())
        _frame.value = RideFrame(view, playing, now, plan.songs.getOrNull(songIndex), plan.songs.getOrNull(songIndex + 1))

        val s = settings
        if (s.audibleCues) {
            if (playing) cues.schedule(view.cues, s.cueVolume) else cues.hold()
        }
        if (s.metronomeClick && playing) clickOnStroke(view, songIndex, s)
        syncTvLyrics(songIndex, itemIndex)
        if (++ticks % TV_PUBLISH_EVERY == 0) tvHost.value?.publish(ride, songIndex, positionMs, now, playing)
        if (fadeSeconds > 0f && playing && preparedItem != itemIndex && itemIndex < rideMediaIds.lastIndex &&
            view.remainingS.toLong() * 1000 <= RIDE_FADE_PREPARE_MS
        ) {
            // the next song buffers on the second deck a few seconds before the change; true once it is ready
            if (fader?.prepare(itemIndex + 1) == true) preparedItem = itemIndex
        }
        if (view.finished) {
            finish(plan, view.totalS.toInt(), completed = true)
        } else if (view.skipToNext && skippedItem != itemIndex) {
            // the song is longer than its part: move on at the planned time, once per song
            skippedItem = itemIndex
            if (itemIndex < rideMediaIds.lastIndex) {
                advance(player, itemIndex, positionMs, plan.songs.getOrNull(songIndex))
            } else {
                app.player.pause()
                finish(plan, view.totalS.toInt(), completed = true)
            }
        }
    }

    /** The part is over: the next song fades in over the ride's fade when it is ready, else the cut. */
    private fun advance(player: Player, itemIndex: Int, positionMs: Long, song: PlannedSong?) {
        val durationMs = player.duration.takeIf { it > 0 } ?: song?.entry?.durationMs?.toLong() ?: 0L
        val fadeMs = rideFadeMs(fadeSeconds, durationMs, positionMs)
        if (fadeMs > 0 && fader?.fadeTo(itemIndex + 1, fadeMs) == true) return
        app.player.next()
    }

    /** One click per pedal stroke, scheduled just ahead so it lands on the beat the pulse draws. */
    private fun clickOnStroke(view: RideView, songIndex: Int, s: ExerciseSettings) {
        val pulse = view.pulse
        if (pulse.strokeMs <= 0.0) return
        val position = pulse.songPositionMs.toDouble() - s.pulseOffsetMs
        val sinceFirst = position - pulse.firstBeatMs
        val stroke = ceil(sinceFirst / pulse.strokeMs)
        val inMs = stroke * pulse.strokeMs - sinceFirst
        val key = songIndex.toLong() * CLICK_SONG_STRIDE + stroke.toLong()
        if (inMs < TICK_MS * 1.5 && key != lastClick) {
            lastClick = key
            cues.click(inMs.toLong(), s.cueVolume * CLICK_GAIN)
        }
    }

    // --------------------------------------------------------------- swap

    /**
     * Puts another song in segment [segmentIndex] (the plan's song index). On the preview it replaces the song in
     * the plan. While riding, a song still to come is replaced in the queue; the song playing now ([current]) is
     * replaced by a clean cut (the new one starts at its beginning, the part's clock goes on). A song that is
     * already behind, or a ride that moved on while the search ran, is left alone. Progress is [swapping]; a
     * failure ("no other song", the setting off) goes to the notes in plain words and the plan stays as it was.
     */
    fun swapSong(segmentIndex: Int, current: Boolean) {
        if (_swapping.value != null) return
        val shown = _ui.value
        val riding = shown is RideUi.Riding
        if (!riding && shown !is RideUi.Preview) return
        swapJob = scope.launch {
            _swapping.value = segmentIndex
            try {
                if (shown is RideUi.Preview) swapInPreview(shown, segmentIndex) else swapWhileRiding(segmentIndex, current)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                app.notes.post(e.userMessage())
            } finally {
                _swapping.value = null
            }
        }
    }

    private suspend fun swapInPreview(preview: RideUi.Preview, segmentIndex: Int) {
        val request = _form.value.toRequest(settings.taste)
        val swapped = runSwap(request, preview.built, segmentIndex, atSegmentMs = 0L)
        // the plan on screen may have been replaced (a rebuild, back to the setup) while the song was searched
        _ui.update { if (it is RideUi.Preview && it.built === preview.built) it.copy(built = swapped) else it }
        preview.saved?.let { app.rides.replacePlan(it.id, swapped.plan, null) }
    }

    private suspend fun swapWhileRiding(segmentIndex: Int, current: Boolean) {
        val playlist = ridePlaylist ?: return
        val request = rideRequest ?: return
        val ridden = plan ?: return
        val song = ridden.songs.getOrNull(segmentIndex) ?: return
        val player = app.player.player ?: return
        val itemIndex = itemToSong.indexOf(segmentIndex)
        val playing = player.currentMediaItemIndex
        if (itemIndex < 0) {
            app.notes.post(app.getString(R.string.ex_no_playable))
            return
        }
        // the screen's "current song" is the player's truth: a tap on a song already gone by does nothing
        val live = itemIndex == playing
        if (itemIndex < playing || (current && !live)) return
        val startedAtMs = song.atSegmentMs.toLong() + if (live) player.currentPosition.coerceAtLeast(0) else 0L
        val swapped = runSwap(request, RideBuilt(ridden, playlist), segmentIndex, atSegmentMs = if (live) startedAtMs else 0L)

        // time went by while the song was searched: the ride may have moved on, and the part's clock moved with it
        val now = app.player.player ?: return
        val nowPlaying = now.currentMediaItemIndex
        if (plan !== ridden || (live && nowPlaying != itemIndex) || (!live && nowPlaying >= itemIndex)) return
        val entry = swapped.plan.songs.getOrNull(segmentIndex)?.entry ?: return
        val mediaItem = entryMediaItem(entry, app.library::find)?.withSource(swapped.playlist.source())
        if (mediaItem == null) {
            app.notes.post(app.getString(R.string.ex_no_playable))
            return
        }
        val newPlan = if (live) withLiveClock(swapped.plan, segmentIndex, song.atSegmentMs.toLong() + now.currentPosition.coerceAtLeast(0) - startedAtMs) else swapped.plan
        commitSwap(newPlan, swapped.playlist, itemIndex, live)
        now.replaceMediaItem(itemIndex, mediaItem)
        withContext(Dispatchers.IO) { app.playlists.save(swapped.playlist) }
        ridingSavedId?.let { app.rides.replacePlan(it, newPlan, swapped.playlist.id) }
    }

    /** Core call off the main thread's way: the AI only when the setting lets the swap ask it. */
    private suspend fun runSwap(request: RideRequest, built: RideBuilt, segmentIndex: Int, atSegmentMs: Long): RideBuilt {
        val s = settings
        val ready = if (s.swapUsesAi) withContext(Dispatchers.IO) { app.ai.ready() } else null
        return app.engine.exerciseSwap(
            request, built, segmentIndex.toUInt(), atSegmentMs.toULong(), s, ready?.config, ready?.key, languageTag(), libraryRefs(), listener,
        )
    }

    /** The song swapped in after [lateMs] of searching: its clock starts later and its cut comes that much sooner. */
    private fun withLiveClock(newPlan: RidePlan, segmentIndex: Int, lateMs: Long): RidePlan {
        if (lateMs <= 0) return newPlan
        val songs = newPlan.songs.toMutableList()
        val song = songs[segmentIndex]
        songs[segmentIndex] = song.copy(
            atSegmentMs = song.atSegmentMs + lateMs.toULong(),
            playMs = song.playMs?.let { (it.toLong() - lateMs).coerceAtLeast(MIN_CUT_MS).toULong() },
        )
        return newPlan.copy(songs = songs)
    }

    /** The ride goes on with the new plan: same queue slot, a ride object for the new plan, the clock and the fade start over for that song. */
    private fun commitSwap(newPlan: RidePlan, playlist: HomePlaylist, itemIndex: Int, live: Boolean) {
        val old = rideMediaIds.getOrNull(itemIndex)
        val entry = newPlan.songs.getOrNull(itemToSong.getOrNull(itemIndex) ?: itemIndex)?.entry
        plan = newPlan
        ridePlaylist = playlist
        ride = ExerciseRide(newPlan, settings.bike)
        _ui.update { if (it is RideUi.Riding) RideUi.Riding(newPlan) else it }
        preparedItem = NO_ITEM
        val newId = entryMediaItem(entry ?: return, app.library::find)?.mediaId ?: return
        rideMediaIds[itemIndex] = newId
        if (live) {
            if (old != null && old != newId) retiredMediaIds[itemIndex] = old
            skippedItem = NO_ITEM
            lyricsSig = null
        }
    }

    // --------------------------------------------------------- bike calibration

    /** The calibration's metronome: a click every turn at [rpm] (80 RPM = 750 ms) for [seconds], at the cue volume. */
    fun playClickTrain(seconds: Int, rpm: Int) {
        _calibrating.value = true
        cues.playClickTrain(seconds, rpm, settings.cueVolume) { _calibrating.value = false }
    }

    fun stopClickTrain() {
        cues.stopClickTrain()
        _calibrating.value = false
    }

    // ----------------------------------------------------------------- TV

    private data class LyricsSig(val songIndex: Int, val enabled: Boolean, val running: Boolean)

    /** Once per song (and when the setting or the TV state changes): clears the TV's lyrics, then loads and sends the song's. */
    private fun syncTvLyrics(songIndex: Int, itemIndex: Int) {
        val host = tvHost.value
        val sig = LyricsSig(songIndex, settings.tvLyrics, host?.running == true && _tv.value.active)
        val previous = lyricsSig
        if (sig == previous) return
        lyricsSig = sig
        lyricsJob?.cancel()
        if (host == null) return
        if (previous?.songIndex != songIndex || !sig.enabled) {
            if (sig.enabled || previous?.enabled == true) host.setLyrics("", null, 0, 0)
        }
        if (!sig.enabled || !sig.running) return
        val song = plan?.songs?.getOrNull(songIndex)?.entry ?: return
        val mediaId = rideMediaIds.getOrNull(itemIndex) ?: return
        val key = "$songIndex:${song.tidalId ?: song.title}"
        lyricsJob = scope.launch {
            val found = loadLyrics(mediaId) ?: return@launch
            if (lyricsSig?.songIndex != songIndex) return@launch
            host.setLyrics(key, found, app.settings.current.lyricsOffsetMs, song.durationMs.toLong())
        }
    }

    /** The song's lyrics through the app's cache (nothing is fetched twice); null when none, on failure or after [LYRICS_WAIT_MS]. */
    private suspend fun loadLyrics(mediaId: String): SongLyrics? = withContext(Dispatchers.IO) {
        try {
            app.lyrics.request(mediaId)
            val state = withTimeoutOrNull(LYRICS_WAIT_MS) {
                app.lyrics.states.first { it[mediaId].let { s -> s != null && s != LyricsState.Loading } }[mediaId]
            }
            (state as? LyricsState.Ready)?.lyrics?.let { l ->
                SongLyrics(l.lines.map { LyricLine(it.timeMs?.toULong(), it.text) }, l.synced, l.provider)
            }
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            null
        }
    }

    /** [midRide]: the ride is already going, so its music is left as it is (a paused ride stays paused). */
    private suspend fun startTv(plan: RidePlan, audioToTv: Boolean, rideName: String, midRide: Boolean = false) {
        val s = settings
        _tv.value = RideTvStatus(waitingForTv = audioToTv)
        val host = withTimeoutOrNull(HOST_WAIT_MS) { tvHost.filterNotNull().first() }
        if (host == null) {
            _tv.value = RideTvStatus(error = app.getString(R.string.ex_tv_unavailable))
            if (!midRide) app.player.player?.play()
            return
        }
        val name = app.share.displayName()
        val url = host.start(s.tvPort.toInt(), name, languageTag())
        if (url.isFailure) {
            _tv.value = RideTvStatus(error = url.exceptionOrNull()?.message)
            if (!midRide) app.player.player?.play()
            return
        }
        val listen = if (audioToTv) startAudioHub(plan, name) else null
        host.begin(
            TvSession(
                // the ride's own name on the TV ("30 min · Classic · Rock"), never the phone's model
                title = plan.workout.name.ifBlank { rideName },
                profile = plan.profile,
                parts = ride?.tvParts().orEmpty(),
                audio = if (listen != null) TvAudio.TV else TvAudio.PHONE,
                listen = listen,
                pulseStyle = s.pulseStyle,
                courseLook = s.courseLook,
                courseSummits = s.courseSummits,
                hotPoints = ride?.hotPoints().orEmpty(),
                courseRider = s.courseRider,
            ),
        )
        _tv.value = RideTvStatus(url = url.getOrNull(), audioOnTv = listen != null, waitingForTv = listen != null)
        // a paired TV opens the ride by itself (a TV that runs the ride has no one to tell)
        if (app.deviceForm != DeviceForm.TV) url.getOrNull()?.let(app.tvPairings::sendRide)
        if (listen == null) {
            if (audioToTv) _notice.value = R.string.ex_tv_audio_fallback
            if (!midRide) app.player.player?.play()
            return
        }
        // the music starts when the TV has joined (or after a while, on the phone)
        val joined = withTimeoutOrNull(TV_JOIN_WAIT_MS) {
            while (app.share.hub.value.guests.isEmpty()) delay(JOIN_POLL_MS)
            true
        } == true
        if (!joined) {
            app.share.setHubPlayHere(true)
            _notice.value = R.string.ex_tv_not_joined
        }
        _tv.update { it.copy(waitingForTv = false, audioOnTv = joined) }
        if (!midRide) app.player.player?.play()
    }

    /** The ride's own listen-only hub: the TV joins it with a one-ride code and plays the music on the shared clock. */
    private suspend fun startAudioHub(plan: RidePlan, name: String): RideListen? {
        val token = exerciseRideToken()
        app.share.startHub(
            party = true,
            mode = ShareNetworkMode.LAN,
            ride = RideHubTerms(token, plan.workout.name.ifBlank { name }, exerciseListenPermissions()),
        )
        val hub = withTimeoutOrNull(HUB_START_WAIT_MS) { app.share.hub.first { it.running || it.error != null } }
        val info = hub?.info?.takeIf { hub.running } ?: return null
        ownsHub = true
        return RideListen(info.port, token, info.sessionName)
    }

    private suspend fun stopTv() {
        tvHost.value?.let {
            it.clear()
            it.stop()
        }
        if (ownsHub) {
            ownsHub = false
            app.share.stopHub()
        }
        _tv.value = RideTvStatus()
    }

    private companion object {
        const val TICK_MS = 100L
        /** The TV polls at 2 Hz: publishing every third tick keeps it fresh without waste. */
        const val TV_PUBLISH_EVERY = 3
        const val LYRICS_WAIT_MS = 10_000L
        const val HOST_WAIT_MS = 5_000L
        const val HUB_START_WAIT_MS = 10_000L
        const val TV_JOIN_WAIT_MS = 30_000L
        const val JOIN_POLL_MS = 250L
        const val NO_CLICK = -1L
        const val NO_ITEM = -1
        /** A song swapped in late still plays at least this long before its cut. */
        const val MIN_CUT_MS = 1_000L
        const val CLICK_SONG_STRIDE = 1_000_000L
        /** The click is softer than the cues so it stays under the music. */
        const val CLICK_GAIN = 0.6f
    }
}
