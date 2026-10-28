package com.perseusoft.homefi.ui.share

import com.perseusoft.homefi.ui.ConfirmDialog
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.KeyboardArrowUp
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.automirrored.rounded.PlaylistAdd
import androidx.compose.material.icons.rounded.Album
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.HowToVote
import androidx.compose.material.icons.rounded.KeyboardArrowDown
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material.icons.rounded.SkipNext
import androidx.compose.material.icons.rounded.Speaker
import androidx.compose.material.icons.automirrored.rounded.VolumeUp
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledIconButton
import com.perseusoft.homefi.ui.theme.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.LeadingIconTab
import androidx.compose.material3.TabRow
import androidx.compose.material3.Text
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.TabRowDefaults
import androidx.compose.ui.graphics.Color
import com.perseusoft.homefi.ui.player.PlayerContent
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.ShareCollection
import com.perseusoft.homefi.core.ShareEditMode
import com.perseusoft.homefi.core.SharePermissions
import com.perseusoft.homefi.core.ShareQueueItem
import com.perseusoft.homefi.core.ShareSkipMode
import com.perseusoft.homefi.core.ShareState
import com.perseusoft.homefi.core.ShareTrack
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.share.GuestUi
import com.perseusoft.homefi.ui.formatDuration
import com.perseusoft.homefi.ui.theme.SkinScreen
import com.perseusoft.homefi.ui.library.CoverArt
import kotlinx.coroutines.delay

/** The guest screen's tabs, in order. */
private const val TAB_PLAYER = 0
private const val TAB_ROOM = 1
private const val TAB_SEARCH = 2

/**
 * A guest's remote for the hub: the hub's song in the player layout picked in
 * Settings, the room (who added what, the queue, skip / vote / pause per the
 * hub's rules, "play on this phone" with the speaker delay) and search
 * through the hub.
 */
@Composable
fun GuestScreen(player: GuestPlayer, onClose: () -> Unit) {
    val app = LocalContext.current.homefi
    val share = app.share
    val ui by share.guest.collectAsStateWithLifecycle()
    val snackbar = remember { SnackbarHostState() }
    var tab by rememberSaveable { mutableIntStateOf(if (app.settings.current.share.guestOpensPlayer) TAB_PLAYER else TAB_ROOM) }

    BackHandler {
        if (tab == TAB_SEARCH && ui.page != null) share.closePage() else onClose()
    }
    LaunchedEffect(Unit) {
        share.guestMessages.collect { snackbar.showSnackbar(it) }
    }

    Box(Modifier.fillMaxSize()) {
        if (tab == TAB_PLAYER) {
            // the same player as on this phone's own music, the guest bar on its background
            // its layout and output buttons join the guest bar (the Room tab is the queue)
            PlayerContent(
                player,
                showShare = false,
                showQueue = false,
                header = { actions -> GuestHeader(ui, tab, onTab = { tab = it }, onClose = onClose, onPlayer = true, actions = actions) },
            )
        } else {
            // the skin's base (and backdrop), opaque over the app
            SkinScreen {
                Column(Modifier.fillMaxSize().systemBarsPadding()) {
                    GuestHeader(ui, tab, onTab = { tab = it }, onClose = onClose, onPlayer = false)
                    when (tab) {
                        TAB_ROOM -> PlayingTab(ui)
                        else -> SearchTab(ui, onSent = share::notifySent)
                    }
                }
            }
        }
        SnackbarHost(snackbar, Modifier.align(Alignment.BottomCenter).systemBarsPadding())
    }
}

/**
 * One compact bar: close, session name and status, the player's [actions]
 * (on the Player tab), Leave; then the tabs with the icon beside the label.
 * See-through over the player, so short screens keep room for the cover.
 */
