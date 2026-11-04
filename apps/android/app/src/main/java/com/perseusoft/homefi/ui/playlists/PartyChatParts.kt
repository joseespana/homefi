package com.perseusoft.homefi.ui.playlists

import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.StartOffset
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.Send
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material3.AssistChip
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.PartyProgress
import com.perseusoft.homefi.ui.eq.ChatLine
import com.perseusoft.homefi.ui.haptics
import com.perseusoft.homefi.ui.settings.AiSettingsSection
import com.perseusoft.homefi.ui.theme.Button

// The pieces the party and the "Playlist with AI" screens share: a simple chat and the build bar.

private const val BUBBLE_MAX_WIDTH_DP = 280
private const val BUBBLE_RADIUS_DP = 18
private const val BUBBLE_TAIL_DP = 4
private const val AVATAR_DP = 28
private const val AVATAR_ICON_DP = 16
private const val AVATAR_GAP_DP = 8
private const val SCREEN_PADDING_DP = 16
private const val BAR_ELEVATION_DP = 3
private const val COMPOSER_RADIUS_DP = 24
private const val COMPOSER_MAX_LINES = 4
private const val TYPING_DOT_DP = 8
private const val TYPING_DOT_MIN_ALPHA = 0.25f
private const val TYPING_DOT_PERIOD_MS = 500
private const val TYPING_DOT_OFFSET_MS = 160
private const val TYPING_DOTS = 3

/** Keeps the newest message (or the typing dots) in view as the chat grows. */
@Composable
internal fun ChatAutoScroll(list: LazyListState, chat: PartyChat?) {
    val lines = chat?.lines?.size ?: 0
    val thinking = chat?.thinking == true
    LaunchedEffect(lines, thinking) { if (lines > 0 || thinking) list.animateScrollToItem(list.layoutInfo.totalItemsCount) }
}

