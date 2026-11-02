package com.perseusoft.homefi.player

import com.perseusoft.homefi.ui.sound.autoScope
import androidx.media3.common.MediaItem
import androidx.media3.common.Player
import com.perseusoft.homefi.HomefiApplication
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.AssistantContext
import com.perseusoft.homefi.core.AssistantTurn
import com.perseusoft.homefi.core.EqAssistantSession
import com.perseusoft.homefi.core.EqProfile
import com.perseusoft.homefi.core.EqSettings
import com.perseusoft.homefi.core.ItemRef
import com.perseusoft.homefi.core.ListeningSummary
import com.perseusoft.homefi.core.LoudnessSettings
import com.perseusoft.homefi.core.NowPlayingContext
import com.perseusoft.homefi.core.ReactionContext
import com.perseusoft.homefi.core.ReplayGain
import com.perseusoft.homefi.core.SoundReaction
import com.perseusoft.homefi.core.SourceContext
import com.perseusoft.homefi.core.TasteCreator
import com.perseusoft.homefi.core.TasteIntensity
import com.perseusoft.homefi.core.TasteRule
import com.perseusoft.homefi.core.TasteScope
import com.perseusoft.homefi.core.TasteSettings
import com.perseusoft.homefi.core.Tilt
import com.perseusoft.homefi.core.SearchResults
import com.perseusoft.homefi.core.eqProfileFor
import com.perseusoft.homefi.core.loudnessGainDb
import com.perseusoft.homefi.core.readReplayGain
import com.perseusoft.homefi.core.readTrackInfo
import com.perseusoft.homefi.core.HomefiException
import com.perseusoft.homefi.core.TargetHit
import com.perseusoft.homefi.core.TargetMove
import com.perseusoft.homefi.core.TargetSource
import com.perseusoft.homefi.core.TasteAudition
import com.perseusoft.homefi.core.resolveTasteWith
import com.perseusoft.homefi.core.tasteAssign
import com.perseusoft.homefi.core.tasteAuditionApplies
import com.perseusoft.homefi.core.tasteAuditionFor
import com.perseusoft.homefi.core.tasteKeep
import com.perseusoft.homefi.core.tasteRankTargets
import com.perseusoft.homefi.core.tasteSave
import com.perseusoft.homefi.core.tasteSetOutputs
import com.perseusoft.homefi.core.tasteSoundCovering
import com.perseusoft.homefi.core.tasteUnassign
import com.perseusoft.homefi.core.tasteUndoMoves
import com.perseusoft.homefi.core.soundReact
import com.perseusoft.homefi.core.soundUndoOn
import com.perseusoft.homefi.core.TasteChange
import com.perseusoft.homefi.core.tasteApplyChange
import com.perseusoft.homefi.core.tasteCurveSummary
import com.perseusoft.homefi.core.tasteNewRule
import com.perseusoft.homefi.core.tasteRemove
import com.perseusoft.homefi.core.tasteScopes
import com.perseusoft.homefi.core.tasteSplitGenres
import com.perseusoft.homefi.data.userMessage
import com.perseusoft.homefi.ui.eq.eqIdKind
import com.perseusoft.homefi.ui.player.label
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import java.io.File
import java.util.Locale
import com.perseusoft.homefi.core.AudioOutput as AssistantOutput
import com.perseusoft.homefi.core.OutputKind as AssistantOutputKind

/** What the taste layers know about the track playing now, for the player and the EQ screen. */
data class NowSound(
    val mediaId: String? = null,
    val title: String? = null,
    val context: NowPlayingContext? = null,
    /** Scopes a rule for this track can have, most specific first. */
    val scopes: List<TasteScope> = emptyList(),
    /** The sound that plays (null: none, or Sound is off). While trying it is the tried one. */
    val rule: TasteRule? = null,
    /** The target that made [rule] play (the song, the album, the artist...). */
    val target: TasteScope? = null,
    /** The output playing, as the equalizer names it ([Equalizer.outputId]). */
    val output: String? = null,
    /** [rule] is being tried (not saved). */
    val auditioning: Boolean = false,
    /** For each of [scopes], the id of the sound that holds it on this output (null: none). */
    val coveredBy: Map<TasteScope, String?> = emptyMap(),
    /** The equalizer profile's name on this output while the equalizer works ("" = unnamed). */
    val eqProfile: String? = null,
    /** Loudness normalization's gain for this track (0 = none). */
    val loudnessDb: Float = 0f,
    /** Any filter runs: not bit-perfect. */
    val active: Boolean = false,
)

/**
 * A one-line note after a reaction (with its undo), a move of targets between sounds
 * (undone with [moves]), an undo or a failure.
 */
data class SoundNotice(
    val message: String,
    val undoToken: String? = null,
    val error: Boolean = false,
    val moves: List<TargetMove> = emptyList(),
)

/** What "Try" did. */
sealed interface TryResult {
    data object Playing : TryResult
    data object SoundOff : TryResult
    data object NothingPlaying : TryResult
}