@Composable
private fun GuestHeader(
    ui: GuestUi,
    tab: Int,
    onTab: (Int) -> Unit,
    onClose: () -> Unit,
    onPlayer: Boolean,
    actions: @Composable RowScope.() -> Unit = {},
) {
    val share = LocalContext.current.homefi.share
    Row(Modifier.fillMaxWidth().height(52.dp).padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        IconButton(onClick = onClose, modifier = Modifier.size(48.dp)) {
            Icon(Icons.Rounded.KeyboardArrowDown, stringResource(R.string.player_close))
        }
        Column(Modifier.weight(1f)) {
            Text(ui.sessionName, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(
                stringResource(if (ui.playingHere) R.string.sharing_guest_status_party else R.string.sharing_guest_status_remote),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.primary,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        actions()
        var leaving by remember { mutableStateOf(false) }
        if (leaving) {
            ConfirmDialog(
                title = stringResource(R.string.sharing_leave_title, ui.sessionName),
                text = stringResource(R.string.sharing_leave_text),
                confirm = stringResource(R.string.sharing_leave),
                onConfirm = share::leave,
                onDismiss = { leaving = false },
            )
        }
        TextButton(onClick = { leaving = true }, contentPadding = PaddingValues(horizontal = 8.dp)) {
            Text(stringResource(R.string.sharing_leave), color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.titleSmall)
        }
    }
    val tabs = listOf(
        Triple(TAB_PLAYER, R.string.sharing_tab_player, Icons.Rounded.Album),
        Triple(TAB_ROOM, R.string.sharing_tab_room, Icons.AutoMirrored.Rounded.QueueMusic),
        Triple(TAB_SEARCH, R.string.sharing_tab_search, Icons.Rounded.Search),
    )
    TabRow(
        selectedTabIndex = tab,
        // over the player the tabs take its background and ink (retro skins included)
        containerColor = if (onPlayer) Color.Transparent else TabRowDefaults.primaryContainerColor,
        contentColor = if (onPlayer) LocalContentColor.current else TabRowDefaults.primaryContentColor,
        divider = { if (!onPlayer) HorizontalDivider() },
    ) {
        tabs.forEach { (index, label, icon) ->
            // icon beside the label: 48 dp instead of 72
            LeadingIconTab(
                selected = tab == index,
                onClick = { onTab(index) },
                text = { Text(stringResource(label), maxLines = 1, overflow = TextOverflow.Ellipsis) },
                icon = { Icon(icon, null, Modifier.size(20.dp)) },
            )
        }
    }
}

// ------------------------------------------------------------------ playing

@Composable
private fun PlayingTab(ui: GuestUi) {
    val state = ui.state
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
        item { NowPlayingCard(ui) }
        if (state != null) {
            item { Controls(state) }
            item { PartyCard(ui) }
            item {
                Text(
                    stringResource(R.string.sharing_up_next, state.queue.size),
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.Bold,
                    modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 16.dp, bottom = 4.dp),
                )
            }
            if (state.queue.isEmpty()) {
                item {
                    Text(
                        stringResource(R.string.sharing_queue_empty),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                    )
                }
            }
            itemsIndexed(state.queue, key = { _, it -> it.queueId.toLong() }) { i, item -> QueueRow(item, i, state.queue.size, ui.guestId, state.permissions) }
        }
    }
}

@Composable
private fun NowPlayingCard(ui: GuestUi) {
    val share = LocalContext.current.homefi.share
    val now = ui.state?.nowPlaying
    var position by remember { mutableLongStateOf(0L) }
    LaunchedEffect(ui.state) {
        while (true) {
            position = share.guestPositionMs()
            delay(500)
        }
    }
    Column(Modifier.fillMaxWidth().padding(16.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            CoverArt(share.resolveUrl(now?.track?.coverUrl), 112.dp, radius = 12.dp)
            Spacer(Modifier.width(16.dp))
            Column(Modifier.weight(1f)) {
                Text(
                    now?.track?.title ?: stringResource(R.string.sharing_nothing_playing),
                    style = MaterialTheme.typography.titleLarge,
                    fontWeight = FontWeight.Bold,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
                now?.track?.artist?.takeIf(String::isNotBlank)?.let {
                    Text(it, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
                now?.addedBy?.let { by ->
                    Spacer(Modifier.height(6.dp))
                    AddedBy(by.name, by.color)
                }
            }
        }
        val duration = now?.track?.durationMs?.toLong() ?: 0L
        if (now != null && duration > 0) {
            Spacer(Modifier.height(16.dp))
            LinearProgressIndicator(
                progress = { (position.toFloat() / duration).coerceIn(0f, 1f) },
                modifier = Modifier.fillMaxWidth().height(6.dp),
            )
            Row(Modifier.fillMaxWidth().padding(top = 4.dp), horizontalArrangement = Arrangement.SpaceBetween) {
                Text(formatDuration(position), style = MaterialTheme.typography.labelMedium)
                Text(formatDuration(duration), style = MaterialTheme.typography.labelMedium)
            }
        }
    }
}

@Composable
private fun AddedBy(name: String, color: String) {
    Row(
        Modifier
            .background(MaterialTheme.colorScheme.surfaceContainerHigh, RoundedCornerShape(50))
            .padding(horizontal = 10.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        MemberDot(color, 10.dp)
        Spacer(Modifier.width(6.dp))
        Text(name, style = MaterialTheme.typography.labelMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

/** Skip (or vote), pause and volume: what the hub doesn't allow stays greyed out. */
@Composable
private fun Controls(state: ShareState) {
    val share = LocalContext.current.homefi.share
    val playback = state.permissions.playback
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp)) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(16.dp, Alignment.CenterHorizontally), verticalAlignment = Alignment.CenterVertically) {
            FilledIconButton(onClick = share::toggle, enabled = playback, modifier = Modifier.size(72.dp)) {
                Icon(
                    if (state.playing) Icons.Rounded.Pause else Icons.Rounded.PlayArrow,
                    stringResource(if (state.playing) R.string.player_pause else R.string.player_play),
                    Modifier.size(40.dp),
                )
            }
            when (state.permissions.skip) {
                ShareSkipMode.VOTE -> FilledTonalButton(onClick = share::voteSkip, modifier = Modifier.height(56.dp)) {
                    Icon(Icons.Rounded.HowToVote, null)
                    Spacer(Modifier.width(8.dp))
                    Text(stringResource(R.string.sharing_vote_skip, state.skipVotes.toInt(), state.skipVotesNeeded.toInt()))
                }
                else -> FilledTonalButton(
                    onClick = share::skip,
                    enabled = state.permissions.skip == ShareSkipMode.EVERYONE,
                    modifier = Modifier.height(56.dp),
                ) {
                    Icon(Icons.Rounded.SkipNext, null)
                    Spacer(Modifier.width(8.dp))
                    Text(stringResource(R.string.player_next))
                }
            }
        }
        if (playback) {
            // the slider moves locally; the hub gets the value when you let go
            var volume by remember(state.volume) { mutableFloatStateOf(state.volume ?: 1f) }
            Row(Modifier.fillMaxWidth().padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                Icon(Icons.AutoMirrored.Rounded.VolumeUp, stringResource(R.string.sharing_volume), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                Spacer(Modifier.width(12.dp))
                Slider(value = volume, onValueChange = { volume = it }, onValueChangeFinished = { share.volume(volume) }, modifier = Modifier.weight(1f))
            }
        }
    }
}

/** Party mode on this phone: the switch and the speaker delay, applied live. */
@Composable
private fun PartyCard(ui: GuestUi) {
    val app = LocalContext.current.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val streaming = ui.state?.audio != null
    Surface(
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
        shape = RoundedCornerShape(16.dp),
        modifier = Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, top = 16.dp),
    ) {
        Column(Modifier.padding(16.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(Icons.Rounded.Speaker, null, tint = MaterialTheme.colorScheme.primary)
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(stringResource(R.string.sharing_play_here), style = MaterialTheme.typography.titleMedium)
                    Text(
                        stringResource(if (streaming) R.string.sharing_play_here_hint else R.string.sharing_play_here_off),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                Switch(checked = ui.playAlong, onCheckedChange = app.share::setPlayAlong)
            }
            if (ui.playAlong) {
                val delayMs = settings.share.speakerDelayMs
                Spacer(Modifier.height(8.dp))
                Text(stringResource(R.string.sharing_speaker_delay, delayMs), style = MaterialTheme.typography.labelLarge)
                Slider(
                    value = delayMs.toFloat(),
                    onValueChange = { v -> app.settings.updateShare { it.copy(speakerDelayMs = (v / 10).toInt() * 10) } },
                    valueRange = -500f..500f,
                )
                Text(
                    stringResource(R.string.sharing_speaker_delay_hint),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                ui.roundTripMs?.let {
                    Text(
                        stringResource(R.string.sharing_sync_precision, (it / 2).toInt()),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    }
}

/**
 * A song waiting: tap it (or ⋮) for play now, play next, move up / down and
 * remove. What the hub's rules don't allow stays greyed, with the reason.
 */
@Composable
private fun QueueRow(item: ShareQueueItem, index: Int, count: Int, myId: String, permissions: SharePermissions) {
    val share = LocalContext.current.homefi.share
    val canEdit = when (permissions.edit) {
        ShareEditMode.ALL -> true
        ShareEditMode.OWN -> item.addedBy.guestId == myId
        ShareEditMode.NONE -> false
    }
    val canPlayNow = permissions.playback || permissions.skip == ShareSkipMode.EVERYONE
    var menu by remember { mutableStateOf(false) }
    var removing by remember { mutableStateOf(false) }
    if (removing) {
        ConfirmDialog(
            title = stringResource(R.string.sharing_remove_title, item.track.title),
            confirm = stringResource(R.string.sharing_remove),
            onConfirm = { share.remove(item.queueId) },
            onDismiss = { removing = false },
        )
    }
    ListItem(
        modifier = Modifier.clickable { menu = true },
        colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surface),
        leadingContent = { CoverArt(share.resolveUrl(item.track.coverUrl), 48.dp, radius = 6.dp) },
        headlineContent = { Text(item.track.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        supportingContent = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                MemberDot(item.addedBy.color, 10.dp)
                Spacer(Modifier.width(6.dp))
                Text(
                    stringResource(R.string.sharing_by, item.track.artist, item.addedBy.name),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        },
        trailingContent = {
            Box {
                IconButton(onClick = { menu = true }, modifier = Modifier.size(48.dp)) {
                    Icon(Icons.Rounded.MoreVert, stringResource(R.string.sharing_song_options))
                }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    val act = { block: () -> Unit -> { menu = false; block() } }
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.sharing_play_now)) },
                        leadingIcon = { Icon(Icons.Rounded.PlayArrow, null) },
                        enabled = canPlayNow,
                        onClick = act { share.playNow(item.queueId) },
                    )
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.queue_play_next)) },
                        leadingIcon = { Icon(Icons.Rounded.SkipNext, null) },
                        enabled = canEdit && index > 0,
                        onClick = act { share.move(item.queueId, 0) },
                    )
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.sharing_move_up)) },
                        leadingIcon = { Icon(Icons.Rounded.KeyboardArrowUp, null) },
                        enabled = canEdit && index > 0,
                        onClick = act { share.move(item.queueId, index - 1) },
                    )
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.sharing_move_down)) },
                        leadingIcon = { Icon(Icons.Rounded.KeyboardArrowDown, null) },
                        enabled = canEdit && index < count - 1,
                        onClick = act { share.move(item.queueId, index + 1) },
                    )
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.sharing_remove)) },
                        leadingIcon = { Icon(Icons.Rounded.Delete, null) },
                        enabled = canEdit,
                        onClick = act { removing = true },
                    )
                    // why some of it is greyed out
                    val reasons = listOfNotNull(
                        if (!canPlayNow) R.string.sharing_play_now_denied else null,
                        when {
                            canEdit -> null
                            permissions.edit == ShareEditMode.OWN -> R.string.sharing_edit_own_only
                            else -> R.string.sharing_edit_denied
                        },
                    )
                    if (reasons.isNotEmpty()) {
                        HorizontalDivider()
                        reasons.forEach { r ->
                            Text(
                                stringResource(r),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.padding(horizontal = 16.dp, vertical = 6.dp).widthIn(max = 240.dp),
                            )
                        }
                    }
                }
            }
        },
    )
}

