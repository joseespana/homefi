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
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyGridState
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.itemsIndexed
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.ErrorOutline
import androidx.compose.material.icons.rounded.GraphicEq
import androidx.compose.material.icons.rounded.MusicNote
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.res.stringResource
import com.perseusoft.homefi.R
import com.perseusoft.homefi.ui.library.CoverArt
import com.perseusoft.homefi.ui.theme.LocalMotion

// The pieces the TV's content screens share (Library, Search, TIDAL, Downloads): a song row, the
// tab pills, a grid of covers, the long-press menu, a detail page and the loading / error states.
// They sit on TvFocusable / TvPill / TvGlass, so every screen has the one focus look.

internal const val TV_GRID_COLUMNS = 6
private val ROW_HEIGHT = 44.dp
private val ROW_COVER = 32.dp
private val GRID_START = 12.dp
private val GRID_END = 24.dp
private const val DIMMED_ALPHA = 0.45f

/** First focus when a screen opens: [key] changes ask for it again. LEFT from this element reaches the drawer. */
fun Modifier.tvEdge(): Modifier = tvLeftToDrawer()

/** A thing of the list that takes focus back after a detail page closed (when [restore] is true). */
fun Modifier.tvRestoreFocus(restore: Boolean, nonce: Int): Modifier = tvInitialFocus(key = nonce, afterMs = PAGE_FOCUS_DELAY_MS, enabled = restore)

/**
 * One line of a list (a song, a download, a result): cover, title, subtitle and a tag or time on the
 * right. Focused it turns white; the playing one is teal with the bars icon; [dimmed] is a song
 * on a drive that is not plugged in.
 */
