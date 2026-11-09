package com.perseusoft.homefi.tv

import androidx.compose.animation.Crossfade
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawOutline
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.zIndex
import com.perseusoft.homefi.ui.theme.LocalMotion
import kotlinx.coroutines.delay

// The TV's focus language (docs/TV_UI.md section 5): focus is a SHAPE, never only a colour. Scale
// up, a white ring with an accent ring around it and a glow; pills and rows also turn white.

private val RING_WHITE = 2.5.dp
private val RING_ACCENT = 2.dp
private val GLOW_SPREAD = 6.dp
private const val SCALE_MS = 220
private val GLOW_CORNER = 18.dp
private const val GLOW_LAYERS = 5
private const val GLOW_ALPHA = 0.06f
private const val SUBTLE_SCALE_SHARE = 0.5f

/** How big a focused thing grows under the motion setting: full, half (SUBTLE) or not at all (OFF). */
internal fun focusScale(target: Float, motionEnabled: Boolean, motionMoves: Boolean): Float = when {
    !motionEnabled -> 1f
    motionMoves -> target
    else -> 1f + (target - 1f) * SUBTLE_SCALE_SHARE
}

/**
 * Anything the D-pad can land on: grows, gets the ring and the glow when focused, runs [onClick]
 * on OK. [content] receives the focus state so it can recolour itself (a pill turns white).
 * Put a `focusRequester` in [modifier] to make it a first-focus target.
 */
@OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)
@Composable
fun TvFocusable(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    /** Long press of OK: the context menu of a card or a row. */
    onLongClick: (() -> Unit)? = null,
    shape: Shape = RoundedCornerShape(TvDimens.cardRadius),
    focusedScale: Float = 1.06f,
    ring: Boolean = true,
    glow: Boolean = true,
    enabled: Boolean = true,
    onFocused: (Boolean) -> Unit = {},
    content: @Composable BoxScope.(focused: Boolean) -> Unit,
) {
    val look = LocalTvLook.current
    val motion = LocalMotion.current
    var focused by remember { mutableStateOf(false) }
    val onFocusedNow by rememberUpdatedState(onFocused)
    val scale by animateFloatAsState(
        if (focused) focusScale(focusedScale, motion.enabled, motion.moves) else 1f,
        // a little overshoot when things move freely; plain fades under SUBTLE / OFF
        animationSpec = if (motion.moves) spring(dampingRatio = 0.62f, stiffness = 520f) else motion.tween(SCALE_MS),
        label = "tv-focus-scale",
    )
    Box(
        modifier = modifier
            .onFocusChanged {
                if (focused != it.isFocused) {
                    focused = it.isFocused
                    onFocusedNow(it.isFocused)
                }
            }
            .zIndex(if (focused) 1f else 0f)
            .graphicsLayer {
                scaleX = scale
                scaleY = scale
            }
            .drawBehind {
                if (focused && glow) {
                    // soft light that follows the shape: stacked, growing, fainter outlines
                    val spread = GLOW_SPREAD.toPx()
                    for (i in 1..GLOW_LAYERS) {
                        val grow = spread * i / GLOW_LAYERS
                        val alpha = GLOW_ALPHA * (1f - (i - 1f) / GLOW_LAYERS)
                        drawRoundRect(
                            look.accent.copy(alpha = alpha),
                            topLeft = Offset(-grow, -grow),
                            size = Size(size.width + grow * 2, size.height + grow * 2),
                            cornerRadius = CornerRadius(minOf(size.height / 2, GLOW_CORNER.toPx()) + grow),
                        )
                    }
                }
            }
            .drawWithContent {
                drawContent()
                if (focused && ring) {
                    val white = RING_WHITE.toPx()
                    val accent = RING_ACCENT.toPx()
                    // white ring hugging the shape, accent ring outside it
                    translate(-white, -white) {
                        drawOutline(shape.createOutline(Size(size.width + white * 2, size.height + white * 2), layoutDirection, this), Color.White, style = Stroke(white))
                    }
                    val outer = white + accent
                    translate(-outer, -outer) {
                        drawOutline(shape.createOutline(Size(size.width + outer * 2, size.height + outer * 2), layoutDirection, this), look.accent, style = Stroke(accent))
                    }
                }
            }
            .combinedClickable(
                enabled = enabled,
                interactionSource = remember { MutableInteractionSource() },
                indication = null,
                role = Role.Button,
                onLongClick = onLongClick,
                onClick = onClick,
            ),
        // the caller's width and height reach the content: a pill with a fixed width fills it
        propagateMinConstraints = true,
        content = { content(focused) },
    )
}

/**
 * A button: a glass pill, or (primary) the accent gradient; white with dark text when focused.
 * [onFocused] tells the parent (a list scrolls to it).
 */
