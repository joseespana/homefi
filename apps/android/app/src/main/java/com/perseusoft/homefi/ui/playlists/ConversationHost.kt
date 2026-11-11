package com.perseusoft.homefi.ui.playlists

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.library.ScreenHeader

/**
 * Shows the open chat with this [id], marks it as the one on screen (no "ready" notice for it),
 * and says so when it was closed (from the chats menu) while the screen was open.
 */
@Composable
internal fun ConversationHost(id: String, onBack: () -> Unit, content: @Composable (PartyConversation) -> Unit) {
    val app = LocalContext.current.homefi
    val chats by app.conversations.list.collectAsStateWithLifecycle()
    val chat = chats.firstOrNull { it.id == id }
    DisposableEffect(id) {
        app.conversations.visibleId.value = id
        onDispose { app.conversations.visibleId.compareAndSet(id, null) }
    }
    if (chat != null) {
        content(chat)
    } else {
        Column(Modifier.fillMaxSize()) {
            ScreenHeader(stringResource(R.string.pl_chats_title), onBack = onBack)
            Text(
                stringResource(R.string.pl_chat_closed),
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(16.dp),
            )
        }
    }
}
