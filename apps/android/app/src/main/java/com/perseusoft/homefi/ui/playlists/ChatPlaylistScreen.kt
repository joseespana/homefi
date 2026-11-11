package com.perseusoft.homefi.ui.playlists

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedCard
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.ChatBrief
import com.perseusoft.homefi.core.HomePlaylist
import com.perseusoft.homefi.ui.library.ScreenHeader

/**
 * "Make a playlist with AI": a simple chat with the user's AI (the party session without the
 * questions), a one-line summary of what it understood, then the same preview the other modes
 * have. It shows the open chat [conversationId] (kept in [com.perseusoft.homefi.data.AiConversations], so it goes on when
 * this screen closes); a saved playlist's chat starts from its brief and building saves over it, and a seed song opens
 * the chat asking for a playlist like it. Build waits for the first message. Without an AI it only offers to set one up.
 */
@Composable
fun ChatPlaylistScreen(conversationId: String, onBack: () -> Unit, onDelete: (HomePlaylist) -> Unit) {
    ConversationHost(conversationId, onBack) { ChatPlaylistBody(it, onBack, onDelete) }
}

@Composable
private fun ChatPlaylistBody(vm: PartyConversation, onBack: () -> Unit, onDelete: (HomePlaylist) -> Unit) {
    val ui by vm.ui.collectAsStateWithLifecycle()
    var setup by rememberSaveable { mutableStateOf(false) }
    val list = rememberLazyListState()
    BackHandler(enabled = ui is PartyUi.Building) { vm.cancel() }

    val state = ui
    val form = when (state) {
        is PartyUi.Asking -> state.form
        is PartyUi.Building -> state.form
        is PartyUi.Built -> state.form
        PartyUi.NeedsAi -> null
    }
    val busy = state is PartyUi.Building
    ChatAutoScroll(list, form?.chat)

    Column(Modifier.fillMaxSize()) {
        LazyColumn(state = list, modifier = Modifier.weight(1f).fillMaxWidth(), contentPadding = PaddingValues(bottom = 24.dp)) {
            item(key = "top") { ScreenHeader(stringResource(R.string.pl_chat_title), onBack = onBack) }
            if (form == null) {
                item(key = "needs-ai") { NeedsAi(R.string.pl_chat_needs_ai, onSetUp = { setup = true }) }
            } else {
                item(key = "intro") {
                    Text(
                        stringResource(R.string.pl_chat_intro),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
                    )
                }
                form.chatBrief?.let { brief ->
                    item(key = "summary") { BriefSummary(chatSummary(brief)) }
                }
                if (form.chat.started) chat(form.chat, busy, R.string.pl_chat_hint, vm::send)
                if (state is PartyUi.Built) {
                    val playlist = state.built.playlist
                    playlistPreview(
                        playlist = playlist,
                        fallbackNote = state.built.note,
                        aiName = null,
                        actions = PreviewActions(
                            saved = state.saved,
                            onSave = vm::save,
                            onDiscard = vm::discard,
                            onSearchAgain = vm::build,
                            onDelete = { onDelete(playlist); vm.discard() },
                            searchAgainLabel = R.string.pl_party_build_again,
                        ),
                        showsOpen = false,
                        onToggleShows = {},
                    )
                }
            }
        }
        if (form != null) {
            // nothing to build from until the user has said something
            val buildEnabled = !form.chat.thinking && form.chat.userTurns > 0
            ActionBar(state, form, buildEnabled = buildEnabled, onBuild = vm::build, onCancel = vm::cancel)
        }
    }
    if (setup) {
        SetUpAiSheet(onDismiss = { setup = false; vm.refreshAi() })
    }
}

/** What the AI understood so far, as a quiet card above the chat. */
@Composable
private fun BriefSummary(text: String) {
    OutlinedCard(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
        Text(
            text,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.heightIn(min = MIN_TOUCH_DP.dp).padding(horizontal = 16.dp, vertical = 12.dp),
        )
    }
}

/** "Sad 90s in English · 1 h · Steady · Explicit allowed"; "Not set yet" while the AI has no title. */
@Composable
internal fun chatSummary(brief: ChatBrief): String {
    val title = brief.title.ifBlank { stringResource(R.string.pl_chat_summary_unset) }
    val explicit = stringResource(if (brief.allowExplicit) R.string.pl_party_explicit_summary_allow else R.string.pl_party_explicit_summary_avoid)
    return listOf(title, partyLength(brief.minutes.toInt()), curveLabel(brief.curve), explicit).joinToString(" · ")
}
