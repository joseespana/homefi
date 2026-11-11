package com.perseusoft.homefi.ui.playlists

import com.perseusoft.homefi.HomefiApplication
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.ChatBrief
import com.perseusoft.homefi.core.HomePlaylist
import com.perseusoft.homefi.core.PartyBrief
import com.perseusoft.homefi.core.PartyBuilt
import com.perseusoft.homefi.core.PartyListener
import com.perseusoft.homefi.core.PartyProgress
import com.perseusoft.homefi.core.PartySession
import com.perseusoft.homefi.core.PartyTurn
import com.perseusoft.homefi.core.PlaylistOrigin
import com.perseusoft.homefi.core.SongSeed
import com.perseusoft.homefi.core.chatSongWish
import com.perseusoft.homefi.core.chatDefaultBrief
import com.perseusoft.homefi.core.partyDefaultBrief
import com.perseusoft.homefi.data.userMessage
import com.perseusoft.homefi.ui.eq.ChatLine
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** User turns the optional chat takes; mirrors the core's `MAX_CHAT_TURNS` (the core also enforces it). */
internal const val MAX_CHAT_TURNS = 6

/** User turns of the free "Playlist with AI" chat; mirrors the core's `MAX_FREE_CHAT_TURNS`. */
internal const val MAX_FREE_CHAT_TURNS = 12

/** Which AI-made playlist the screen is making: from tapped answers, or from a free chat. */
enum class PartyMode(val maxTurns: Int) {
    PARTY(MAX_CHAT_TURNS),
    CHAT(MAX_FREE_CHAT_TURNS),
}

/** The optional chat before (and between) builds. */
data class PartyChat(
    val lines: List<ChatLine> = emptyList(),
    val replies: List<String> = emptyList(),
    /** The AI is answering: Build waits. */
    val thinking: Boolean = false,
    val userTurns: Int = 0,
    val maxTurns: Int = MAX_CHAT_TURNS,
) {
    val started: Boolean get() = lines.isNotEmpty() || thinking
    val closed: Boolean get() = userTurns >= maxTurns
}

/**
 * The answers plus the chat; every state after [PartyUi.NeedsAi] carries them. In chat mode [brief]
 * is unused and [chatBrief] is what the AI understood so far (null until the opener arrives).
 */
data class PartyForm(
    val brief: PartyBrief,
    val chat: PartyChat = PartyChat(),
    val error: String? = null,
    val chatBrief: ChatBrief? = null,
)

/** What the party screen is doing. */
sealed interface PartyUi {
    /** No AI set up (or its key is missing): the screen only offers to set one up. */
    data object NeedsAi : PartyUi

    data class Asking(val form: PartyForm) : PartyUi

    /** [progress] is null until the core reports the first step; [previous] is the draft being replaced. */
    data class Building(val form: PartyForm, val progress: PartyProgress?, val aiName: String, val previous: PartyBuilt?) : PartyUi

    /** A draft, or (once [saved]) the playlist as stored. */
    data class Built(val form: PartyForm, val built: PartyBuilt, val saved: Boolean = false) : PartyUi
}

/** What a conversation is doing, in the words of the chats menu. */
sealed interface ChatStatus {
    /** Open, nothing in flight: it waits for the user. */
    data object Waiting : ChatStatus
    data object Thinking : ChatStatus

    /** [fraction] is null until the core reports how many songs it found. */
    data class Building(val fraction: Float?) : ChatStatus
    data object Ready : ChatStatus
    data class Failed(val message: String) : ChatStatus
}

/** The menu's status for a state; [buildFailure] is the last build's error while no newer action cleared it. */
internal fun chatStatus(ui: PartyUi, buildFailure: String?): ChatStatus = when (ui) {
    PartyUi.NeedsAi -> ChatStatus.Waiting
    is PartyUi.Building -> {
        val p = ui.progress as? PartyProgress.Resolving
        ChatStatus.Building(p?.takeIf { it.total > 0u }?.let { it.done.toFloat() / it.total.toFloat() })
    }
    is PartyUi.Built -> ChatStatus.Ready
    is PartyUi.Asking -> when {
        buildFailure != null -> ChatStatus.Failed(buildFailure)
        ui.form.chat.thinking -> ChatStatus.Thinking
        else -> ChatStatus.Waiting
    }
}

/**
 * One AI playlist conversation (party answers or free chat): the answers, the optional chat with
 * the user's AI and the build. It lives in [AiConversations] (application scope), so it keeps
 * going when its screen closes. The session closes with [close]; the listener runs on a Rust
 * thread, so it only touches the thread-safe state flow. A cancelled coroutine aborts the Rust task.
 */
