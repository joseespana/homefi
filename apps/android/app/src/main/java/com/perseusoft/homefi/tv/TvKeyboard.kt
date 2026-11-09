package com.perseusoft.homefi.tv

import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.Backspace
import androidx.compose.material.icons.rounded.Clear
import androidx.compose.material.icons.rounded.Mic
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material.icons.rounded.SpaceBar
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.perseusoft.homefi.R
import com.perseusoft.homefi.ui.theme.LocalMotion

// The TV's on-screen keyboard: letters and digits in a grid the D-pad walks, space, delete, clear.
// Used by Search (beside its results) and by every short text prompt (the library filter).

private const val KEY_COLUMNS = 9
private val KEY_SIZE = 30.dp
private val KEY_GAP = 6.dp
private const val CARET_MS = 600
private val KEYS = (('a'..'z') + ('0'..'9')).map { it.toString() }

/** The width of [KEY_COLUMNS] keys with their gaps. */
val TV_KEYBOARD_WIDTH: Dp = KEY_SIZE * KEY_COLUMNS + KEY_GAP * (KEY_COLUMNS - 1)

/** What the keys do to the text: a pure rule, tested without a screen. */
fun typed(text: String, key: String): String = when (key) {
    KEY_DELETE -> text.dropLast(1)
    KEY_CLEAR -> ""
    KEY_SPACE -> if (text.isEmpty() || text.endsWith(" ")) text else "$text "
    else -> text + key
}

private const val KEY_DELETE = "⌫"
private const val KEY_CLEAR = "✕"
private const val KEY_SPACE = " "

