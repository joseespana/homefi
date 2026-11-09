package com.perseusoft.homefi.tv

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material.icons.rounded.Keyboard
import androidx.compose.material.icons.rounded.Mic
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.perseusoft.homefi.R
import com.perseusoft.homefi.ui.theme.LocalMotion

// The chat both AI screens share (the playlist chat and the sound assistant): the transcript, the
// assistant's quick replies as pills, a mic (the Google TV remote's) and the on-screen keyboard.

/** One message of a TV chat. */
data class TvChatLine(val id: Int, val fromUser: Boolean, val text: String)

private val BUBBLE_MAX = 400.dp
private val AVATAR = 24.dp
private const val DOT_COUNT = 3
private const val DOT_PERIOD_MS = 520
private const val DOT_STAGGER_MS = 150
private const val DOT_MIN_ALPHA = 0.25f

/**
 * The conversation: the transcript scrolls to its end by itself, [extraItems] adds cards after the last
 * message (a proposal, a preview), then one pill per quick reply and a row with Speak, Type and
 * the screen's own [actions]. Spoken words go out as the message; typing opens the keyboard card.
 */
@Composable
fun TvChat(
    lines: List<TvChatLine>,
    typing: Boolean,
    replies: List<String>,
    canSend: Boolean,
    onSend: (String) -> Unit,
    entryKey: Int,
    promptTitle: String,
    placeholder: String,
    modifier: Modifier = Modifier,
    /** Shown instead of the composer when the conversation can take no more turns. */
    closedNote: String? = null,
    /** False while a card in the transcript (a proposal) has the focus's first claim. */
    claimFocus: Boolean = true,
    extraItems: LazyListScope.() -> Unit = {},
    actions: @Composable RowScope.() -> Unit = {},
) {
    val look = LocalTvLook.current
    val list = rememberLazyListState()
    var keyboard by remember { mutableStateOf(false) }
    val mic = rememberVoiceInput { if (canSend) onSend(it) }
    val shownReplies = if (canSend && !typing) replies else emptyList()

    // the newest message (or the typing dots) stays in view
    LaunchedEffect(lines.size, typing) {
        val last = list.layoutInfo.totalItemsCount - 1
        if (last >= 0) list.animateScrollToItem(last)
    }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        LazyColumn(
            state = list,
            modifier = Modifier.weight(1f).fillMaxWidth(),
            contentPadding = PaddingValues(start = 12.dp, end = 12.dp, top = 8.dp, bottom = 8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            items(lines, key = { "line-${it.id}" }) { line -> TvBubble(line) }
            if (typing) item(key = "typing") { TvTyping() }
            extraItems()
        }
        if (closedNote != null) {
            Text(closedNote, style = TvType.secondary.copy(fontSize = 13.sp), color = look.ink2, modifier = Modifier.padding(horizontal = 12.dp))
        }
        if (shownReplies.isNotEmpty()) {
            LazyRow(contentPadding = PaddingValues(horizontal = 12.dp, vertical = 6.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                itemsIndexed(shownReplies, key = { i, r -> "reply-$i-$r" }) { i, reply ->
                    TvPill(
                        reply,
                        { onSend(reply) },
                        Modifier
                            .then(if (i == 0) Modifier.tvInitialFocus(key = lines.size to entryKey, enabled = claimFocus).tvLeftToDrawer() else Modifier)
                            .widthIn(max = BUBBLE_MAX),
                        height = 30.dp,
                        labelStyle = TvType.body.copy(fontSize = 14.sp),
                    )
                }
            }
        }
        Row(Modifier.padding(horizontal = 12.dp), horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.CenterVertically) {
            val first = shownReplies.isEmpty()
            TvPill(
                stringResource(R.string.tv_chat_type),
                { keyboard = true },
                Modifier.then(if (first) Modifier.tvInitialFocus(key = lines.size to entryKey, enabled = claimFocus).tvLeftToDrawer() else Modifier).alpha(if (canSend) 1f else 0.5f),
                icon = Icons.Rounded.Keyboard,
                height = 34.dp,
                primary = first && canSend,
            )
            if (mic != null) TvPill(stringResource(R.string.tv_kb_speak), mic, icon = Icons.Rounded.Mic, height = 34.dp)
            actions()
        }
    }
    if (keyboard) {
        TvTextPrompt(
            title = promptTitle,
            initial = "",
            placeholder = placeholder,
            doneLabel = stringResource(R.string.pl_party_send),
            onDone = { text ->
                keyboard = false
                if (text.isNotBlank() && canSend) onSend(text)
            },
            onDismiss = { keyboard = false },
        )
    }
}

/** One message: the assistant's on the left with its mark, the user's on the right in the accent. */
@Composable
fun TvBubble(line: TvChatLine, modifier: Modifier = Modifier) {
    val look = LocalTvLook.current
    val motion = LocalMotion.current
    // a new message settles in (fade and a short rise) unless motion is off
    var shown by remember(line.id) { mutableStateOf(!motion.enabled) }
    LaunchedEffect(line.id) { shown = true }
    val appear by animateFloatAsState(if (shown) 1f else 0f, motion.tween(220), label = "tv-bubble")
    val shape = RoundedCornerShape(
        topStart = 16.dp,
        topEnd = 16.dp,
        bottomStart = if (line.fromUser) 16.dp else 4.dp,
        bottomEnd = if (line.fromUser) 4.dp else 16.dp,
    )
    Row(
        modifier.fillMaxWidth().alpha(appear),
        horizontalArrangement = if (line.fromUser) Arrangement.End else Arrangement.Start,
        verticalAlignment = Alignment.Top,
    ) {
        if (!line.fromUser) {
            Box(Modifier.size(AVATAR).background(look.accent.copy(alpha = 0.2f), CircleShape), contentAlignment = Alignment.Center) {
                Icon(Icons.Rounded.AutoAwesome, null, tint = look.accent2, modifier = Modifier.size(14.dp))
            }
            Box(Modifier.size(8.dp))
        }
        val fill = if (line.fromUser) Modifier.background(look.gradient, shape) else Modifier.background(Color.White.copy(alpha = 0.10f), shape).border(1.dp, look.glassLine, shape)
        Text(
            line.text,
            style = TvType.body.copy(fontSize = 15.sp, fontWeight = androidx.compose.ui.text.font.FontWeight.Medium),
            color = if (line.fromUser) look.onAccent else look.ink,
            modifier = Modifier.widthIn(max = BUBBLE_MAX).then(fill).padding(horizontal = 12.dp, vertical = 8.dp),
        )
    }
}

/** Three dots that breathe while the AI answers (still when motion is off). */
@Composable
fun TvTyping(modifier: Modifier = Modifier) {
    val look = LocalTvLook.current
    val motion = LocalMotion.current
    Row(modifier.padding(start = AVATAR + 8.dp), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
        val transition = if (motion.enabled) rememberInfiniteTransition(label = "tv-typing") else null
        repeat(DOT_COUNT) { i ->
            val alpha = transition?.animateFloat(
                initialValue = DOT_MIN_ALPHA,
                targetValue = 1f,
                animationSpec = infiniteRepeatable(
                    androidx.compose.animation.core.tween(DOT_PERIOD_MS, delayMillis = i * DOT_STAGGER_MS),
                    androidx.compose.animation.core.RepeatMode.Reverse,
                ),
                label = "tv-typing-dot-$i",
            )?.value ?: 0.6f
            Box(Modifier.size(8.dp).alpha(alpha).background(look.accent2, CircleShape))
        }
    }
}

/** A card that comes up under the last message (a proposal, a preview): fades in, rises a little. */
@Composable
fun TvChatCard(modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    val motion = LocalMotion.current
    AnimatedVisibility(
        visible = true,
        enter = fadeIn(motion.tween(220)) + (if (motion.moves) slideInVertically(motion.tween(260)) { it / 8 } else androidx.compose.animation.EnterTransition.None),
        exit = fadeOut(motion.tween(120)),
        modifier = modifier,
    ) { TvGlass(Modifier.fillMaxWidth()) { Box(Modifier.padding(14.dp)) { content() } } }
}
