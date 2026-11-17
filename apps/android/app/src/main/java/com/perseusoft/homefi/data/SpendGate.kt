package com.perseusoft.homefi.data

import com.perseusoft.homefi.core.AiFeature
import com.perseusoft.homefi.core.AiSpendDecision
import com.perseusoft.homefi.core.aiSpendCheck
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import java.util.Locale

/**
 * "Ask before spending": every AI action asks the core's guard first (`ai_spend_check`). A confirmation or the
 * "limit reached" notice is published in [ask] for the dialog host of the phone or TV root to show; the action
 * waits for the answer. The core also enforces the monthly limit on every AI call, so this is the friendly part,
 * not the only one. Holds no Context.
 */
class SpendGate(private val ai: AiSetup) {
    /** What the dialog shows; [answer] completes with the user's choice. */
    sealed interface Ask {
        val title: String
        val text: String
        val answer: CompletableDeferred<Boolean>

        /** "This usually costs ≈$0.04 · you've spent $1.20 this month. Continue?" */
        class Confirm(override val title: String, override val text: String) : Ask {
            override val answer = CompletableDeferred<Boolean>()
        }

        /** The monthly limit is reached: said plainly, only an OK. */
        class Blocked(override val title: String, override val text: String) : Ask {
            override val answer = CompletableDeferred<Boolean>()
        }
    }

    private val _ask = MutableStateFlow<Ask?>(null)
    val ask: StateFlow<Ask?> = _ask.asStateFlow()

    /** One question at a time: a second action waits for the first answer. */
    private val asking = Mutex()

    /**
     * True when [feature] may start. No AI set up (nothing will be spent) or a failed check (the core still
     * guards) never stops the user. Call from a coroutine; the checks run off the main thread.
     */
    suspend fun allow(feature: AiFeature): Boolean {
        val decision = withContext(Dispatchers.IO) {
            if (!ai.hasProvider()) null
            else runCatching { aiSpendCheck(feature, Locale.getDefault().toLanguageTag()) }.getOrNull()
        } ?: return true
        return when (decision) {
            is AiSpendDecision.Allow -> true
            is AiSpendDecision.Confirm -> show(Ask.Confirm(decision.title, decision.text))
            is AiSpendDecision.Blocked -> {
                show(Ask.Blocked(decision.title, decision.text))
                false
            }
        }
    }

    private suspend fun show(question: Ask): Boolean = asking.withLock {
        _ask.value = question
        try {
            question.answer.await()
        } finally {
            // answered, or the action was cancelled while asking: the dialog goes either way
            _ask.compareAndSet(question, null)
        }
    }

    /** The dialog's answer (OK on a blocked notice is `false`: nothing starts). */
    fun answer(yes: Boolean) {
        _ask.value?.answer?.complete(yes)
    }
}
