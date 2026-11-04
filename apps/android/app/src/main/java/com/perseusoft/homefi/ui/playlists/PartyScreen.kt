package com.perseusoft.homefi.ui.playlists

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.HomePlaylist
import com.perseusoft.homefi.core.partyDefaultBrief
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.haptics
import com.perseusoft.homefi.ui.library.ScreenHeader

internal const val MIN_TOUCH_DP = 48

/**
 * "Make a party playlist": a few questions, an optional short chat with the user's AI, then the
 * same preview concert mode has. With [replace] (a saved party playlist) the answers start as its
 * brief and building saves over it. Without an AI it only offers to set one up.
 */
@Composable
fun PartyScreen(
    replace: HomePlaylist?,
    onBack: () -> Unit,
    onDelete: (HomePlaylist) -> Unit,
    vm: PartyViewModel = viewModel(key = "party-${replace?.id}"),
) {
    val app = LocalContext.current.homefi
    val ui by vm.ui.collectAsStateWithLifecycle()
    val settings by app.settings.playlists.collectAsStateWithLifecycle()
    var editing by rememberSaveable { mutableStateOf(false) }
    var moreOpen by rememberSaveable { mutableStateOf<Boolean?>(null) }
    var setup by rememberSaveable { mutableStateOf(false) }
    val list = rememberLazyListState()
    LaunchedEffect(Unit) { vm.open(replace) }
    BackHandler(enabled = ui is PartyUi.Building) { vm.cancel() }

    val state = ui
    val form = when (state) {
        is PartyUi.Asking -> state.form
        is PartyUi.Building -> state.form
        is PartyUi.Built -> state.form
        PartyUi.NeedsAi -> null
    }
    val busy = state is PartyUi.Building
    // "More details" opens by itself when a folded answer differs from the Settings defaults
    val detailsDiffer = form != null && form.brief.detailsDiffer(partyDefaultBrief(settings))
    ChatAutoScroll(list, form?.chat)

    Column(Modifier.fillMaxSize()) {
        LazyColumn(state = list, modifier = Modifier.weight(1f).fillMaxWidth(), contentPadding = PaddingValues(bottom = 24.dp)) {
            item(key = "top") { ScreenHeader(stringResource(R.string.pl_party_title), onBack = onBack) }
            if (form == null) {
                item(key = "needs-ai") { NeedsAi(R.string.pl_party_needs_ai, onSetUp = { setup = true }) }
            } else {
                item(key = "intro") {
                    Text(
                        stringResource(R.string.pl_party_intro),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
                    )
                }
                item(key = "questions") {
                    val folded = (form.chat.started || state is PartyUi.Built) && !editing
                    Column(Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
                        if (folded) {
                            SummaryCard(briefSummary(form.brief)) { editing = true }
                        } else {
                            PartyQuestions(
                                brief = form.brief,
                                enabled = !busy,
                                moreOpen = moreOpen ?: detailsDiffer,
                                onToggleMore = { moreOpen = !(moreOpen ?: detailsDiffer) },
                                onBrief = vm::setBrief,
                            )
                        }
                    }
                }
                if (form.chat.started) chat(form.chat, busy, R.string.pl_party_chat_hint, vm::send)
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
            ActionBar(state, form, buildEnabled = !form.chat.thinking, onBuild = vm::build, onCancel = vm::cancel, onTalk = vm::talk)
        }
    }
    if (setup) {
        SetUpAiSheet(onDismiss = { setup = false; vm.refreshAi() })
    }
}

@Composable
private fun SummaryCard(text: String, onEdit: () -> Unit) {
    val haptics = haptics()
    OutlinedCard(Modifier.fillMaxWidth().clickable { haptics.tick(); onEdit() }) {
        Text(text, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.heightIn(min = MIN_TOUCH_DP.dp).padding(16.dp))
    }
}
