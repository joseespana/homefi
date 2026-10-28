package com.perseusoft.homefi.ui.eq

import android.app.Application
import android.content.Context
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.AssistantTurn
import com.perseusoft.homefi.core.EqAssistantSession
import com.perseusoft.homefi.core.EqProfile
import com.perseusoft.homefi.core.EqProposal
import com.perseusoft.homefi.core.eqDefaultSettings
import com.perseusoft.homefi.core.eqSettingsFromJson
import com.perseusoft.homefi.core.eqSettingsToJson
import com.perseusoft.homefi.data.userMessage
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.AssistantHandoff
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject

data class ChatLine(val id: Int, val fromUser: Boolean, val text: String)

data class AssistantUi(
    val lines: List<ChatLine> = emptyList(),
    val replies: List<String> = emptyList(),
    val proposal: EqProposal? = null,
    /** "Keep talking" put the proposal aside. */
    val proposalAside: Boolean = false,
    val typing: Boolean = false,
    val error: String? = null,
    val ai: Boolean = false,
    /** Provider name in AI mode. */
    val aiName: String? = null,
    val done: Boolean = false,
    /** Output id the last profile was applied to. */
    val applied: String? = null,
)

/**
 * "Set up my sound": an [EqAssistantSession] with the AI the user set up (key from the
 * Keystore) or the guided wizard. The conversation is saved after every turn (the core's
 * state, which has no key, plus what's on screen) so it continues after the app is killed.
 * A confirmed proposal is saved for its output and the equalizer turned on.
 */
class AssistantViewModel(app: Application) : AndroidViewModel(app) {
    private val home = app.homefi
    private val prefs = app.getSharedPreferences("eq_assistant", Context.MODE_PRIVATE)
    private val _ui = MutableStateFlow(AssistantUi())
    val ui: StateFlow<AssistantUi> = _ui.asStateFlow()

    private var session: EqAssistantSession? = null
    private var job: Job? = null
    private var nextId = 0
    /** What to call again on "Try again": null = start, else the reply that failed. */
    private var pending: String? = null

    /** Resuming or starting the saved conversation; a handoff replaces it. */
    private val opening: Job = viewModelScope.launch { open() }

    private suspend fun open() {
        val saved = prefs.getString(STATE, null)
        val ready = home.ai.ready()
        if (saved != null) {
            val resumed = withContext(Dispatchers.Default) {
                runCatching { EqAssistantSession.resume(saved, home.autoeqCacheDir, ready?.config, ready?.key) }.getOrNull()
            }
            // an AI conversation whose key is gone starts over rather than half-resuming
            if (resumed != null && resumed.isAi() == prefs.getBoolean(AI, false)) {
                session = resumed
                restore(resumed.isAi(), ready?.info?.name)
                return
            }
            resumed?.close()
        }
        start()
    }

    fun restart(notes: String? = null) {
        job?.cancel()
        prefs.edit().clear().apply()
        session?.close()
        session = null
        viewModelScope.launch { start(notes) }
    }

    /**
     * The screen opened: a suggestion card's answer goes on in its own session, and
     * "Sound for this… → Ask the assistant" starts over about that music.
     */
    fun takeHandoff() {
        val h = home.sound.takeHandoff() ?: return
        opening.cancel()
        when (h) {
            is AssistantHandoff.Notes -> restart(h.text)
            is AssistantHandoff.Suggestion -> {
                job?.cancel()
                prefs.edit().clear().apply()
                session?.close()
                session = h.session
                nextId = 0
                val ai = h.session.isAi()
                _ui.value = AssistantUi(
                    lines = listOf(ChatLine(nextId++, false, h.turn.text)),
                    ai = ai,
                    aiName = home.ai.ready()?.info?.name?.takeIf { ai },
                )
                send(h.reply)
            }
        }
    }

    private suspend fun start(notes: String? = null) {
        val ready = home.ai.ready()
        val context = home.sound.assistantContext(notes)
        val s = withContext(Dispatchers.Default) {
            if (ready != null) {
                EqAssistantSession(ready.config, ready.key, context, home.autoeqCacheDir)
            } else {
                EqAssistantSession.guided(context, home.autoeqCacheDir)
            }
        }
        session = s
        nextId = 0
        _ui.value = AssistantUi(ai = s.isAi(), aiName = ready?.info?.name?.takeIf { s.isAi() })
        pending = null
        turn(null) { it.start() }
    }

    fun send(text: String) {
        val t = text.trim()
        if (t.isEmpty() || _ui.value.typing || session == null) return
        _ui.update { it.copy(lines = it.lines + ChatLine(nextId++, true, t), proposalAside = true) }
        turn(t) { it.reply(t) }
    }