/** The "set up an AI" prompt shown instead of the screen when no AI (or key) is available. */
@Composable
internal fun NeedsAi(text: Int, onSetUp: () -> Unit) {
    val haptics = haptics()
    Column(Modifier.padding(horizontal = 16.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(
            stringResource(text),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Button(onClick = { haptics.press(); onSetUp() }) { Text(stringResource(R.string.pl_party_set_up_ai)) }
    }
}

/** The same setup sheet the concert screen offers. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun SetUpAiSheet(onDismiss: () -> Unit) {
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).navigationBarsPadding()) {
            AiSettingsSection()
        }
    }
}

/**
 * A simple chat: the AI's turns on the left (with its icon), the user's on the right, typing dots
 * while it answers, quick replies under the last AI turn and a rounded composer. After the last
 * turn only a closing line. [hint] is the composer's placeholder.
 */
internal fun LazyListScope.chat(chat: PartyChat, busy: Boolean, hint: Int, onSend: (String) -> Unit) {
    items(chat.lines.size, key = { "chat-${chat.lines[it].id}" }) { ChatBubble(chat.lines[it]) }
    if (chat.thinking) item(key = "chat-thinking") { TypingIndicator() }
    item(key = "chat-input") {
        val haptics = haptics()
        var text by rememberSaveable { mutableStateOf("") }
        Column(Modifier.padding(horizontal = SCREEN_PADDING_DP.dp)) {
            if (chat.closed) {
                Text(
                    stringResource(R.string.pl_party_chat_closing),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(vertical = 8.dp),
                )
                return@Column
            }
            val canSend = text.isNotBlank() && !chat.thinking && !busy
            if (chat.replies.isNotEmpty() && !chat.thinking && !busy) {
                // under the last AI turn, in line with its bubble
                Row(Modifier.padding(start = (AVATAR_DP + AVATAR_GAP_DP).dp).horizontalScroll(rememberScrollState())) {
                    chat.replies.forEach { reply ->
                        AssistChip(
                            onClick = { haptics.tick(); onSend(reply) },
                            label = { Text(reply) },
                            modifier = Modifier.heightIn(min = MIN_TOUCH_DP.dp).padding(end = 8.dp),
                        )
                    }
                }
            }
            val send = {
                if (canSend) {
                    haptics.press()
                    onSend(text)
                    text = ""
                }
            }
            Row(Modifier.fillMaxWidth().padding(vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                OutlinedTextField(
                    value = text,
                    onValueChange = { text = it },
                    modifier = Modifier.weight(1f),
                    enabled = !busy && !chat.thinking,
                    placeholder = { Text(stringResource(hint)) },
                    maxLines = COMPOSER_MAX_LINES,
                    shape = RoundedCornerShape(COMPOSER_RADIUS_DP.dp),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send),
                    keyboardActions = KeyboardActions(onSend = { send() }),
                )
                Spacer(Modifier.size(8.dp))
                FilledIconButton(onClick = send, enabled = canSend, modifier = Modifier.size(MIN_TOUCH_DP.dp)) {
                    Icon(Icons.AutoMirrored.Rounded.Send, stringResource(R.string.pl_party_send))
                }
            }
        }
    }
}

/** The small round AI mark that leads the AI's turns. */
@Composable
private fun AiAvatar() {
    Box(
        Modifier.size(AVATAR_DP.dp).clip(CircleShape).background(MaterialTheme.colorScheme.primaryContainer),
        contentAlignment = Alignment.Center,
    ) {
        Icon(Icons.Rounded.AutoAwesome, null, Modifier.size(AVATAR_ICON_DP.dp), tint = MaterialTheme.colorScheme.onPrimaryContainer)
    }
}

private fun bubbleShape(mine: Boolean) = RoundedCornerShape(
    topStart = BUBBLE_RADIUS_DP.dp,
    topEnd = BUBBLE_RADIUS_DP.dp,
    bottomStart = (if (mine) BUBBLE_RADIUS_DP else BUBBLE_TAIL_DP).dp,
    bottomEnd = (if (mine) BUBBLE_TAIL_DP else BUBBLE_RADIUS_DP).dp,
)

/** One turn as plain text (no Markdown): the user's on the right in the primary colour, the AI's on the left. */
@Composable
private fun ChatBubble(line: ChatLine) {
    val scheme = MaterialTheme.colorScheme
    val mine = line.fromUser
    // TalkBack reads who said it, then the text, as one item
    val spoken = stringResource(if (mine) R.string.pl_chat_you_said else R.string.pl_chat_ai_said, line.text)
    Row(
        Modifier.fillMaxWidth().padding(horizontal = SCREEN_PADDING_DP.dp, vertical = 4.dp).clearAndSetSemantics { contentDescription = spoken },
        horizontalArrangement = if (mine) Arrangement.End else Arrangement.Start,
        verticalAlignment = Alignment.Bottom,
    ) {
        if (!mine) {
            AiAvatar()
            Spacer(Modifier.size(AVATAR_GAP_DP.dp))
        }
        Text(
            line.text,
            style = MaterialTheme.typography.bodyMedium,
            color = if (mine) scheme.onPrimary else scheme.onSurface,
            modifier = Modifier
                .widthIn(max = BUBBLE_MAX_WIDTH_DP.dp)
                .clip(bubbleShape(mine))
                .background(if (mine) scheme.primary else scheme.surfaceContainerHigh)
                .padding(horizontal = 14.dp, vertical = 10.dp),
        )
    }
}

/** Three pulsing dots in an AI bubble while the AI answers. */
@Composable
private fun TypingIndicator() {
    val typing = stringResource(R.string.pl_chat_typing)
    val transition = rememberInfiniteTransition(label = "typing")
    Row(
        Modifier
            .padding(horizontal = SCREEN_PADDING_DP.dp, vertical = 4.dp)
            .semantics { contentDescription = typing; liveRegion = LiveRegionMode.Polite },
        verticalAlignment = Alignment.Bottom,
    ) {
        AiAvatar()
        Spacer(Modifier.size(AVATAR_GAP_DP.dp))
        Row(
            Modifier
                .clip(bubbleShape(mine = false))
                .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                .padding(horizontal = 14.dp, vertical = 14.dp),
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            repeat(TYPING_DOTS) { index ->
                val alpha by transition.animateFloat(
                    initialValue = TYPING_DOT_MIN_ALPHA,
                    targetValue = 1f,
                    animationSpec = infiniteRepeatable(
                        tween(TYPING_DOT_PERIOD_MS),
                        RepeatMode.Reverse,
                        initialStartOffset = StartOffset(index * TYPING_DOT_OFFSET_MS),
                    ),
                    label = "typing-dot-$index",
                )
                Box(
                    Modifier
                        .size(TYPING_DOT_DP.dp)
                        .graphicsLayer { this.alpha = alpha }
                        .clip(CircleShape)
                        .background(MaterialTheme.colorScheme.onSurfaceVariant),
                )
            }
        }
    }
}

/**
 * The buttons (always reachable above the keyboard), or the build's progress with Cancel. The
 * "talk it over" link shows only when [onTalk] is given and the chat hasn't started.
 */
@Composable
internal fun ActionBar(
    state: PartyUi,
    form: PartyForm,
    buildEnabled: Boolean,
    onBuild: () -> Unit,
    onCancel: () -> Unit,
    onTalk: (() -> Unit)? = null,
) {
    val haptics = haptics()
    Surface(Modifier.fillMaxWidth().imePadding(), tonalElevation = BAR_ELEVATION_DP.dp) {
        Column(Modifier.padding(horizontal = 16.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            form.error?.let { error ->
                Text(error, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.error)
            }
            if (state is PartyUi.Building) {
                BuildingProgress(state, onCancel)
                return@Column
            }
            val again = state is PartyUi.Built
            Button(
                onClick = { haptics.press(); onBuild() },
                enabled = buildEnabled,
                modifier = Modifier.fillMaxWidth().heightIn(min = MIN_TOUCH_DP.dp),
            ) { Text(stringResource(if (again) R.string.pl_party_build_again else R.string.pl_party_build)) }
            if (onTalk != null && !form.chat.started && !again) {
                TextButton(
                    onClick = { haptics.press(); onTalk() },
                    modifier = Modifier.fillMaxWidth().heightIn(min = MIN_TOUCH_DP.dp),
                ) { Text(stringResource(R.string.pl_party_talk)) }
            }
        }
    }
}

@Composable
private fun BuildingProgress(building: PartyUi.Building, onCancel: () -> Unit) {
    val progress = building.progress
    val line = when {
        progress is PartyProgress.Resolving -> stringResource(R.string.pl_progress_resolving, progress.done.toInt(), progress.total.toInt())
        progress is PartyProgress.Asking && progress.round > 0u -> stringResource(R.string.pl_party_progress_more, building.aiName)
        else -> stringResource(R.string.pl_party_progress_asking, building.aiName)
    }
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(line, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite })
        if (progress is PartyProgress.Resolving && progress.total > 0u) {
            LinearProgressIndicator(progress = { progress.done.toFloat() / progress.total.toFloat() }, modifier = Modifier.fillMaxWidth())
        } else {
            LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
        }
        TextButton(onClick = onCancel) { Text(stringResource(R.string.pl_cancel)) }
    }
}
