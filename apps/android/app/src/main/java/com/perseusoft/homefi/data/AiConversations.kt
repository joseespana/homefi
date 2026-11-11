package com.perseusoft.homefi.data

import com.perseusoft.homefi.HomefiApplication
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.HomePlaylist
import com.perseusoft.homefi.core.PlaylistSettings
import com.perseusoft.homefi.core.SongSeed
import com.perseusoft.homefi.core.tooManyChatsMessage
import com.perseusoft.homefi.ui.playlists.ChatRequest
import com.perseusoft.homefi.ui.playlists.ChatRequests
import com.perseusoft.homefi.ui.playlists.ChatStatus
import com.perseusoft.homefi.ui.playlists.PartyConversation
import com.perseusoft.homefi.ui.playlists.PartyMode
import com.perseusoft.homefi.ui.playlists.PartyUi
import com.perseusoft.homefi.ui.playlists.languageTag
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.util.UUID
import java.util.concurrent.ConcurrentHashMap

/** What the cap needs to know about a chat. */
internal data class ChatEntry(val id: String, val status: ChatStatus, val lastActivityMs: Long)

/** The chat to close to make room: the oldest one that failed; never a build, a draft or a chat in progress. */
internal fun evictable(chats: List<ChatEntry>): String? =
    chats.filter { it.status is ChatStatus.Failed }.minByOrNull { it.lastActivityMs }?.id

/**
 * The AI playlist chats (party and free chat) that are open. They live as long as the process, so
 * leaving a screen doesn't stop them; the user closes them from the chats menu. At the cap
 * (`chatMaxOpen`) the oldest failed chat makes room, else the new one is refused with a notice.
 * Everything here is in memory only.
 */
class AiConversations(
    private val app: HomefiApplication,
    private val scope: CoroutineScope,
    private val settings: StateFlow<PlaylistSettings>,
    private val notes: Notes,
    private val requests: ChatRequests,
) {
    private val _list = MutableStateFlow<List<PartyConversation>>(emptyList())

    /** Open chats, oldest first (including ones that only offer to set an AI up; the menu leaves those out). */
    val list: StateFlow<List<PartyConversation>> = _list.asStateFlow()

    /** The chat whose screen is showing: it gets no "ready" notice. */
    val visibleId = MutableStateFlow<String?>(null)

    /** The chats menu is open. */
    val menuOpen = MutableStateFlow(false)

    private val watchers = ConcurrentHashMap<String, Job>()

    /** Any chat is thinking or building (the dot on the menu's icon). */
    @OptIn(ExperimentalCoroutinesApi::class)
    val busy: StateFlow<Boolean> = _list.flatMapLatest { chats ->
        if (chats.isEmpty()) flowOf(false)
        else combine(chats.map { it.ui }) { states ->
            states.any { it is PartyUi.Building || (it is PartyUi.Asking && it.form.chat.thinking) }
        }
    }.stateIn(scope, SharingStarted.Eagerly, false)

    fun get(id: String): PartyConversation? = _list.value.firstOrNull { it.id == id }

    /**
     * Starts a chat and returns it, or null when the cap leaves no room (a notice then says so,
     * with a way to the menu). Call it from the main thread.
     */
    fun open(mode: PartyMode, replace: HomePlaylist? = null, seed: SongSeed? = null): PartyConversation? {
        val max = settings.value.chatMaxOpen.toInt()
        val counted = _list.value.filterNot { it.unavailable }
        if (counted.size >= max) {
            val victim = evictable(counted.map { ChatEntry(it.id, it.status, it.lastActivityMs.value) })
            if (victim == null) {
                refuse(max)
                return null
            }
            close(victim)
        }
        val chat = PartyConversation(UUID.randomUUID().toString(), mode, app, scope.coroutineContext[Job])
        _list.update { it + chat }
        watch(chat)
        chat.open(replace, seed)
        return chat
    }

    /** Cancels the chat's work, frees its session and drops it from the list. */
    fun close(id: String) {
        val chat = get(id) ?: return
        _list.update { chats -> chats.filterNot { it.id == id } }
        watchers.remove(id)?.cancel()
        chat.close()
        if (_list.value.isEmpty()) menuOpen.value = false
    }

    /** The user left the chat's screen: one that only offered to set an AI up, or whose playlist is saved, is done. */
    fun leave(id: String) {
        if (get(id)?.let { it.unavailable || it.isSaved } == true) close(id)
    }

    private fun refuse(max: Int) {
        val text = tooManyChatsMessage(languageTag(), max.toUByte())
        notes.post(text, NoteAction(app.getString(R.string.pl_chats_open)) { menuOpen.value = true })
    }

    /** Posts a notice when a build ends (done or failed) while the chat's screen isn't showing. */
    private fun watch(chat: PartyConversation) {
        watchers[chat.id] = scope.launch {
            var before: PartyUi = chat.ui.value
            chat.ui.collect { now ->
                val from = before
                before = now
                if (from is PartyUi.Building) {
                    when {
                        now is PartyUi.Built && now.built !== from.previous -> done(chat, failure = null)
                        now is PartyUi.Asking && chat.buildFailure.value != null -> done(chat, chat.buildFailure.value)
                    }
                }
            }
        }
    }

    private fun done(chat: PartyConversation, failure: String?) {
        if (!settings.value.chatDoneNotice || visibleId.value == chat.id) return
        val title = chat.title.value
        val text = if (failure == null) app.getString(R.string.pl_chat_ready, title)
        else app.getString(R.string.pl_chat_failed, title, failure)
        notes.post(text, NoteAction(app.getString(R.string.pl_chat_open)) { requests.open(ChatRequest.Resume(chat.id)) })
    }
}