/** The text as the user has typed it, with a caret (still when motion is off); [placeholder] while empty. */
@Composable
fun TvField(text: String, placeholder: String, modifier: Modifier = Modifier, onMic: (() -> Unit)? = null) {
    val look = LocalTvLook.current
    val motion = LocalMotion.current
    val caret = if (motion.enabled) {
        rememberInfiniteTransition(label = "tv-caret").animateFloat(
            initialValue = 1f,
            targetValue = 0f,
            animationSpec = infiniteRepeatable(tween(CARET_MS), RepeatMode.Reverse),
            label = "tv-caret-alpha",
        ).value
    } else {
        1f
    }
    Row(
        modifier
            .height(42.dp)
            .background(Color.White.copy(alpha = 0.10f), CircleShape)
            .border(1.dp, look.glassLine, CircleShape)
            .padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Icon(Icons.Rounded.Search, null, tint = look.ink2, modifier = Modifier.size(20.dp))
        Row(Modifier.weight(1f), verticalAlignment = Alignment.CenterVertically) {
            if (text.isEmpty()) {
                Text(placeholder, style = TvType.body, color = look.ink3, maxLines = 1, overflow = TextOverflow.Ellipsis)
            } else {
                Text(text, style = TvType.body.copy(fontSize = 18.sp), color = look.ink, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
            }
            Box(Modifier.padding(start = 2.dp).size(width = 2.dp, height = 22.dp).alpha(caret).background(look.accent2))
        }
        if (onMic != null) Icon(Icons.Rounded.Mic, null, tint = look.ink2, modifier = Modifier.size(20.dp))
    }
}

/**
 * The key grid. [onText] gets the new text after every key; [onSubmit] runs on the "Search" key.
 * [firstFocus] is for the first letter (a screen's first-focus target).
 */
@Composable
fun TvKeyboard(
    text: String,
    onText: (String) -> Unit,
    onSubmit: () -> Unit,
    modifier: Modifier = Modifier,
    firstFocus: Modifier = Modifier,
    /** RIGHT from the last column goes here (the results beside the keys). */
    rightTo: androidx.compose.ui.focus.FocusRequester? = null,
    submitLabel: String = stringResource(R.string.tv_kb_search),
) {
    Column(modifier.width(TV_KEYBOARD_WIDTH), verticalArrangement = Arrangement.spacedBy(KEY_GAP)) {
        KEYS.chunked(KEY_COLUMNS).forEachIndexed { row, keys ->
            Row(horizontalArrangement = Arrangement.spacedBy(KEY_GAP)) {
                keys.forEachIndexed { col, key ->
                    TvKey(
                        label = key,
                        onClick = { onText(typed(text, key)) },
                        modifier = Modifier
                            .then(if (col == 0) Modifier.tvLeftToDrawer() else Modifier)
                            .then(if (col == KEY_COLUMNS - 1) Modifier.tvRightTo(rightTo) else Modifier)
                            .then(if (row == 0 && col == 0) firstFocus else Modifier),
                    )
                }
            }
        }
        val wide = KEY_SIZE * 2 + KEY_GAP
        Row(horizontalArrangement = Arrangement.spacedBy(KEY_GAP)) {
            TvKey(KEY_SPACE, { onText(typed(text, KEY_SPACE)) }, Modifier.tvLeftToDrawer(), width = wide, icon = Icons.Rounded.SpaceBar, description = stringResource(R.string.tv_kb_space))
            TvKey(KEY_DELETE, { onText(typed(text, KEY_DELETE)) }, width = wide, icon = Icons.AutoMirrored.Rounded.Backspace, description = stringResource(R.string.tv_kb_delete))
            TvKey(KEY_CLEAR, { onText(typed(text, KEY_CLEAR)) }, width = wide, icon = Icons.Rounded.Clear, description = stringResource(R.string.clear))
            TvKey(submitLabel, onSubmit, Modifier.tvRightTo(rightTo), width = KEY_SIZE * 3 + KEY_GAP * 2, primary = true)
        }
    }
}

@Composable
private fun TvKey(
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    width: Dp = KEY_SIZE,
    icon: androidx.compose.ui.graphics.vector.ImageVector? = null,
    description: String? = null,
    primary: Boolean = false,
) {
    val look = LocalTvLook.current
    val shape = RoundedCornerShape(8.dp)
    TvFocusable(
        onClick = onClick,
        modifier = modifier.size(width = width, height = KEY_SIZE),
        shape = shape,
        focusedScale = 1.12f,
        glow = false,
    ) { focused ->
        val fill = when {
            focused -> Modifier.background(Color.White, shape)
            primary -> Modifier.background(look.gradient, shape)
            else -> Modifier.background(Color.White.copy(alpha = 0.10f), shape).border(1.dp, look.glassLine, shape)
        }
        val ink = when {
            focused -> look.onFocus
            primary -> look.onAccent
            else -> look.ink
        }
        Box(fill, contentAlignment = Alignment.Center) {
            if (icon != null) Icon(icon, description, tint = ink, modifier = Modifier.size(18.dp))
            else Text(label.uppercase(), style = TvType.body.copy(fontSize = if (primary) 13.sp else 15.sp), color = ink, maxLines = 1)
        }
    }
}

/** A short text asked on a card of its own (the library filter): the field, the keyboard, mic, Done. */
@Composable
fun TvTextPrompt(
    title: String,
    initial: String,
    onDone: (String) -> Unit,
    onDismiss: () -> Unit,
    placeholder: String = stringResource(R.string.tv_lib_filter),
    doneLabel: String = stringResource(R.string.tv_kb_done),
) {
    val look = LocalTvLook.current
    var text by remember { mutableStateOf(initial) }
    val mic = rememberVoiceInput { text = it }
    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Box(Modifier.fillMaxSize().background(Color(0xCC040510)), contentAlignment = Alignment.Center) {
            TvGlass(Modifier.width(TV_KEYBOARD_WIDTH + 56.dp), solid = true) {
                Column(Modifier.padding(28.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Text(title, style = TvType.section, color = look.ink, maxLines = 1)
                    TvField(text, placeholder, Modifier.fillMaxWidth(), onMic = mic)
                    TvKeyboard(
                        text = text,
                        onText = { text = it },
                        onSubmit = { onDone(text.trim()) },
                        firstFocus = Modifier.tvInitialFocus(),
                        submitLabel = doneLabel,
                    )
                    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        if (mic != null) TvPill(stringResource(R.string.tv_kb_speak), mic, icon = Icons.Rounded.Mic, height = 30.dp)
                        TvPill(stringResource(R.string.cancel), onDismiss, height = 30.dp)
                    }
                }
            }
        }
    }
}