/** How the assistant should start next time it opens. */
sealed interface AssistantHandoff {
    /** Go on from a suggestion card: its session and turn, and the answer tapped. */
    class Suggestion(val session: EqAssistantSession, val turn: AssistantTurn, val reply: String) : AssistantHandoff

    /**
     * Start over with these notes ("Sound for this album → Ask the assistant"); with
     * [focusRule] the AI starts from that rule's curve ("Ask the AI to refine it").
     */
    data class Notes(val text: String, val focusRule: String? = null) : AssistantHandoff
}

/** The suggestion card of the taste rules tab. */
sealed interface SuggestionState {
    data object Idle : SuggestionState
    data object Loading : SuggestionState
    class Ready(val turn: AssistantTurn, val session: EqAssistantSession) : SuggestionState
    /** "Not now", answered, failed or turned off: gone until the app restarts. */
    data object Hidden : SuggestionState
}

/** "Sound ▸": the sheet that moves [scope] to one of the sounds. */
data class SoundMenuRequest(val scope: TasteScope)

/** Tags and ReplayGain of a file, read off the main thread once per track. */
private data class TrackExtras(
    val replayGain: ReplayGain?,
    val genres: List<String> = emptyList(),
    val albumArtist: String = "",
)

/** The track playing as the service's player knows it. */
private data class Playing(
    val mediaId: String,
    val title: String,
    val artist: String,
    val album: String,
    val albumArtist: String,
    val genre: String,
    val source: SourceContext?,
    val shuffle: Boolean,
)

/**
 * Taste layers and loudness: on every track change (and every settings change) works out
 * what plays (genres, artists, album, where the queue came from), the rule that applies,
 * the loudness gain from the file's ReplayGain and hands it all to [EqRuntime], which the
 * audio processor glides into. Also "How does it sound?" (reactions with undo), the rules
 * themselves, the listening summary and suggestion for the assistant.
 */