@Composable
fun TvPill(
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    icon: ImageVector? = null,
    primary: Boolean = false,
    selected: Boolean = false,
    height: Dp = TvDimens.pillHeight,
    labelStyle: androidx.compose.ui.text.TextStyle = TvType.body,
    /** What a screen reader says for an icon-only pill. */
    description: String? = null,
    onFocused: (Boolean) -> Unit = {},
) {
    val look = LocalTvLook.current
    TvFocusable(
        onClick = onClick,
        modifier = modifier.height(height).defaultMinSize(minWidth = height),
        shape = CircleShape,
        onFocused = onFocused,
    ) { focused ->
        val fill: Modifier = when {
            focused -> Modifier.background(Color.White, CircleShape)
            primary || selected -> Modifier.background(look.gradient, CircleShape)
            else -> Modifier.background(Color.White.copy(alpha = 0.10f), CircleShape).border(BorderStroke(1.dp, look.glassLine), CircleShape)
        }
        val ink = when {
            focused -> look.onFocus
            primary || selected -> look.onAccent
            else -> look.ink
        }
        val side = if (label.isEmpty()) height * 0.25f else height * 0.5f
        Box(fill, contentAlignment = Alignment.Center) {
            Row(
                Modifier.padding(horizontal = side),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
            ) {
                // play / pause and the like morph instead of snapping
                if (icon != null) Crossfade(icon, animationSpec = LocalMotion.current.tween(160), label = "tv-pill-icon") { shown ->
                    Icon(shown, label.ifEmpty { description }, tint = ink, modifier = Modifier.size(height * 0.5f))
                }
                if (label.isNotEmpty()) Text(label, style = labelStyle, color = ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}

/** A small status tag: words, never colour alone. */
@Composable
fun TvTag(text: String, modifier: Modifier = Modifier, tone: Color? = null) {
    val look = LocalTvLook.current
    val ink = tone ?: look.ink2
    Text(
        text,
        style = TvType.secondary.copy(fontWeight = androidx.compose.ui.text.font.FontWeight.Bold),
        color = ink,
        maxLines = 1,
        modifier = modifier
            .background(if (tone != null) tone.copy(alpha = 0.18f) else Color.White.copy(alpha = 0.12f), RoundedCornerShape(8.dp))
            .padding(horizontal = 8.dp, vertical = 2.dp),
    )
}

/** The glass card: faint white wash, 1 px line, soft shadow. */
@Composable
fun TvGlass(modifier: Modifier = Modifier, radius: Dp = TvDimens.panelRadius, solid: Boolean = false, content: @Composable BoxScope.() -> Unit) {
    val look = LocalTvLook.current
    val shape = RoundedCornerShape(radius)
    Box(
        modifier
            .background(if (solid) Color(0xFF0B1224) else Color.Black.copy(alpha = 0.18f), shape)
            .background(look.glassFill, shape)
            .border(1.dp, look.glassLine, shape),
        content = content,
    )
}

/** "OK  Open" style hint of one key. */
@Composable
fun TvKeyHint(key: String, label: String) {
    val look = LocalTvLook.current
    Row(verticalAlignment = Alignment.CenterVertically) {
        Text(
            key,
            style = TvType.secondary.copy(fontWeight = androidx.compose.ui.text.font.FontWeight.Bold),
            color = Color.White,
            modifier = Modifier
                .defaultMinSize(minWidth = 22.dp)
                .border(1.dp, Color.White.copy(alpha = 0.3f), RoundedCornerShape(5.dp))
                .padding(horizontal = 6.dp, vertical = 1.dp),
        )
        Spacer(Modifier.width(5.dp))
        Text(label, style = TvType.secondary, color = look.ink2, maxLines = 1)
    }
}

/** A row of [TvKeyHint]s. */
@Composable
fun TvKeyHints(hints: List<Pair<String, String>>, modifier: Modifier = Modifier) {
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(22.dp), verticalAlignment = Alignment.CenterVertically) {
        hints.forEach { (key, label) -> TvKeyHint(key, label) }
    }
}

/**
 * First focus: the element this modifier is on takes focus as soon as it is on screen, so the
 * first OK press already works (nothing has focus on a cold start otherwise). Pass the same
 * [requester] to move focus there again later.
 */
fun Modifier.tvInitialFocus(requester: FocusRequester? = null, key: Any? = Unit, afterMs: Long = 0L, enabled: Boolean = true): Modifier = composed {
    val own = requester ?: remember { FocusRequester() }
    LaunchedEffect(key, enabled) {
        if (!enabled) return@LaunchedEffect
        // a page that slides over another waits for the old one to leave, or the old one's exit takes the focus along
        if (afterMs > 0) delay(afterMs)
        // the node attaches in the same frame as the effect starts; ask again a few times if it is not there yet
        repeat(FOCUS_TRIES) {
            if (runCatching { own.requestFocus() }.getOrDefault(false)) return@LaunchedEffect
            delay(FOCUS_RETRY_MS)
        }
    }
    focusRequester(own)
}

/** The current section's item in the drawer: LEFT from an element with no neighbour on its left goes there. */
val LocalTvDrawerEntry = androidx.compose.runtime.staticCompositionLocalOf<FocusRequester?> { null }

/** LEFT goes to the drawer (an element at the right edge of a screen has no one else on its left). */
fun Modifier.tvLeftToDrawer(): Modifier = composed {
    val entry = LocalTvDrawerEntry.current
    if (entry == null) this else focusProperties { left = entry }
}

private const val FOCUS_TRIES = 8
private const val FOCUS_RETRY_MS = 60L

/**
 * DOWN goes to [target] (the first thing of the list under a header). The focus search alone can pick
 * the list's own container instead of one of its rows, which drops the focus.
 */
@OptIn(androidx.compose.ui.ExperimentalComposeUiApi::class)
fun Modifier.tvDownTo(target: FocusRequester?): Modifier =
    if (target == null) this else focusProperties { down = target }

/** RIGHT goes to [target] (from a column of pills to the list beside it). */
@OptIn(androidx.compose.ui.ExperimentalComposeUiApi::class)
fun Modifier.tvRightTo(target: FocusRequester?): Modifier =
    if (target == null) this else focusProperties { right = target }

/** LEFT goes to [target] (from a list back to the pills or keys beside it). */
@OptIn(androidx.compose.ui.ExperimentalComposeUiApi::class)
fun Modifier.tvLeftTo(target: FocusRequester?): Modifier =
    if (target == null) this else focusProperties { left = target }