// ------------------------------------------------------------------- search

@Composable
private fun SearchTab(ui: GuestUi, onSent: (String) -> Unit) {
    val share = LocalContext.current.homefi.share
    val keyboard = LocalSoftwareKeyboardController.current
    var query by rememberSaveable { mutableStateOf("") }
    val explicitOk = ui.state?.permissions?.allowExplicit ?: true

    Column(Modifier.fillMaxSize()) {
        OutlinedTextField(
            value = query,
            onValueChange = { query = it },
            placeholder = { Text(stringResource(R.string.sharing_search_hint)) },
            leadingIcon = { Icon(Icons.Rounded.Search, null) },
            singleLine = true,
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
            keyboardActions = KeyboardActions(onSearch = {
                keyboard?.hide()
                share.search(query)
            }),
            modifier = Modifier.fillMaxWidth().padding(16.dp),
        )
        if (ui.searching) {
            Box(Modifier.fillMaxWidth().padding(24.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
        }
        val page = ui.page
        LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
            if (page != null) {
                item {
                    Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                        IconButton(onClick = share::closePage, modifier = Modifier.size(48.dp)) {
                            Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.back))
                        }
                        CoverArt(share.resolveUrl(page.header.coverUrl), 48.dp, radius = 6.dp)
                        Spacer(Modifier.width(12.dp))
                        Column(Modifier.weight(1f)) {
                            Text(page.header.title, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            if (page.header.subtitle.isNotBlank()) {
                                Text(page.header.subtitle, style = MaterialTheme.typography.bodySmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            }
                        }
                    }
                }
                tracks(page.tracks, "page", explicitOk, onSent)
            } else {
                val results = ui.results
                if (results != null) {
                    if (results.tracks.isEmpty() && results.albums.isEmpty() && results.artists.isEmpty() && results.playlists.isEmpty()) {
                        item {
                            Text(
                                stringResource(R.string.sharing_search_empty),
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.padding(16.dp),
                            )
                        }
                    }
                    section(R.string.sharing_songs, results.tracks.isNotEmpty())
                    tracks(results.tracks, "t", explicitOk, onSent)
                    section(R.string.library_albums, results.albums.isNotEmpty())
                    collections(results.albums, "a")
                    section(R.string.library_artists, results.artists.isNotEmpty())
                    collections(results.artists, "r")
                    section(R.string.library_playlists, results.playlists.isNotEmpty())
                    collections(results.playlists, "p")
                }
            }
        }
    }
}