class PartyConversation internal constructor(
    val id: String,
    val mode: PartyMode,
    private val app: HomefiApplication,
    parent: Job?,
) {
    private val scope = CoroutineScope(SupervisorJob(parent) + Dispatchers.Main.immediate)
    private val _ui = MutableStateFlow<PartyUi>(PartyUi.NeedsAi)
    val ui: StateFlow<PartyUi> = _ui.asStateFlow()

    private val _title = MutableStateFlow(
        app.getString(if (mode == PartyMode.CHAT) R.string.pl_chat_title else R.string.pl_party_title),
    )
    /** What the core calls this conversation (the AI's title, the wish's first words, the party name). */
    val title: StateFlow<String> = _title.asStateFlow()

    private val _lastActivityMs = MutableStateFlow(System.currentTimeMillis())
    val lastActivityMs: StateFlow<Long> = _lastActivityMs.asStateFlow()

    private val _buildFailure = MutableStateFlow<String?>(null)
    /** The last build's error until the next action (the chat stays usable). */
    val buildFailure: StateFlow<String?> = _buildFailure.asStateFlow()

    val status: ChatStatus get() = chatStatus(_ui.value, _buildFailure.value)

    /** No AI is set up: the screen only offers to set one up, and the chat doesn't count toward the cap. */
    @Volatile
    var unavailable = false
        private set

    private var session: PartySession? = null
    private var aiName: String = ""
    private var replaceId: String? = null
    private var opened = false
    private var closed = false
    private var job: Job? = null
    private var briefJob: Job? = null
    private var seedJob: Job? = null
    private var songSeed: SongSeed? = null
    private var nextLineId = 0

    /** True once the user (or the answers of a replaced playlist) saved the draft. */
    val isSaved: Boolean get() = (_ui.value as? PartyUi.Built)?.saved == true

    private val listener = object : PartyListener {
        override fun onProgress(progress: PartyProgress) {
            _ui.update { if (it is PartyUi.Building) it.copy(progress = progress) else it }
        }
    }

    private fun touch() {
        _lastActivityMs.value = System.currentTimeMillis()
    }

    /**
     * Starts the screen. With [replace] (a saved playlist of the same [mode]) the answers are its
     * brief and a build saves over it; otherwise they come from the Settings defaults. In chat mode
     * the opener shows at once, and a [seed] (a song to make a playlist like) is then sent as the
     * first message. Runs once per conversation.
     */
    fun open(replace: HomePlaylist?, seed: SongSeed? = null) {
        if (opened) return
        opened = true
        songSeed = seed.takeIf { mode == PartyMode.CHAT }
        replaceId = replace?.id
        when (mode) {
            PartyMode.PARTY -> startSession((replace?.origin as? PlaylistOrigin.Party)?.origin?.brief, null)
            PartyMode.CHAT -> startSession(null, (replace?.origin as? PlaylistOrigin.Chat)?.origin?.brief)
        }
    }

    /** Looks at the AI choice again (after the setup sheet closed). */
    fun refreshAi() {
        if (_ui.value is PartyUi.NeedsAi) startSession(null, null)
    }

    private fun startSession(savedParty: PartyBrief?, savedChat: ChatBrief?) {
        scope.launch {
            val ready = withContext(Dispatchers.IO) { app.ai.ready() }
            unavailable = ready == null
            if (ready == null) {
                _ui.value = PartyUi.NeedsAi
                return@launch
            }
            val settings = app.settings.playlists.value
            val brief = savedParty ?: partyDefaultBrief(settings)
            val chatBrief = if (mode == PartyMode.CHAT) savedChat ?: chatDefaultBrief(settings) else null
            val fresh = withContext(Dispatchers.IO) {
                if (chatBrief != null) PartySession.newChat(ready.config, ready.key, chatBrief, languageTag())
                else PartySession(ready.config, ready.key, brief, languageTag())
            }
            if (closed) {
                fresh.close()
                return@launch
            }
            session?.close()
            session = fresh
            aiName = ready.info.name
            runCatching { fresh.title() }.onSuccess { _title.value = it }
            _ui.value = PartyUi.Asking(PartyForm(brief, PartyChat(maxTurns = mode.maxTurns), chatBrief = chatBrief))
            if (mode == PartyMode.CHAT) {
                talk()
                sendSongWish()
            }
        }
    }

    /** Once the opener is in, the core words the seed song as the user's first message and it goes out like typed text. */
    private fun sendSongWish() {
        val seed = songSeed ?: return
        songSeed = null
        seedJob = scope.launch {
            job?.join()
            val wish = withContext(Dispatchers.Default) { chatSongWish(seed, languageTag()) }
            send(wish)
        }
    }

    /** The answers changed: shown at once, handed to the session (chat stays) in order. */
    fun setBrief(brief: PartyBrief) {
        _ui.update { state ->
            when (state) {
                is PartyUi.Asking -> state.copy(form = state.form.copy(brief = brief))
                is PartyUi.Built -> state.copy(form = state.form.copy(brief = brief))
                else -> state
            }
        }
        val current = session ?: return
        val before = briefJob
        briefJob = scope.launch {
            before?.join()
            runCatching { current.setBrief(brief) }
                .onSuccess { runCatching { current.title() }.onSuccess { t -> _title.value = t } }
        }
    }

    /** "Talk it over with the AI first": the AI's first question. */
    fun talk() {
        val form = formOrNull() ?: return
        if (form.chat.started) return
        turn(form) { it.start() }
    }

    fun send(text: String) {
        val form = formOrNull() ?: return
        val answer = text.trim()
        if (answer.isEmpty() || form.chat.thinking || form.chat.closed) return
        val user = ChatLine(nextLineId++, true, answer)
        turn(form, user) { it.reply(answer) }
    }

    private fun turn(form: PartyForm, user: ChatLine? = null, call: suspend (PartySession) -> PartyTurn) {
        val current = session ?: return
        val chat = form.chat
        val asked = chat.copy(
            lines = if (user != null) chat.lines + user else chat.lines,
            replies = emptyList(),
            thinking = true,
            userTurns = if (user != null) chat.userTurns + 1 else chat.userTurns,
        )
        updateForm { it.copy(chat = asked, error = null) }
        _buildFailure.value = null
        touch()
        job?.cancel()
        job = scope.launch {
            try {
                briefJob?.join()
                val reply = call(current)
                if (reply.title.isNotBlank()) _title.value = reply.title
                touch()
                updateForm {
                    val line = ChatLine(nextLineId++, false, reply.text)
                    it.copy(
                        chat = it.chat.copy(lines = it.chat.lines + line, replies = reply.quickReplies, thinking = false),
                        chatBrief = reply.chatBrief ?: it.chatBrief,
                    )
                }
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                updateForm { it.copy(chat = it.chat.copy(thinking = false), error = e.userMessage()) }
            }
        }
    }

    /** Asks the AI for songs and checks them against the library and Tidal. Replaces a draft. */
    fun build() {
        val state = _ui.value
        val form = formOrNull() ?: return
        if (form.chat.thinking || !canBuild(form)) return
        val previous = (state as? PartyUi.Built)?.built
        val current = session ?: return
        job?.cancel()
        _buildFailure.value = null
        touch()
        _ui.value = PartyUi.Building(form.copy(error = null), null, aiName, previous)
        job = scope.launch {
            try {
                briefJob?.join()
                if (mode == PartyMode.PARTY) current.setBrief(form.brief)
                val built = app.engine.partyBuild(current, app.settings.playlists.value, app.libraryRefs(), replaceId, listener)
                val id = replaceId
                if (id != null) {
                    val error = app.playlists.save(built.playlist)
                    if (error != null) _buildFailure.value = error
                    _ui.value = if (error == null) PartyUi.Built(form, built, saved = true) else PartyUi.Asking(form.copy(error = error))
                } else {
                    _ui.value = PartyUi.Built(form, built)
                }
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                val message = e.userMessage()
                _buildFailure.value = message
                _ui.value = PartyUi.Asking(form.copy(error = message))
            }
            touch()
        }
    }

    /** Stops a build or a turn; the answers and the chat stay (a replaced draft comes back). */
    fun cancel() {
        seedJob?.cancel()
        job?.cancel()
        _ui.update { state ->
            when (state) {
                is PartyUi.Building ->
                    state.previous?.let { PartyUi.Built(state.form, it) } ?: PartyUi.Asking(state.form)
                is PartyUi.Asking -> state.copy(form = state.form.copy(chat = state.form.chat.copy(thinking = false)))
                else -> state
            }
        }
    }

    /** Saves the draft in HomeFi (only there; nothing goes to Tidal). */
    fun save() {
        val built = (_ui.value as? PartyUi.Built)?.takeIf { !it.saved } ?: return
        scope.launch {
            val error = app.playlists.save(built.built.playlist)
            _ui.value = if (error == null) built.copy(saved = true) else built.copy(form = built.form.copy(error = error))
        }
    }

    /** Drops the draft; the answers and the chat stay. */
    fun discard() {
        val state = _ui.value as? PartyUi.Built ?: return
        _ui.value = PartyUi.Asking(state.form)
    }

    /** A chat playlist needs at least one message from the user; a party one can build from its answers. */
    private fun canBuild(form: PartyForm) = mode == PartyMode.PARTY || form.chat.userTurns > 0

    private fun formOrNull(): PartyForm? = when (val state = _ui.value) {
        is PartyUi.Asking -> state.form
        is PartyUi.Built -> state.form
        else -> null
    }

    private fun updateForm(change: (PartyForm) -> PartyForm) {
        _ui.update { state ->
            when (state) {
                is PartyUi.Asking -> state.copy(form = change(state.form))
                is PartyUi.Built -> state.copy(form = change(state.form))
                is PartyUi.Building -> state.copy(form = change(state.form))
                else -> state
            }
        }
    }

    /** Stops everything and frees the session; the conversation is not used afterwards. */
    fun close() {
        if (closed) return
        closed = true
        scope.cancel()
        session?.close()
        session = null
    }
}