@Composable
fun TvRow(
    title: String,
    subtitle: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    cover: Any? = null,
    coverIcon: ImageVector = Icons.Rounded.MusicNote,
    round: Boolean = false,
    playing: Boolean = false,
    dimmed: Boolean = false,
    tag: String? = null,
    trailing: String? = null,
    onLongClick: (() -> Unit)? = null,
    onFocused: (Boolean) -> Unit = {},
) {
    val look = LocalTvLook.current
    TvFocusable(
        onClick = onClick,
        onLongClick = onLongClick,
        modifier = modifier.fillMaxWidth().height(ROW_HEIGHT),
        shape = RoundedCornerShape(12.dp),
        focusedScale = 1.02f,
        glow = false,
        onFocused = onFocused,
    ) { focused ->
        Row(
            Modifier
                .fillMaxSize()
                .alpha(if (dimmed) DIMMED_ALPHA else 1f)
                .then(if (focused) Modifier.background(Color.White, RoundedCornerShape(12.dp)) else Modifier)
                .padding(horizontal = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            val radius = if (round) ROW_COVER / 2 else 6.dp
            Box(contentAlignment = Alignment.Center) {
                CoverArt(cover, ROW_COVER, radius = radius, icon = coverIcon)
                if (playing) {
                    Box(Modifier.size(ROW_COVER).background(Color.Black.copy(alpha = 0.5f), RoundedCornerShape(radius)))
                    Icon(Icons.Rounded.GraphicEq, null, tint = look.accent2, modifier = Modifier.size(18.dp))
                }
            }
            Column(Modifier.weight(1f)) {
                val ink = when {
                    focused -> look.onFocus
                    playing -> look.accent2
                    else -> look.ink
                }
                Text(title, style = TvType.cardTitle.copy(fontSize = 14.sp), color = ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
                if (subtitle.isNotBlank()) {
                    Text(subtitle, style = TvType.secondary, color = if (focused) ROW_SUB_FOCUSED else look.ink2, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
            if (tag != null) TvTag(tag, tone = if (focused) null else look.accent2)
            if (trailing != null) Text(trailing, style = TvType.secondary, color = if (focused) ROW_SUB_FOCUSED else look.ink2, maxLines = 1)
        }
    }
}

/** The grey of a secondary line on the white focus fill. */
internal val ROW_SUB_FOCUSED = Color(0xFF3A4660)

/**
 * The view switcher (Songs / Albums / ...): pills that select as they take focus (LEFT / RIGHT),
 * and that return the focus to the selected one when it comes back from the list below.
 */
@OptIn(androidx.compose.ui.ExperimentalComposeUiApi::class)
@Composable
fun TvTabs(
    labels: List<String>,
    selected: Int,
    onSelect: (Int) -> Unit,
    modifier: Modifier = Modifier,
    /** Changes ask the selected pill for focus again (a section that just opened). */
    entryKey: Int = 0,
    /** DOWN from a pill goes here (the first item of the view below). */
    downTo: FocusRequester? = null,
) {
    val requesters = remember(labels.size) { List(labels.size) { FocusRequester() } }
    Row(
        modifier,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        labels.forEachIndexed { i, label ->
            TvPill(
                label,
                { onSelect(i) },
                // the same modifier chain on every pill, so a pill keeps its focus when the selection moves
                Modifier
                    .tvInitialFocus(requesters[i], key = entryKey, enabled = i == selected)
                    .tvDownTo(downTo)
                    .then(if (i == 0) Modifier.tvLeftToDrawer() else Modifier),
                selected = i == selected,
                height = 30.dp,
                labelStyle = TvType.body.copy(fontSize = 14.sp),
                onFocused = { if (it) onSelect(i) },
            )
        }
    }
}

/** Screen title with a count / subtitle line and a slot on the right (clock, tags, pills). */
@Composable
fun TvScreenHeader(
    title: String,
    subtitle: String?,
    modifier: Modifier = Modifier,
    titleStyle: androidx.compose.ui.text.TextStyle = TvType.title,
    trailing: @Composable () -> Unit = {},
) {
    val look = LocalTvLook.current
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.Bottom) {
        Column(Modifier.weight(1f)) {
            Text(title, style = titleStyle, color = look.ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (!subtitle.isNullOrBlank()) Text(subtitle, style = TvType.secondary, color = look.ink2, maxLines = 1)
        }
        trailing()
    }
}

/**
 * A grid of cover cards sized so [TV_GRID_COLUMNS] fill the width: LEFT from the first column goes
 * to the drawer, [restoreKey] takes focus back after a detail page, the scroll [state] survives it.
 */
@Composable
fun <T> TvCoverGrid(
    items: List<T>,
    key: (T) -> Any,
    modifier: Modifier = Modifier,
    state: LazyGridState = rememberLazyGridState(),
    restoreKey: Any? = null,
    restoreNonce: Int = 0,
    /** The header above DOWN-moves to this: the first card takes it. */
    firstItem: FocusRequester? = null,
    card: @Composable (item: T, size: Dp, modifier: Modifier) -> Unit,
) {
    BoxWithConstraints(modifier) {
        val size = (maxWidth - GRID_START - GRID_END - TvDimens.gap * (TV_GRID_COLUMNS - 1)) / TV_GRID_COLUMNS
        LazyVerticalGrid(
            columns = GridCells.Fixed(TV_GRID_COLUMNS),
            state = state,
            modifier = Modifier.fillMaxSize(),
            // room for the focused card's growth, ring and glow: a lazy grid clips
            contentPadding = PaddingValues(start = GRID_START, end = GRID_END, top = 12.dp, bottom = 28.dp),
            horizontalArrangement = Arrangement.spacedBy(TvDimens.gap),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            itemsIndexed(items, key = { _, it -> key(it) }) { index, item ->
                val edge = if (index % TV_GRID_COLUMNS == 0) Modifier.tvLeftToDrawer() else Modifier
                val first = if (index == 0 && firstItem != null) Modifier.focusRequester(firstItem) else Modifier
                card(item, size, first.then(edge).tvRestoreFocus(restoreKey != null && key(item) == restoreKey, restoreNonce))
            }
        }
    }
}

/** What a long press of OK offers on a card or a row. */
data class TvAction(val icon: ImageVector, val label: String, val onClick: () -> Unit)

/** The long-press menu: a small glass list in its own window, the first action focused, BACK closes. */
@Composable
fun TvActionMenu(title: String, subtitle: String?, actions: List<TvAction>, onDismiss: () -> Unit) {
    val look = LocalTvLook.current
    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Box(Modifier.fillMaxSize().background(Color(0xCC040510)), contentAlignment = Alignment.Center) {
            TvGlass(Modifier.width(300.dp), solid = true) {
                Column(Modifier.padding(horizontal = 14.dp, vertical = 16.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text(title, style = TvType.section, color = look.ink, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(horizontal = 6.dp))
                    if (!subtitle.isNullOrBlank()) {
                        Text(subtitle, style = TvType.secondary, color = look.ink2, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(horizontal = 6.dp))
                    }
                    Spacer(Modifier.height(8.dp))
                    actions.forEachIndexed { i, action ->
                        TvFocusable(
                            onClick = {
                                onDismiss()
                                action.onClick()
                            },
                            modifier = Modifier.fillMaxWidth().height(40.dp).then(if (i == 0) Modifier.tvInitialFocus() else Modifier),
                            shape = RoundedCornerShape(12.dp),
                            focusedScale = 1.03f,
                            glow = false,
                        ) { focused ->
                            val ink = if (focused) look.onFocus else look.ink
                            Row(
                                Modifier
                                    .fillMaxSize()
                                    .then(if (focused) Modifier.background(Color.White, RoundedCornerShape(12.dp)) else Modifier)
                                    .padding(horizontal = 10.dp),
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(12.dp),
                            ) {
                                Icon(action.icon, null, tint = if (focused) look.onFocus else look.accent2, modifier = Modifier.size(20.dp))
                                Text(action.label, style = TvType.body, color = ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            }
                        }
                    }
                }
            }
        }
    }
}

/**
 * A detail page (album, artist, playlist): the cover with its title, tags and pills in a column on
 * the left, the songs on the right. The first pill takes focus; BACK is handled by the caller.
 */
@Composable
fun TvDetailPage(
    title: String,
    subtitle: String,
    cover: Any?,
    modifier: Modifier = Modifier,
    round: Boolean = false,
    coverIcon: ImageVector = Icons.Rounded.MusicNote,
    tags: List<String> = emptyList(),
    pills: @Composable () -> Unit,
    content: @Composable () -> Unit,
) {
    val look = LocalTvLook.current
    val safe = tvSafePadding()
    Row(
        modifier.fillMaxSize().padding(top = safe.calculateTopPadding(), end = safe.calculateRightPadding(androidx.compose.ui.unit.LayoutDirection.Ltr)),
        horizontalArrangement = Arrangement.spacedBy(22.dp),
    ) {
        Column(Modifier.width(DETAIL_PANE).padding(start = 12.dp, top = 8.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            CoverArt(cover, DETAIL_COVER, radius = if (round) DETAIL_COVER / 2 else 14.dp, icon = coverIcon)
            Spacer(Modifier.height(4.dp))
            Text(title, style = TvType.section.copy(fontSize = 22.sp), color = look.ink, maxLines = 3, overflow = TextOverflow.Ellipsis)
            if (subtitle.isNotBlank()) Text(subtitle, style = TvType.secondary.copy(fontSize = 13.sp), color = look.ink2, maxLines = 2, overflow = TextOverflow.Ellipsis)
            if (tags.isNotEmpty()) Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) { tags.forEach { TvTag(it) } }
            Spacer(Modifier.height(6.dp))
            Column(verticalArrangement = Arrangement.spacedBy(8.dp), content = { pills() })
        }
        Box(Modifier.weight(1f).fillMaxSize(), content = { content() })
    }
}

private val DETAIL_PANE = 190.dp
private val DETAIL_COVER = 150.dp
internal val DETAIL_PILL_WIDTH = 170.dp

/** A slow shimmer of grey rows while a list loads (still under motion OFF). */
@Composable
fun TvSkeleton(modifier: Modifier = Modifier, rows: Int = 7, round: Boolean = false) {
    val motion = LocalMotion.current
    val pulse = if (motion.enabled) {
        rememberInfiniteTransition(label = "tv-skeleton").animateFloat(
            initialValue = 0.5f,
            targetValue = 1f,
            animationSpec = infiniteRepeatable(tween(SKELETON_MS), RepeatMode.Reverse),
            label = "tv-skeleton-alpha",
        ).value
    } else {
        0.75f
    }
    Column(modifier.fillMaxWidth().padding(start = 12.dp, top = 12.dp).alpha(pulse), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        repeat(rows) {
            Row(Modifier.height(ROW_HEIGHT).padding(horizontal = 10.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Box(Modifier.size(ROW_COVER).background(Color.White.copy(alpha = 0.10f), if (round) CircleShape else RoundedCornerShape(6.dp)))
                Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Box(Modifier.size(width = 230.dp, height = 9.dp).background(Color.White.copy(alpha = 0.10f), CircleShape))
                    Box(Modifier.size(width = 130.dp, height = 7.dp).background(Color.White.copy(alpha = 0.06f), CircleShape))
                }
            }
        }
    }
}

private const val SKELETON_MS = 1_600

/** An error that names what failed, with a red-lined glass card and a focused "Try again". */
@Composable
fun TvErrorState(message: String, onRetry: () -> Unit, entryKey: Int, modifier: Modifier = Modifier) {
    val look = LocalTvLook.current
    Box(modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        TvGlass(Modifier.width(300.dp).border(1.dp, look.error.copy(alpha = 0.7f), RoundedCornerShape(TvDimens.panelRadius)), radius = TvDimens.panelRadius) {
            Column(
                Modifier.fillMaxWidth().padding(22.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Icon(Icons.Rounded.ErrorOutline, null, tint = look.error, modifier = Modifier.size(34.dp))
                Text(message, style = TvType.secondary.copy(fontSize = 13.sp), color = look.ink2, textAlign = TextAlign.Center, maxLines = 4, overflow = TextOverflow.Ellipsis)
                TvPill(stringResource(R.string.tv_retry), onRetry, Modifier.tvInitialFocus(key = entryKey), icon = Icons.Rounded.Refresh, primary = true)
            }
        }
    }
}

/** A detail page takes focus once the page it slid over has left (the transition lasts about this long). */
internal const val PAGE_FOCUS_DELAY_MS = 260L