private fun LazyListScope.section(title: Int, visible: Boolean) {
    if (!visible) return
    item(key = "h$title") {
        Text(
            stringResource(title),
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.Bold,
            modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 16.dp, bottom = 4.dp),
        )
    }
}

private fun LazyListScope.tracks(list: List<ShareTrack>, prefix: String, explicitOk: Boolean, onSent: (String) -> Unit) {
    items(list.size, key = { i -> "$prefix$i-${list[i].source}" }) { i -> TrackRow(list[i], explicitOk || !list[i].explicit, onSent) }
}

private fun LazyListScope.collections(list: List<ShareCollection>, prefix: String) {
    items(list.size, key = { i -> "$prefix$i-${list[i].id}" }) { i -> CollectionRow(list[i]) }
}

@Composable
private fun TrackRow(track: ShareTrack, allowed: Boolean, onSent: (String) -> Unit) {
    val share = LocalContext.current.homefi.share
    ListItem(
        colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surface),
        leadingContent = { CoverArt(share.resolveUrl(track.coverUrl), 48.dp, radius = 6.dp) },
        headlineContent = { Text(track.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        supportingContent = {
            Text(
                listOfNotNull(
                    if (track.explicit) stringResource(R.string.sharing_explicit) else null,
                    track.artist.ifBlank { null },
                    track.album.ifBlank { null },
                ).joinToString(" · "),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        },
        trailingContent = {
            Row {
                IconButton(
                    onClick = {
                        share.add(track, next = true)
                        onSent(track.title)
                    },
                    enabled = allowed,
                    modifier = Modifier.size(48.dp),
                ) { Icon(Icons.Rounded.SkipNext, stringResource(R.string.queue_play_next)) }
                IconButton(
                    onClick = {
                        share.add(track, next = false)
                        onSent(track.title)
                    },
                    enabled = allowed,
                    modifier = Modifier.size(48.dp),
                ) { Icon(Icons.AutoMirrored.Rounded.PlaylistAdd, stringResource(R.string.sharing_add)) }
            }
        },
    )
}

@Composable
private fun CollectionRow(c: ShareCollection) {
    val share = LocalContext.current.homefi.share
    val icon = when (c.kind) {
        com.perseusoft.homefi.core.ShareBrowseKind.ARTIST -> Icons.Rounded.Person
        com.perseusoft.homefi.core.ShareBrowseKind.PLAYLIST -> Icons.AutoMirrored.Rounded.QueueMusic
        else -> Icons.Rounded.Album
    }
    ListItem(
        modifier = Modifier.clickable { share.browse(c.kind, c.id) },
        colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surface),
        leadingContent = { CoverArt(share.resolveUrl(c.coverUrl), 48.dp, radius = 6.dp, icon = icon) },
        headlineContent = { Text(c.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        supportingContent = c.subtitle.takeIf(String::isNotBlank)?.let { s -> { Text(s, maxLines = 1, overflow = TextOverflow.Ellipsis) } },
    )
}
