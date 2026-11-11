package com.perseusoft.homefi.ui.playlists

import android.text.format.DateUtils
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.clickable
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.homefi

/** The open AI chats: tap one to go back to it, close it to cancel its work. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ConversationsSheet(onDismiss: () -> Unit) {
    val app = LocalContext.current.homefi
    val chats by app.conversations.list.collectAsStateWithLifecycle()
    val shown = chats.filterNot { it.unavailable }
    ModalBottomSheet(onDismissRequest = onDismiss) {
        Column(Modifier.navigationBarsPadding().padding(bottom = 16.dp)) {
            Text(
                stringResource(R.string.pl_chats_title),
                style = MaterialTheme.typography.titleLarge,
                modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp),
            )
            if (shown.isEmpty()) {
                Text(
                    stringResource(R.string.pl_chats_empty),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = 24.dp, vertical = 12.dp),
                )
            } else {
                LazyColumn {
                    items(shown, key = { it.id }) { chat ->
                        ChatRow(
                            chat,
                            onOpen = {
                                app.chatRequests.open(ChatRequest.Resume(chat.id))
                                onDismiss()
                            },
                            onClose = { app.conversations.close(chat.id) },
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun ChatRow(chat: PartyConversation, onOpen: () -> Unit, onClose: () -> Unit) {
    val title by chat.title.collectAsStateWithLifecycle()
    val last by chat.lastActivityMs.collectAsStateWithLifecycle()
    val ui by chat.ui.collectAsStateWithLifecycle()
    val failure by chat.buildFailure.collectAsStateWithLifecycle()
    val status = chatStatus(ui, failure)
    val ago = DateUtils.getRelativeTimeSpanString(last, System.currentTimeMillis(), DateUtils.MINUTE_IN_MILLIS).toString()
    Row(
        Modifier.fillMaxWidth().heightIn(min = MIN_TOUCH_DP.dp).clickable(onClick = onOpen).padding(start = 24.dp, end = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Column(Modifier.weight(1f).padding(vertical = 8.dp)) {
            Text(title, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(
                "${statusWords(status)} · $ago",
                style = MaterialTheme.typography.bodySmall,
                color = if (status is ChatStatus.Failed) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        IconButton(onClick = onClose, modifier = Modifier.size(MIN_TOUCH_DP.dp)) {
            Icon(Icons.Rounded.Close, stringResource(R.string.pl_chat_close, title))
        }
    }
}

@Composable
private fun statusWords(status: ChatStatus): String = when (status) {
    ChatStatus.Waiting -> stringResource(R.string.pl_chat_status_waiting)
    ChatStatus.Thinking -> stringResource(R.string.pl_chat_status_thinking)
    is ChatStatus.Building -> status.fraction?.let { stringResource(R.string.pl_chat_status_building_pct, (it * PERCENT).toInt()) }
        ?: stringResource(R.string.pl_chat_status_building)
    ChatStatus.Ready -> stringResource(R.string.pl_chat_status_ready)
    is ChatStatus.Failed -> stringResource(R.string.pl_chat_status_failed)
}

private const val PERCENT = 100