    fun retry() {
        val text = pending
        if (text == null) turn(null) { it.start() } else turn(text) { it.reply(text) }
    }

    /** The proposal's Apply: saved and on now, and the assistant is told yes. */
    fun apply() {
        val p = _ui.value.proposal ?: return
        home.equalizer.applyProposal(p.outputId, p.profile)
        _ui.update { it.copy(applied = p.outputId) }
        send(getApplication<Application>().getString(R.string.assistant_apply_reply))
    }

    fun keepTalking() = _ui.update { it.copy(proposalAside = true) }

    private fun turn(text: String?, call: suspend (EqAssistantSession) -> AssistantTurn) {
        val s = session ?: return
        pending = text
        job = viewModelScope.launch {
            _ui.update { it.copy(typing = true, error = null, replies = emptyList()) }
            try {
                handle(call(s))
                save(s)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                _ui.update { it.copy(typing = false, error = e.userMessage()) }
            }
        }
    }

    private fun handle(turn: AssistantTurn) {
        val before = _ui.value
        val proposal = turn.proposal ?: before.proposal.takeIf { turn.confirmed }
        var applied = before.applied
        if (turn.confirmed && proposal != null) {
            home.equalizer.applyProposal(proposal.outputId, proposal.profile)
            applied = proposal.outputId
        }
        // taste rules or loudness the user agreed to: saved and heard at once
        if (turn.taste != null || turn.loudness != null) home.sound.applyAssistant(turn.taste, turn.loudness)
        _ui.update {
            it.copy(
                lines = if (turn.text.isBlank()) it.lines else it.lines + ChatLine(nextId++, false, turn.text),
                replies = turn.quickReplies,
                proposal = proposal,
                proposalAside = turn.proposal == null || turn.confirmed,
                typing = false,
                done = turn.done,
                applied = applied,
            )
        }
    }

    // ------------------------------------------------------------ saving

    private suspend fun save(s: EqAssistantSession) {
        val state = s.stateJson()
        val ui = _ui.value
        val lines = JSONArray().apply { ui.lines.forEach { put(JSONObject().put("u", it.fromUser).put("t", it.text)) } }
        prefs.edit()
            .putString(STATE, state)
            .putBoolean(AI, ui.ai)
            .putString(LINES, lines.toString())
            .putString(REPLIES, JSONArray(ui.replies).toString())
            .putBoolean(DONE, ui.done)
            .putString(APPLIED, ui.applied)
            .putString(PROPOSAL, ui.proposal?.let(::proposalJson))
            .putBoolean(ASIDE, ui.proposalAside)
            .apply()
    }

    private fun restore(ai: Boolean, aiName: String?) {
        val lines = runCatching {
            val a = JSONArray(prefs.getString(LINES, "[]"))
            (0 until a.length()).map { i -> a.getJSONObject(i).let { ChatLine(i, it.getBoolean("u"), it.getString("t")) } }
        }.getOrDefault(emptyList())
        nextId = lines.size
        val replies = runCatching {
            val a = JSONArray(prefs.getString(REPLIES, "[]"))
            (0 until a.length()).map(a::getString)
        }.getOrDefault(emptyList())
        _ui.value = AssistantUi(
            lines = lines,
            replies = replies,
            proposal = prefs.getString(PROPOSAL, null)?.let(::proposalOf),
            proposalAside = prefs.getBoolean(ASIDE, true),
            ai = ai,
            aiName = aiName.takeIf { ai },
            done = prefs.getBoolean(DONE, false),
            applied = prefs.getString(APPLIED, null),
        )
    }

    override fun onCleared() {
        session?.close()
    }

    private companion object {
        const val STATE = "state"
        const val AI = "ai"
        const val LINES = "lines"
        const val REPLIES = "replies"
        const val DONE = "done"
        const val APPLIED = "applied"
        const val PROPOSAL = "proposal"
        const val ASIDE = "aside"

        // the profile goes through the settings' own JSON (as one per-output entry)
        fun proposalJson(p: EqProposal): String = JSONObject()
            .put("output", p.outputId)
            .put("explanation", p.explanation)
            .put("profile", eqSettingsToJson(eqDefaultSettings().copy(perOutput = mapOf("p" to p.profile))))
            .toString()

        fun proposalOf(json: String): EqProposal? = runCatching {
            val o = JSONObject(json)
            val profile: EqProfile = eqSettingsFromJson(o.getString("profile")).perOutput["p"] ?: return null
            EqProposal(outputId = o.getString("output"), profile = profile, explanation = o.optString("explanation"))
        }.getOrNull()
    }
}
