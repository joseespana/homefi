package com.perseusoft.homefi.ui.player

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.togetherWith
import androidx.compose.runtime.CompositionLocalProvider
import com.perseusoft.homefi.player.PlayerUiState
import com.perseusoft.homefi.ui.theme.LocalCoverVisibility
import com.perseusoft.homefi.ui.theme.LocalMotion
import com.perseusoft.homefi.ui.theme.sharedCover
import androidx.compose.foundation.background
import androidx.compose.material.icons.rounded.DownloadDone
import androidx.compose.material.icons.rounded.Download
import kotlinx.coroutines.launch
import androidx.compose.material.icons.rounded.Cloud
import androidx.compose.material.icons.rounded.PhoneAndroid
import androidx.compose.material.icons.rounded.EmojiEmotions
import androidx.compose.material.icons.rounded.Speed
import androidx.compose.material.icons.rounded.GraphicEq
import androidx.compose.material.icons.rounded.Album
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.draw.clip
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.KeyboardArrowDown
import androidx.compose.material.icons.rounded.MusicNote
import androidx.compose.material.icons.rounded.Groups
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Repeat
import androidx.compose.material.icons.rounded.RepeatOne
import androidx.compose.material.icons.rounded.Shuffle
import androidx.compose.material.icons.rounded.SkipNext
import androidx.compose.material.icons.rounded.SkipPrevious
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import com.perseusoft.homefi.ui.theme.LocalSkin
import com.perseusoft.homefi.ui.theme.chrome
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.media3.common.Player
import com.perseusoft.homefi.R
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.library.EmbeddedCover
import com.perseusoft.homefi.player.NowPlaying
import com.perseusoft.homefi.player.QueueEntry
import com.perseusoft.homefi.player.isFilePath
import com.perseusoft.homefi.player.tidalId
import com.perseusoft.homefi.ui.QualityBadge
import com.perseusoft.homefi.ui.formatDuration
import com.perseusoft.homefi.ui.library.CoverArt

/** Cover model for the current item: embedded art of a library file, or what ExoPlayer extracted. */
/** Cover model of a queue entry. */
internal fun QueueEntry.coverModel(): Any? = if (isFilePath(mediaId)) EmbeddedCover(mediaId) else artworkUri

internal fun NowPlaying.coverModel(): Any? = if (isFilePath(mediaId)) EmbeddedCover(mediaId) else artwork ?: artworkUri

/**
 * Compact bar above the navigation bar while something is loaded: this
 * phone's player, or the room's song while this phone is in someone else's
 * room ([source] is then the guest's, and its rules grey out the buttons).
 * It rises in when something starts and folds away when nothing is left;
 * [coverShown] false hands its cover to the full-screen player above.
 */
@Composable
fun MiniPlayer(source: PlayerSource, onOpen: () -> Unit, coverShown: Boolean = true) {
    @Suppress("NAME_SHADOWING") val source = source.felt()
    val state by source.state.collectAsStateWithLifecycle()
    val caps by source.capabilities.collectAsStateWithLifecycle()
    val motion = LocalMotion.current
    val room = caps.remoteName
    // in a room the bar stays even between songs, as the way back to it
    AnimatedVisibility(
        visible = state.current != null || room != null,
        enter = motion.reveal(),
        exit = motion.conceal(),
    ) {
        MiniPlayerBar(source, state, caps, onOpen, coverShown)
    }
}

