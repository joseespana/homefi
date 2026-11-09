package com.perseusoft.homefi.ui.player

// Pieces of the full-screen player that only draw state, shared by the phone player and the TV screens.

import androidx.compose.animation.Crossfade
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.GraphicEq
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import coil3.compose.AsyncImage
import coil3.request.ImageRequest
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.ShareMember
import com.perseusoft.homefi.data.ImmersiveShade
import com.perseusoft.homefi.player.PlayerUiState
import com.perseusoft.homefi.ui.library.CoverArt
import com.perseusoft.homefi.ui.share.MemberDot
import com.perseusoft.homefi.ui.theme.LocalMotion
import com.perseusoft.homefi.ui.theme.LocalSkin
import com.perseusoft.homefi.ui.theme.currentTitleColor
import com.perseusoft.homefi.ui.theme.itemMotion
import com.perseusoft.homefi.ui.theme.rememberCoverLightness
import com.perseusoft.homefi.ui.theme.skinRowColors

/**
 * The album behind everything: faint in most layouts, the whole backdrop in
 * the immersive one, always fading into the surface so text stays readable.
 */
@Composable
internal fun Backdrop(cover: Any?, strong: Boolean, shade: ImmersiveShade) {
    val surface = LocalSkin.current.base
    val context = LocalContext.current
    // a new album dissolves into the old one
    Crossfade(cover, animationSpec = LocalMotion.current.tween(500), label = "backdrop") { shown ->
        if (shown != null) {
            // the desktop's blurred album: Modifier.blur needs Android 12, so the cover is
            // also decoded tiny and stretched, which blurs it on any version
            AsyncImage(
                model = if (strong) shown else ImageRequest.Builder(context).data(shown).size(24, 24).build(),
                contentDescription = null,
                contentScale = ContentScale.Crop,
                filterQuality = FilterQuality.High,
                alpha = if (strong) 1f else 0.75f,
                modifier = Modifier.fillMaxSize().then(if (strong) Modifier else Modifier.blur(24.dp)),
            )
        }
    }
    // immersive: how much the fade under the title hides the album (0 none, 1 strong)
    val lower = rememberCoverLightness(if (strong && shade == ImmersiveShade.AUTO) cover else null)
    val amount = when (shade) {
        ImmersiveShade.STRONG -> 1f
        ImmersiveShade.SOFT -> 0.5f
        ImmersiveShade.OFF -> 0f
        // the text is the surface's opposite: a cover close to the text's tone needs more shade
        ImmersiveShade.AUTO -> lower?.let { l -> if (surface.luminance() < 0.5f) l else 1f - l }
            ?.let { 0.35f + 0.65f * it }
            ?: 0.8f
    }
    val stops = if (strong) {
        listOf(surface.copy(alpha = 0.35f * amount), Color.Transparent, surface.copy(alpha = 0.92f * amount), surface.copy(alpha = amount))
    } else {
        listOf(surface.copy(alpha = 0.25f), surface.copy(alpha = 0.55f), surface.copy(alpha = 0.92f))
    }
    Box(Modifier.fillMaxSize().background(Brush.verticalGradient(stops)))
}

/**
 * The play queue as a list, the current song marked: what the queue sheet shows, and what a TV
 * panel draws. [addedBy] says who added each song while sharing (by queue id); [onSkipTo] null
 * makes the rows inert (a guest without that right).
 */
@Composable
internal fun QueueList(
    state: PlayerUiState,
    addedBy: Map<Long, ShareMember>,
    onSkipTo: ((Int) -> Unit)?,
    modifier: Modifier = Modifier,
    listState: LazyListState = rememberLazyListState(initialFirstVisibleItemIndex = state.index.coerceAtLeast(0)),
) {
    LazyColumn(state = listState, modifier = modifier) {
        items(state.queue.size, key = { i -> "$i-${state.queue[i].mediaId}" }) { i ->
            val entry = state.queue[i]
            val isCurrent = i == state.index
            val skin = LocalSkin.current
            ListItem(
                modifier = itemMotion().clickable(enabled = onSkipTo != null) { onSkipTo?.invoke(i) },
                colors = if (isCurrent && skin.selection != null) {
                    skinRowColors(i, current = true)
                } else {
                    ListItemDefaults.colors(containerColor = if (skin.stripe != null && i % 2 == 1) skin.stripe else MaterialTheme.colorScheme.surfaceContainerLow)
                },
                leadingContent = {
                    Box(contentAlignment = Alignment.Center) {
                        CoverArt(entry.coverModel(), 44.dp, radius = 6.dp)
                        if (isCurrent) {
                            Box(Modifier.size(44.dp).clip(RoundedCornerShape(6.dp)).background(Color.Black.copy(alpha = 0.45f)))
                            Icon(Icons.Rounded.GraphicEq, null, tint = Color.White)
                        }
                    }
                },
                headlineContent = {
                    Text(
                        entry.title,
                        color = currentTitleColor(isCurrent),
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                },
                supportingContent = {
                    val by = entry.queueId?.let(addedBy::get)
                    if (by == null) {
                        Text(entry.artist, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    } else {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            MemberDot(by.color, 10.dp)
                            Spacer(Modifier.width(6.dp))
                            Text(stringResource(R.string.sharing_by, entry.artist, by.name), maxLines = 1, overflow = TextOverflow.Ellipsis)
                        }
                    }
                },
            )
        }
    }
}