class SoundLayers(
    private val app: HomefiApplication,
    private val equalizer: Equalizer,
    private val outputs: AudioOutputs,
    private val scope: CoroutineScope,
) {
    private val store get() = app.settings
    val taste: StateFlow<TasteSettings> get() = store.taste
    val loudness: StateFlow<LoudnessSettings> get() = store.loudness

    private val playing = MutableStateFlow<Playing?>(null)
    private val extrasTick = MutableStateFlow(0)
    /** By media id; a queue rarely holds more than this many that played. */
    private val extras = object : LinkedHashMap<String, TrackExtras>(64, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, TrackExtras>?) = size > 64
    }
    private val reading = HashSet<String>()

    private val _now = MutableStateFlow(NowSound())
    val now: StateFlow<NowSound> = _now.asStateFlow()

    private val _busy = MutableStateFlow<SoundReaction?>(null)
    /** The reaction being worked out (one at a time). */
    val busy: StateFlow<SoundReaction?> = _busy.asStateFlow()

    private val _notices = MutableSharedFlow<SoundNotice>(extraBufferCapacity = 4)
    val notices: SharedFlow<SoundNotice> = _notices.asSharedFlow()

    private val _soundMenu = MutableStateFlow<SoundMenuRequest?>(null)
    val soundMenu: StateFlow<SoundMenuRequest?> = _soundMenu.asStateFlow()

    private val _audition = MutableStateFlow<TasteAudition?>(null)
    /** The sound being tried on the song playing (not saved); ends with the song. */
    val audition: StateFlow<TasteAudition?> = _audition.asStateFlow()

    private val _onlyThisOutput = MutableStateFlow(false)
    /** "Only on this output" for the sounds kept next. */
    val onlyThisOutput: StateFlow<Boolean> = _onlyThisOutput.asStateFlow()

    private val _lastMoves = MutableStateFlow<List<TargetMove>>(emptyList())
    /** Targets taken from other sounds by the last assignment (the Undo snackbar). */
    val lastMoves: StateFlow<List<TargetMove>> = _lastMoves.asStateFlow()

    private val _assistantOpen = MutableStateFlow(false)
    /** The "Ask the AI" bottom sheet is open. */
    val assistantOpen: StateFlow<Boolean> = _assistantOpen.asStateFlow()

    private val _howOpen = MutableStateFlow(false)
    /** The "How does it sound?" sheet is open. */
    val howOpen: StateFlow<Boolean> = _howOpen.asStateFlow()

    private val _suggestion = MutableStateFlow<SuggestionState>(SuggestionState.Idle)
    val suggestion: StateFlow<SuggestionState> = _suggestion.asStateFlow()

    @Volatile private var handoff: AssistantHandoff? = null
    private var summary: Pair<Long, ListeningSummary>? = null

    init {
        scope.launch {
            val eqSide = combine(store.eq, equalizer.outputId, equalizer.held) { eq, id, held -> Triple(eq, id, held) }
            val trackSide = combine(playing, extrasTick) { p, _ -> p }
            val soundSide = combine(store.taste, _audition) { t, a -> t to a }
            combine(eqSide, soundSide, store.loudness, trackSide) { e, s, l, p -> Inputs(e.first, e.second, e.third, s.first, l, p, s.second) }
                .collect(::apply)
        }
    }

    private data class Inputs(
        val eq: EqSettings,
        val outputId: String,
        val held: Boolean,
        val taste: TasteSettings,
        val loudness: LoudnessSettings,
        val playing: Playing?,
        val audition: TasteAudition? = null,
    )

    @Volatile private var last: Inputs? = null

    /** What [p] runs with the current settings: its context, taste rule and loudness gain. */
    private fun inputOf(i: Inputs, p: Playing?): Pair<SoundInput, Float> {
        val ex = p?.let { synchronized(extras) { extras[it.mediaId] } }
        val ctx = p?.let { contextOf(it, ex) } ?: EMPTY
        val albumInOrder = p?.source?.kind == "album" && !p.shuffle
        val gain = ex?.replayGain?.let { runCatching { loudnessGainDb(i.loudness, it, albumInOrder) }.getOrNull() } ?: 0f
        // a try ends with its song: only the track it was started on hears it
        val audition = i.audition?.takeIf { p != null && runCatching { tasteAuditionApplies(it, ctx) }.getOrDefault(false) }
        return SoundInput(i.eq, i.taste, i.outputId, ctx, gain, i.held, audition) to gain
    }

    /**
     * The filters for [item] on the other deck (crossfade): the song coming in keeps its own
     * taste and loudness while both play. Its tags are read now if they weren't yet.
     */
    fun inputFor(item: MediaItem, shuffle: Boolean): SoundInput? {
        val i = last ?: return null
        readExtras(item.mediaId)
        return inputOf(i, playingOf(item, shuffle)).first
    }

    /** What [item] needs to know for the crossfade: its album and (album) artist. */
    fun fadeInfo(item: MediaItem): Pair<ItemRef?, ItemRef?> {
        val p = playingOf(item, false)
        val album = p.album.takeIf { it.isNotBlank() }?.let { ItemRef(id = "", name = it) }
        val artist = (p.albumArtist.ifBlank { p.artist.split(';', '/').firstOrNull().orEmpty() }).trim()
            .takeIf { it.isNotEmpty() }?.let { ItemRef(id = "", name = it) }
        return album to artist
    }

    private fun apply(i: Inputs) {
        last = i
        val p = i.playing
        val (input, gain) = inputOf(i, p)
        val ctx = input.now
        EqRuntime.configure(input)
        val active = runCatching { input.copy(held = false).biquads(48_000).isNotEmpty() }.getOrDefault(false)
        equalizer.publishActive(active)
        if (i.audition != null && input.audition == null) _audition.value = null
        val resolved = if (!i.eq.enabled) null else runCatching { resolveTasteWith(i.taste, i.outputId, input.audition, ctx) }.getOrNull()
        val rule = resolved?.rule
        val scopes = if (p == null) emptyList() else runCatching { tasteScopes(ctx) }.getOrDefault(emptyList())
        val profile = if (i.eq.enabled) runCatching { eqProfileFor(i.eq, i.outputId) }.getOrNull()?.takeIf { it.bands.any { b -> b.enabled } } else null
        _now.value = NowSound(
            mediaId = p?.mediaId,
            title = p?.title,
            context = p?.let { ctx },
            scopes = scopes,
            rule = rule,
            target = resolved?.target,
            output = i.outputId,
            auditioning = resolved?.auditioning == true,
            coveredBy = scopes.associateWith { sc -> runCatching { tasteSoundCovering(i.taste, sc, i.outputId) }.getOrNull()?.id },
            eqProfile = profile?.name?.trim(),
            loudnessDb = gain,
            active = active,
        )
    }

    // ---------------------------------------------------------------- tracks

    /**
     * The service's player moved to another track, or its queue / shuffle changed. Called on
     * the main thread right at the transition, so the new filters glide in at the boundary
     * (the next tracks' tags are read ahead, so nothing waits for the disk then).
     */
    fun onTrack(player: Player) {
        val item = player.currentMediaItem
        playing.value = item?.let { playingOf(it, player.shuffleModeEnabled) }
        val ahead = buildList {
            item?.let(::add)
            val next = player.nextMediaItemIndex
            if (next != androidx.media3.common.C.INDEX_UNSET && next < player.mediaItemCount) add(player.getMediaItemAt(next))
        }
        ahead.forEach { readExtras(it.mediaId) }
    }

    /** A Tidal song was fetched into the stream cache: its tags (genre, ReplayGain) can be read now. */
    fun streamReady(id: String, path: String) = readExtras("tidal:$id", path, force = true)

    private fun playingOf(item: MediaItem, shuffle: Boolean): Playing {
        val m = item.mediaMetadata
        val track = if (isFilePath(item.mediaId)) app.library.find(item.mediaId) else null
        return Playing(
            mediaId = item.mediaId,
            title = m.title?.toString() ?: track?.title.orEmpty(),
            artist = m.artist?.toString() ?: track?.artist.orEmpty(),
            album = m.albumTitle?.toString() ?: track?.album.orEmpty(),
            albumArtist = m.albumArtist?.toString() ?: track?.albumArtist.orEmpty(),
            genre = m.genre?.toString() ?: track?.genre.orEmpty(),
            source = item.source,
            shuffle = shuffle,
        )
    }

    private fun readExtras(mediaId: String, knownPath: String? = null, force: Boolean = false) {
        synchronized(extras) {
            if (!force && (extras.containsKey(mediaId) || mediaId in reading)) return
            reading += mediaId
        }
        scope.launch(Dispatchers.IO) {
            val path = knownPath ?: when {
                isFilePath(mediaId) -> mediaId
                else -> tidalId(mediaId)?.let(::cachedStream)
            }
            val ex = path?.let { p ->
                val info = if (isFilePath(mediaId)) null else runCatching { readTrackInfo(p) }.getOrNull()
                TrackExtras(
                    replayGain = runCatching { readReplayGain(p) }.getOrNull(),
                    genres = info?.genre?.let { runCatching { tasteSplitGenres(it) }.getOrNull() }.orEmpty(),
                    albumArtist = info?.albumArtist.orEmpty(),
                )
            }
            synchronized(extras) {
                reading -= mediaId
                // a stream not fetched yet: try again once it is (see streamReady)
                if (ex != null) extras[mediaId] = ex
            }
            if (ex != null && playing.value?.mediaId == mediaId) extrasTick.update { it + 1 }
        }
    }

    private fun cachedStream(id: String): String? =
        File(app.cacheDir, "stream").listFiles()?.firstOrNull { it.nameWithoutExtension == id && it.length() > 0 }?.absolutePath

    private fun contextOf(p: Playing, ex: TrackExtras?): NowPlayingContext {
        val source = p.source
        val artists = mutableListOf<ItemRef>()
        for (name in p.artist.split(';', '/') + p.albumArtist.ifBlank { ex?.albumArtist.orEmpty() }) {
            val n = name.trim()
            if (n.isNotEmpty() && artists.none { it.name.equals(n, ignoreCase = true) }) {
                val id = if (source?.kind == "artist" && source.name.equals(n, ignoreCase = true)) source.id else ""
                artists += ItemRef(id = id, name = n)
            }
        }
        val genres = p.genre.takeIf { it.isNotBlank() }?.let { runCatching { tasteSplitGenres(it) }.getOrNull() }
            ?: ex?.genres.orEmpty()
        val albumId = if (source?.kind == "album" && source.name.equals(p.album, ignoreCase = true)) source.id else ""
        return NowPlayingContext(
            trackId = tidalId(p.mediaId),
            path = p.mediaId.takeIf(::isFilePath),
            title = p.title.ifBlank { null },
            album = p.album.takeIf { it.isNotBlank() }?.let { ItemRef(id = albumId, name = it) },
            artists = artists,
            genres = genres,
            source = source,
        )
    }

    // ------------------------------------------------------------- reactions

    fun openHow(open: Boolean) {
        _howOpen.value = open
    }

    /**
     * "How does it sound?": one short AI call when a provider is set up (else fixed safe
     * steps); the result is saved and heard at once, and a note says what changed, with Undo.
     * Runs in the app's scope, so closing the sheet doesn't stop it.
     */
    fun react(reaction: SoundReaction, scopeHint: TasteScope?) {
        val ctx = _now.value.context ?: return
        if (_busy.value != null) return
        _busy.value = reaction
        scope.launch {
            try {
                val ready = app.ai.ready()
                val r = soundReact(
                    reaction,
                    ReactionContext(
                        language = language(),
                        nowPlaying = ctx,
                        taste = taste.value,
                        loudness = loudness.value,
                        outputKind = outputKind(),
                        scopeHint = scopeHint,
                        activeOutput = equalizer.outputId.value,
                    ),
                    ready?.config,
                    ready?.key,
                )
                if (r.undoToken.isNotEmpty()) {
                    r.tasteChange?.let(::applyTasteChange)
                    store.setLoudness(r.loudness)
                }
                _howOpen.value = false
                _notices.emit(SoundNotice(r.message, r.undoToken.ifEmpty { null }))
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                _notices.emit(SoundNotice(e.userMessage(), error = true))
            } finally {
                _busy.value = null
            }
        }
    }

    fun undo(token: String) {
        val before = runCatching { soundUndoOn(token, taste.value) }.getOrNull()
        if (before != null) {
            before.tasteChange?.let(::applyTasteChange)
            store.setLoudness(before.loudness)
        }
        _notices.tryEmit(SoundNotice(app.getString(if (before != null) R.string.sound_undone else R.string.sound_already_undone)))
    }

    /** What plays the music, as the core's reactions and assistant name it. */
    fun outputKind(): AssistantOutputKind =
        (outputs.current.value?.kind ?: eqIdKind(equalizer.outputId.value)).forAssistant()

    // ---------------------------------------------------------------- sounds

    fun setIntensity(intensity: TasteIntensity) = store.setTaste(taste.value.copy(intensity = intensity))

    fun setLoudness(next: LoudnessSettings) = store.setLoudness(next)

    fun setOnlyThisOutput(on: Boolean) {
        _onlyThisOutput.value = on
    }

    /** The output a sound made now is limited to ("Only on this output"), else null. */
    private fun onlyOn(): String? = equalizer.outputId.value.takeIf { _onlyThisOutput.value }

    // ------------------------------------------------------------------ try / keep

    /** Plays [rule] on the song playing now (nothing is saved); it ends with the song. */
    fun tryNow(rule: TasteRule): TryResult {
        if (!store.eq.value.enabled) return TryResult.SoundOff
        val ctx = _now.value.context ?: return TryResult.NothingPlaying
        val audition = runCatching { tasteAuditionFor(rule, ctx) }.getOrNull() ?: return TryResult.NothingPlaying
        _audition.value = audition
        return TryResult.Playing
    }

    fun stopTrying() {
        _audition.value = null
    }

    /**
     * Where "Keep" puts the sound tried: the scope the sound already holds for this song,
     * else the artist / genre the core suggests ([autoScope]).
     */
    fun suggestedScope(rule: TasteRule): TasteScope? {
        val now = _now.value
        val own = now.scopes.firstOrNull { now.coveredBy[it] == rule.id }
        // "Keep for artist" first (the mockup): what a listener names a sound after
        return own ?: now.scopes.firstOrNull { it is TasteScope.Artist } ?: autoScope(now.scopes)
    }

    /** Keeps the sound being tried for [scope] (the core assigns or creates it). */
    fun keep(scope: TasteScope): TasteRule? {
        val rule = _audition.value?.rule ?: return null
        return keepRule(rule, scope)
    }

    /** Keeps a proposal (an assistant's or a reaction's sound) for [scope]. */
    fun keepRule(rule: TasteRule, scope: TasteScope): TasteRule? {
        val before = taste.value
        return try {
            val r = tasteKeep(before, rule, scope, onlyOn(), TasteCreator.MANUAL)
            store.setTaste(r.taste)
            _audition.value = null
            announce(before, r.moves)
            r.rule
        } catch (e: HomefiException) {
            _notices.tryEmit(SoundNotice(e.userMessage(), error = true))
            null
        }
    }

    // ------------------------------------------------------------------ edit

    /** Puts [scope] in the sound [soundId]; what it takes from other sounds is announced with Undo. */
    fun assign(soundId: String, scope: TasteScope) {
        val before = taste.value
        try {
            val r = tasteAssign(before, soundId, scope, TasteCreator.MANUAL)
            store.setTaste(r.taste)
            announce(before, r.moves)
        } catch (e: HomefiException) {
            _notices.tryEmit(SoundNotice(e.userMessage(), error = true))
        }
    }

    fun unassign(soundId: String, scope: TasteScope) {
        store.setTaste(tasteUnassign(taste.value, soundId, scope))
    }

    /** Takes [scope] out of the sound [soundId] and says so, with Undo. */
    fun removeTarget(soundId: String, scope: TasteScope) {
        val label = labelOf(soundId)
        unassign(soundId, scope)
        _notices.tryEmit(SoundNotice(app.getString(R.string.sound_removed_from, scope.name(), label), moves = listOf(TargetMove(scope, soundId))))
    }

    fun undoMoves(moves: List<TargetMove>) {
        if (moves.isEmpty()) return
        store.setTaste(tasteUndoMoves(taste.value, moves))
        _lastMoves.value = emptyList()
    }

    /** The outputs a sound plays on (empty = all). */
    fun setOutputs(soundId: String, outputs: List<String>) {
        val before = taste.value
        try {
            val r = tasteSetOutputs(before, soundId, outputs)
            store.setTaste(r.taste)
            announce(before, r.moves)
        } catch (e: HomefiException) {
            _notices.tryEmit(SoundNotice(e.userMessage(), error = true))
        }
    }

    /** Saves [rule] (new, renamed or with another curve); the core clamps it and settles its targets. */
    fun save(rule: TasteRule): TasteRule? {
        val before = taste.value
        return runCatching { tasteSave(before, rule) }.getOrNull()?.let { r ->
            store.setTaste(r.taste)
            announce(before, r.moves)
            r.rule
        }
    }

    /** A new, flat sound for [scopes] (saved: the user shapes it in Edit sound). */
    fun newSound(scopes: List<TasteScope>, label: String? = null, createdBy: TasteCreator = TasteCreator.MANUAL): TasteRule? {
        val name = label?.trim()?.ifEmpty { null } ?: scopes.firstOrNull()?.name()?.ifBlank { null } ?: app.getString(R.string.sound_new_default)
        val rule = runCatching { tasteNewRule(scopes, Tilt(0f, 0f, null), name, createdBy) }.getOrNull() ?: return null
        return save(rule.copy(outputs = listOfNotNull(onlyOn())))
    }

    fun remove(id: String) = store.setTaste(tasteRemove(taste.value, id))

    /** Says what moved between sounds ("Eagles moves from “Warm”"), with Undo. */
    private fun announce(before: TasteSettings, moves: List<TargetMove>) {
        if (moves.isEmpty()) return
        _lastMoves.value = moves
        val labelOf = { id: String -> before.rules.firstOrNull { it.id == id }?.label.orEmpty() }
        val text = moves.joinToString("\n") { m -> app.getString(R.string.sound_moves_from, m.target.name(), labelOf(m.from)) }
        _notices.tryEmit(SoundNotice(text, moves = moves))
    }

    /** A one-line note over every screen. */
    fun notify(message: String) {
        _notices.tryEmit(SoundNotice(message))
    }

    /** The label of a sound by id (empty when it is gone). */
    fun labelOf(id: String): String = taste.value.rules.firstOrNull { it.id == id }?.label.orEmpty()

    /** A rule in one line, from the core: `3 bands · −2 dB 3.2k, +1.5 dB 120 Hz · +2 dB bass`. */
    fun ruleWords(rule: TasteRule): String = runCatching { tasteCurveSummary(rule, language()) }.getOrDefault(rule.label)

    /** The output's own profile under the taste layers (when the equalizer is on). */
    fun deviceProfile(): EqProfile? {
        val eq = store.eq.value
        return if (eq.enabled) runCatching { eqProfileFor(eq, equalizer.outputId.value) }.getOrNull() else null
    }

    /** The music plays on the phone's own speaker: no deep bass in taste curves. */
    fun smallSpeaker(): Boolean = outputKind() == AssistantOutputKind.BUILT_IN

    /** "Ask the AI to refine it": the assistant starts from this sound's curve. */
    fun refine(rule: TasteRule) {
        _howOpen.value = false
        handoff = AssistantHandoff.Notes(app.getString(R.string.taste_refine_notes, rule.label, ruleWords(rule)), rule.id)
        openAssistant(true)
    }

    // ------------------------------------------------- "Sound ▸" and the assistant

    /** Opens the "Sound ▸" sheet for [scope] (null closes it). */
    fun openMenu(scope: TasteScope?) {
        _soundMenu.value = scope?.let(::SoundMenuRequest)
    }

    fun openAssistant(open: Boolean) {
        _assistantOpen.value = open
    }

    /** Opens the assistant about one scope ("New sound from AI…"). */
    fun askAssistant(scope: TasteScope) {
        val id = scope.id().takeIf { it.isNotBlank() }?.let { " (id: $it)" }.orEmpty()
        askAssistantWith(app.getString(R.string.sound_ask_notes, phrase(scope)) + id)
    }

    /** Opens the assistant sheet starting from [text] (null: where it was). */
    fun askAssistantWith(text: String?) {
        _howOpen.value = false
        _soundMenu.value = null
        if (text != null) handoff = AssistantHandoff.Notes(text)
        openAssistant(true)
    }

    /**
     * What "Add" can offer for [query]: the library (tracks, albums, artists, genres, playlists;
     * prefiltered and capped) plus Tidal when signed in (5 s at most; a failure leaves the
     * library only), ranked by the core. Cancel it when the query changes.
     */
    suspend fun searchTargets(query: String): List<TargetHit> = withContext(Dispatchers.Default) {
        val q = query.trim()
        if (q.isEmpty()) return@withContext emptyList()
        val hits = ArrayList<TargetHit>(MAX_LIBRARY_CANDIDATES + 40)
        val library = app.library.state.value
        fun hit(scope: TasteScope, subtitle: String) = TargetHit(scope, subtitle, TargetSource.LIBRARY, null)
        val seen = HashSet<String>()
        fun add(scope: TasteScope, subtitle: String) {
            if (hits.size < MAX_LIBRARY_CANDIDATES && seen.add("${scope::class.simpleName}:${scope.id()}:${scope.name().lowercase()}")) hits += hit(scope, subtitle)
        }
        for (t in library.tracks) {
            if (hits.size >= MAX_LIBRARY_CANDIDATES) break
            if (t.title.contains(q, ignoreCase = true)) add(TasteScope.Track(t.path, t.title), t.artist)
            if (t.album.contains(q, ignoreCase = true)) add(TasteScope.Album("", t.album), t.albumArtist.ifBlank { t.artist })
            val artist = t.albumArtist.ifBlank { t.artist }
            if (artist.contains(q, ignoreCase = true)) add(TasteScope.Artist("", artist), "")
            if (t.genre.contains(q, ignoreCase = true)) {
                runCatching { tasteSplitGenres(t.genre) }.getOrDefault(emptyList()).filter { it.contains(q, ignoreCase = true) }.forEach { add(TasteScope.Genre(it), "") }
            }
        }
        library.playlists.filter { it.name.contains(q, ignoreCase = true) }.forEach { add(TasteScope.Playlist("", it.name), "") }
        if (app.account.isLoggedIn) {
            val found: SearchResults? = withTimeoutOrNull(TIDAL_SEARCH_TIMEOUT_MS) { runCatching { app.engine.search(q) }.getOrNull() }
            found?.let { r ->
                r.tracks.forEach { hits += TargetHit(TasteScope.Track(it.id, it.title), it.subtitle, TargetSource.TIDAL, null) }
                r.albums.forEach { hits += TargetHit(TasteScope.Album(it.id, it.title), it.subtitle, TargetSource.TIDAL, null) }
                r.artists.forEach { hits += TargetHit(TasteScope.Artist(it.id, it.title), "", TargetSource.TIDAL, null) }
                r.playlists.forEach { hits += TargetHit(TasteScope.Playlist(it.id, it.title), it.subtitle, TargetSource.TIDAL, null) }
            }
        }
        runCatching { tasteRankTargets(q, hits, taste.value, equalizer.outputId.value, MAX_TARGET_RESULTS) }.getOrDefault(emptyList())
    }

    /** The assistant takes what was left for it (once). */
    fun takeHandoff(): AssistantHandoff? = handoff.also { handoff = null }

    /** A taste change the assistant agreed with the user: saved and heard at once. */
    fun applyAssistant(change: TasteChange?, loudness: LoudnessSettings?) {
        change?.let(::applyTasteChange)
        loudness?.let(store::setLoudness)
    }

    /**
     * Merges a change from the core (assistant, reaction, undo) into the rules as they are
     * NOW, so a rule deleted meanwhile never comes back.
     */
    private fun applyTasteChange(change: TasteChange) {
        store.setTaste(tasteApplyChange(taste.value, change))
    }

    /**
     * Tells a conversation (a suggestion card's lives all app run) the rules, loudness and
     * equalizer as they are now; call before every reply.
     */
    suspend fun syncSession(session: EqAssistantSession) {
        session.sync(taste.value, loudness.value, store.eq.value)
    }

    /** Loads the suggestion card once per app run (one AI call at most). */
    fun loadSuggestion() {
        if (_suggestion.value != SuggestionState.Idle) return
        if (!store.current.soundSuggestions) return
        _suggestion.value = SuggestionState.Loading
        scope.launch {
            val ready = app.ai.ready()
            val result = runCatching {
                val ctx = assistantContext(null)
                val session = if (ready != null) {
                    EqAssistantSession(ready.config, ready.key, ctx, app.autoeqCacheDir)
                } else {
                    EqAssistantSession.guided(ctx, app.autoeqCacheDir)
                }
                SuggestionState.Ready(session.suggest(), session)
            }
            _suggestion.value = result.getOrNull()?.takeIf { it.turn.text.isNotBlank() } ?: SuggestionState.Hidden
        }
    }

    fun hideSuggestion() {
        (_suggestion.value as? SuggestionState.Ready)?.session?.close()
        _suggestion.value = SuggestionState.Hidden
    }

    /** An answer on the suggestion card: the assistant goes on from it. */
    fun answerSuggestion(reply: String) {
        val s = _suggestion.value as? SuggestionState.Ready ?: return
        handoff = AssistantHandoff.Suggestion(s.session, s.turn, reply)
        _suggestion.value = SuggestionState.Hidden
        openAssistant(true)
    }

    /** Everything the assistant knows before the first question. */
    suspend fun assistantContext(notes: String?, focusRule: String? = null): AssistantContext {
        val connected = outputs.outputs.value.map { o ->
            AssistantOutput(id = o.eqId, name = o.name.ifBlank { app.getString(o.kind.label()) }, kind = o.kind.forAssistant(), sampleRates = o.sampleRates.map { it.toUInt() })
        }
        val nowId = equalizer.outputId.value
        val all = if (connected.any { it.id == nowId }) {
            // the one playing first: the assistant asks about it
            connected.sortedBy { if (it.id == nowId) 0 else 1 }
        } else {
            val kind = eqIdKind(nowId)
            listOf(AssistantOutput(id = nowId, name = nowId.substringAfter(':', "").ifBlank { app.getString(kind.label()) }, kind = kind.forAssistant(), sampleRates = emptyList())) + connected
        }
        return AssistantContext(
            language = language(),
            outputs = all,
            current = store.eq.value,
            notes = notes,
            taste = taste.value,
            loudness = loudness.value,
            listening = listening(),
            nowPlaying = _now.value.context,
            activeOutput = nowId,
            focusRule = focusRule,
        )
    }

    /**
     * What the user listens to most: library genres and artists by track count, Tidal
     * favorite artists (five tracks each) and playlists. First pages only, cached half an hour.
     */
    suspend fun listening(): ListeningSummary = withContext(Dispatchers.Default) {
        summary?.takeIf { System.currentTimeMillis() - it.first < SUMMARY_TTL_MS }?.let { return@withContext it.second }
        val genres = HashMap<String, Pair<String, Int>>()
        val artists = HashMap<String, Pair<String, Int>>()
        for (t in app.library.state.value.tracks) {
            for (g in runCatching { tasteSplitGenres(t.genre) }.getOrDefault(emptyList())) {
                val k = g.lowercase()
                genres[k] = (genres[k]?.first ?: g) to ((genres[k]?.second ?: 0) + 1)
            }
            val a = t.albumArtist.trim().ifEmpty { t.artist.trim() }
            if (a.isNotEmpty()) artists[a.lowercase()] = (artists[a.lowercase()]?.first ?: a) to ((artists[a.lowercase()]?.second ?: 0) + 1)
        }
        val ids = HashMap<String, String>()
        val playlists = mutableListOf<ItemRef>()
        if (app.account.isLoggedIn) {
            withTimeoutOrNull(8_000) {
                runCatching { app.engine.favoriteArtistsPage(0u).items }.getOrNull()?.forEach { a ->
                    ids[a.title.lowercase()] = a.id
                    artists[a.title.lowercase()] = (artists[a.title.lowercase()]?.first ?: a.title) to ((artists[a.title.lowercase()]?.second ?: 0) + 5)
                }
                runCatching { app.engine.myPlaylistsPage(0u).items }.getOrNull()?.take(20)?.forEach { playlists += ItemRef(id = it.id, name = it.title) }
            }
        }
        app.library.state.value.playlists.take(20).forEach { playlists += ItemRef(id = "", name = it.name) }
        fun top(m: Map<String, Pair<String, Int>>, n: Int) =
            m.values.sortedWith(compareByDescending<Pair<String, Int>> { it.second }.thenBy { it.first }).take(n).map { it.first }
        val s = ListeningSummary(
            topGenres = top(genres, 6),
            topArtists = top(artists, 8).map { ItemRef(id = ids[it.lowercase()].orEmpty(), name = it) },
            playlists = playlists.take(20),
        )
        summary = System.currentTimeMillis() to s
        s
    }

    // ---------------------------------------------------------------- words

    /** "the album Discovery", "Electronic music". */
    fun phrase(s: TasteScope): String = when (s) {
        is TasteScope.Genre -> app.getString(R.string.sound_phrase_genre, s.name)
        is TasteScope.Album -> app.getString(R.string.sound_phrase_album, s.name)
        is TasteScope.Artist -> app.getString(R.string.sound_phrase_artist, s.name)
        is TasteScope.Playlist -> app.getString(R.string.sound_phrase_playlist, s.name)
        is TasteScope.Track -> app.getString(R.string.sound_phrase_track, s.name)
    }

    companion object {
        private const val SUMMARY_TTL_MS = 30 * 60 * 1000L
        private const val MAX_LIBRARY_CANDIDATES = 200
        private const val MAX_TARGET_RESULTS = 30u
        private const val TIDAL_SEARCH_TIMEOUT_MS = 5_000L
        private val EMPTY = NowPlayingContext(trackId = null, path = null, title = null, album = null, artists = emptyList(), genres = emptyList(), source = null)

        fun language(): String = if (Locale.getDefault().language == "es") "es" else "en"

        fun OutputKind.forAssistant(): AssistantOutputKind = when (this) {
            OutputKind.USB -> AssistantOutputKind.USB_DAC
            OutputKind.WIRED -> AssistantOutputKind.HEADPHONES
            OutputKind.BLUETOOTH -> AssistantOutputKind.BLUETOOTH
            OutputKind.SPEAKER -> AssistantOutputKind.BUILT_IN
            OutputKind.HDMI -> AssistantOutputKind.HDMI
            OutputKind.LINE, OutputKind.OTHER -> AssistantOutputKind.OTHER
        }

        /** Same thing: same kind and id, or the same name when an id is missing (local tags). */
        fun sameScope(a: TasteScope, b: TasteScope): Boolean {
            if (a::class != b::class) return false
            val (ia, ib) = a.id().trim() to b.id().trim()
            if (a !is TasteScope.Genre && ia.isNotEmpty() && ib.isNotEmpty()) return ia == ib
            return a.name().trim().equals(b.name().trim(), ignoreCase = true)
        }
    }
}

fun TasteScope.name(): String = when (this) {
    is TasteScope.Genre -> name
    is TasteScope.Album -> name
    is TasteScope.Artist -> name
    is TasteScope.Playlist -> name
    is TasteScope.Track -> name
}

fun TasteScope.id(): String = when (this) {
    is TasteScope.Genre -> ""
    is TasteScope.Album -> id
    is TasteScope.Artist -> id
    is TasteScope.Playlist -> id
    is TasteScope.Track -> id
}