@Composable
private fun MiniPlayerBar(source: PlayerSource, state: PlayerUiState, caps: PlayerCapabilities, onOpen: () -> Unit, coverShown: Boolean) {
    val motion = LocalMotion.current
    val room = caps.remoteName
    val current = state.current

    val bar = LocalSkin.current.miniPlayer
    Surface(
        color = if (bar == null) MaterialTheme.colorScheme.surfaceContainerHigh else Color.Transparent,
        contentColor = bar?.content ?: MaterialTheme.colorScheme.onSurface,
        modifier = (if (bar == null) Modifier else Modifier.chrome(bar, edgeAtTop = true)).clickable(onClick = onOpen),
    ) {
        Column {
            val progress = if (state.durationMs > 0) state.positionMs.toFloat() / state.durationMs else 0f
            LinearProgressIndicator(progress = { progress }, modifier = Modifier.fillMaxWidth().height(2.dp), drawStopIndicator = {})
            Row(Modifier.padding(horizontal = 12.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                MiniCover(current, coverShown)
                Spacer(Modifier.width(12.dp))
                // a new song ticks in from below
                AnimatedContent(
                    targetState = current,
                    contentKey = { it?.mediaId },
                    transitionSpec = { motion.ticker() },
                    modifier = Modifier.weight(1f),
                    label = "mini-title",
                ) { shown ->
                    Column {
                        Text(
                            shown?.title ?: stringResource(R.string.sharing_nothing_playing),
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            fontWeight = FontWeight.SemiBold,
                        )
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            if (room != null) {
                                Icon(Icons.Rounded.Groups, null, Modifier.size(14.dp), tint = MaterialTheme.colorScheme.primary)
                                Spacer(Modifier.width(4.dp))
                            }
                            Text(
                                when {
                                    room == null -> shown?.artist.orEmpty().ifBlank { stringResource(R.string.unknown_artist) }
                                    shown == null || shown.artist.isBlank() -> stringResource(R.string.player_source_hub, room)
                                    else -> "${shown.artist} · ${stringResource(R.string.player_source_hub, room)}"
                                },
                                style = MaterialTheme.typography.bodySmall,
                                color = bar?.contentVariant ?: MaterialTheme.colorScheme.onSurfaceVariant,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                    }
                }
                IconButton(onClick = source::togglePlayPause, enabled = caps.canPlayPause && current != null) {
                    if (state.isBuffering) CircularProgressIndicator(Modifier.size(36.dp), strokeWidth = 2.dp)
                    PlayPauseIcon(state.isPlaying)
                }
                // a guest: skip or vote, as the room allows
                val canNext = if (room != null) caps.canNext else state.index < state.queue.lastIndex || state.repeatMode != Player.REPEAT_MODE_OFF
                IconButton(onClick = source::next, enabled = canNext) {
                    Icon(Icons.Rounded.SkipNext, stringResource(R.string.player_next))
                }
            }
        }
    }
}

/** The bar's cover; its 44 dp slot stays while the cover is up in the full player. */
@Composable
private fun MiniCover(current: NowPlaying?, shown: Boolean) {
    val motion = LocalMotion.current
    Box(Modifier.size(44.dp), contentAlignment = Alignment.Center) {
        AnimatedVisibility(
            visible = shown,
            enter = fadeIn(motion.tween(150)),
            exit = fadeOut(motion.tween(150)),
        ) {
            CompositionLocalProvider(LocalCoverVisibility provides this) {
                CoverArt(
                    current?.coverModel(),
                    44.dp,
                    icon = if (current == null) Icons.Rounded.Groups else Icons.Rounded.MusicNote,
                    modifier = Modifier.sharedCover(current?.mediaId),
                )
            }
        }
    }
}

/** Play / pause, the icon turning into the other with a small spin-and-scale. */
@Composable
internal fun PlayPauseIcon(playing: Boolean, modifier: Modifier = Modifier) {
    val motion = LocalMotion.current
    AnimatedContent(
        targetState = playing,
        transitionSpec = {
            if (!motion.moves) {
                motion.fadeThrough()
            } else {
                (fadeIn(motion.tween(150)) + scaleIn(motion.move(), initialScale = 0.6f)) togetherWith
                    (fadeOut(motion.tween(100)) + scaleOut(motion.tween(100), targetScale = 0.6f)) using null
            }
        },
        label = "play-pause",
    ) { isPlaying ->
        Icon(
            if (isPlaying) Icons.Rounded.Pause else Icons.Rounded.PlayArrow,
            stringResource(if (isPlaying) R.string.player_pause else R.string.player_play),
            modifier,
        )
    }
}
