package com.perseusoft.homefi.tv

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExperimentalAnimationApi
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.CloudSync
import androidx.compose.material.icons.rounded.CloudUpload
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.MoreHoriz
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material.icons.rounded.Shuffle
import androidx.compose.material.icons.rounded.SkipNext
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.HomePlaylist
import com.perseusoft.homefi.core.PlaylistOrigin
import com.perseusoft.homefi.core.PartyProgress
import com.perseusoft.homefi.core.SongSeed
import com.perseusoft.homefi.core.TasteScope
import com.perseusoft.homefi.data.TidalSave
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.library.EmbeddedCover
import com.perseusoft.homefi.ui.formatDuration
import com.perseusoft.homefi.ui.playlists.PartyMode
import com.perseusoft.homefi.ui.playlists.PartyUi
import com.perseusoft.homefi.ui.playlists.PartyViewModel
import com.perseusoft.homefi.ui.playlists.chatSummary
import com.perseusoft.homefi.ui.playlists.enqueuePlaylist
import com.perseusoft.homefi.ui.playlists.playPlaylist
import com.perseusoft.homefi.ui.theme.LocalMotion
import com.perseusoft.homefi.ui.tidal.Collection
import com.perseusoft.homefi.ui.tidal.TidalViewModel
import kotlinx.coroutines.launch

private const val SLIDE_FRACTION = 12
private val SIDE_PANE = 292.dp

private enum class PlaylistTab { HOMEFI, TIDAL }

/** Where the Playlists section is: its lists, one HomeFi playlist, or the AI chat that makes one. */
private sealed interface PlaylistsPage {
    data object Root : PlaylistsPage
    data class Detail(val id: String) : PlaylistsPage
    data class Chat(val replaceId: String?, val generation: Int) : PlaylistsPage
}

/**
 * Playlists (docs/TV_UI.md 6.11): the ones HomeFi made (with the card that starts the AI chat) and the user's
 * TIDAL playlists, a playlist page (play, shuffle, queue, save or update on TIDAL, Sound for it) and the
 * chat that makes a playlist from words, with quick replies, voice and the on-screen keyboard. It reads the
 * phone's state holders ([app.playlists][com.perseusoft.homefi.data.HomePlaylists], [PartyViewModel]).
 */
@OptIn(ExperimentalAnimationApi::class)
@Composable
fun TvPlaylists(entryKey: Int) {
    val app = LocalContext.current.homefi
    val tidal: TidalViewModel = viewModel()
    val homeLists by app.playlists.list.collectAsStateWithLifecycle()
    val account by app.account.account.collectAsStateWithLifecycle()
    val songSeed by app.songSeeds.request.collectAsStateWithLifecycle()
    var tab by rememberSaveable { mutableStateOf(PlaylistTab.HOMEFI) }
    var openId by rememberSaveable { mutableStateOf<String?>(null) }
    var chatOpen by rememberSaveable { mutableStateOf(false) }
    var chatReplace by rememberSaveable { mutableStateOf<String?>(null) }
    var generation by rememberSaveable { mutableIntStateOf(0) }
    var seed by remember { mutableStateOf<SongSeed?>(null) }
    var restoreKey by remember { mutableStateOf<Any?>(null) }
    var restoreNonce by remember { mutableIntStateOf(0) }
    LaunchedEffect(Unit) { app.playlists.refresh() }
    // "Playlist like this" from a song's menu: the chat opens with that song
    LaunchedEffect(songSeed) {
        songSeed?.let {
            seed = it
            chatReplace = null
            generation++
            chatOpen = true
            app.songSeeds.consume()
        }
    }
    val signedIn = account != null
    LaunchedEffect(signedIn) { if (!signedIn) tab = PlaylistTab.HOMEFI }

    val page: PlaylistsPage = when {
        chatOpen -> PlaylistsPage.Chat(chatReplace, generation)
        openId != null && homeLists.any { it.id == openId } -> PlaylistsPage.Detail(openId!!)
        else -> PlaylistsPage.Root
    }
    val close = {
        restoreKey = openId ?: NEW_CARD_KEY.takeIf { chatOpen }
        restoreNonce++
        openId = null
        chatOpen = false
        seed = null
    }
    BackHandler(enabled = page != PlaylistsPage.Root, onBack = close)

    val motion = LocalMotion.current
    AnimatedContent(
        targetState = page,
        contentKey = { it::class to ((it as? PlaylistsPage.Detail)?.id) },
        transitionSpec = {
            val deeper = targetState != PlaylistsPage.Root
            val slide = if (motion.moves) slideInHorizontally(motion.tween(260)) { if (deeper) it / SLIDE_FRACTION else -it / SLIDE_FRACTION } else EnterTransition.None
            (fadeIn(motion.tween(220, delay = 90)) + slide) togetherWith fadeOut(motion.tween(90))
        },
        label = "tv-playlists-page",
    ) { shown ->
        when (shown) {
            is PlaylistsPage.Detail -> homeLists.firstOrNull { it.id == shown.id }?.let { playlist ->
                TvHomePlaylistPage(
                    playlist = playlist,
                    signedIn = signedIn,
                    onDeleted = close,
                    onKeepChatting = { chatReplace = playlist.id; generation++; chatOpen = true },
                )
            }
            is PlaylistsPage.Chat -> TvPlaylistChat(
                replace = shown.replaceId?.let { id -> homeLists.firstOrNull { it.id == id } },
                seed = seed,
                generation = shown.generation,
                entryKey = entryKey,
                onRestart = { chatReplace = shown.replaceId; generation++; seed = null },
                onOpenSaved = { id -> chatOpen = false; openId = id },
            )
            PlaylistsPage.Root -> if (tab == PlaylistTab.TIDAL) {
                TvTidalPages(tidal, entryKey) { restore, nonce ->
                    PlaylistsRoot(tab, { tab = it }, signedIn, homeLists, tidal, entryKey, restore, nonce, onOpen = {}, onNew = {})
                }
            } else {
                PlaylistsRoot(
                    tab, { tab = it }, signedIn, homeLists, tidal, entryKey, restoreKey, restoreNonce,
                    onOpen = { openId = it.id; restoreKey = it.id },
                    onNew = { chatReplace = null; seed = null; chatOpen = true; restoreKey = NEW_CARD_KEY },
                )
            }
        }
    }
}

private const val NEW_CARD_KEY = "new-with-ai"

/** The list view: the tabs (HomeFi, TIDAL) and the covers under them. */
@Composable
private fun PlaylistsRoot(
    tab: PlaylistTab,
    onTab: (PlaylistTab) -> Unit,
    signedIn: Boolean,
    homeLists: List<HomePlaylist>,
    tidal: TidalViewModel,
    entryKey: Int,
    restoreKey: Any?,
    restoreNonce: Int,
    onOpen: (HomePlaylist) -> Unit,
    onNew: () -> Unit,
) {
    val safe = tvSafePadding()
    val bodyEntry = remember { FocusRequester() }
    val tabs = if (signedIn) PlaylistTab.entries else listOf(PlaylistTab.HOMEFI)
    val songsState = rememberLazyListState()
    val tidalGrid = rememberLazyGridState()
    val homeGrid = rememberLazyGridState()
    Column(Modifier.fillMaxSize().padding(top = safe.calculateTopPadding(), end = safe.calculateRightPadding(androidx.compose.ui.unit.LayoutDirection.Ltr))) {
        TvScreenHeader(
            stringResource(R.string.tv_nav_playlists),
            stringResource(R.string.tv_pl_sub, homeLists.size),
            Modifier.padding(start = 12.dp),
        ) { TvNowPlayingChip() }
        if (tabs.size > 1) {
            TvTabs(
                labels = tabs.map { stringResource(if (it == PlaylistTab.HOMEFI) R.string.tv_pl_tab_homefi else R.string.tv_pl_tab_tidal) },
                selected = tabs.indexOf(tab),
                onSelect = { onTab(tabs[it]) },
                modifier = Modifier.padding(start = 12.dp, top = 10.dp),
                entryKey = entryKey,
                downTo = bodyEntry,
            )
        }
        Box(Modifier.weight(1f)) {
            if (tab == PlaylistTab.TIDAL) {
                LaunchedEffect(Unit) { tidal.loadCollection(Collection.PLAYLISTS) }
                CollectionBody(tidal, Collection.PLAYLISTS, entryKey, songsState, tidalGrid, restoreKey, restoreNonce, bodyEntry)
            } else {
                HomefiGrid(homeLists, homeGrid, restoreKey, restoreNonce, bodyEntry, entryKey, tabs.size == 1, onOpen, onNew)
            }
        }
    }
}

private sealed interface HomeCard {
    data object New : HomeCard
    data class Made(val playlist: HomePlaylist) : HomeCard
}

@Composable
private fun HomefiGrid(
    lists: List<HomePlaylist>,
    state: androidx.compose.foundation.lazy.grid.LazyGridState,
    restoreKey: Any?,
    restoreNonce: Int,
    firstItem: FocusRequester,
    entryKey: Int,
    firstTakesFocus: Boolean,
    onOpen: (HomePlaylist) -> Unit,
    onNew: () -> Unit,
) {
    val app = LocalContext.current.homefi
    val library by app.library.state.collectAsStateWithLifecycle()
    var menu by remember { mutableStateOf<HomePlaylist?>(null) }
    var deleting by remember { mutableStateOf<HomePlaylist?>(null) }
    val cards = remember(lists) { listOf<HomeCard>(HomeCard.New) + lists.map(HomeCard::Made) }
    TvCoverGrid(
        cards,
        key = { if (it is HomeCard.Made) it.playlist.id else NEW_CARD_KEY },
        state = state,
        restoreKey = restoreKey,
        restoreNonce = restoreNonce,
        firstItem = firstItem,
    ) { card, size, mod ->
        when (card) {
            HomeCard.New -> TvCoverCard(
                cover = null,
                title = stringResource(R.string.tv_pl_new_title),
                subtitle = stringResource(R.string.tv_pl_new_sub),
                icon = Icons.Rounded.AutoAwesome,
                size = size,
                // with no tabs above, the first card is the screen's first focus
                modifier = mod.then(if (firstTakesFocus) Modifier.tvInitialFocus(key = entryKey) else Modifier),
                onClick = onNew,
            )
            is HomeCard.Made -> {
                val playlist = card.playlist
                TvCoverCard(
                    cover = remember(playlist, library.tracks) { playlistCover(playlist, app.library::find) },
                    title = playlist.name,
                    subtitle = playlistSubtitle(playlist),
                    icon = Icons.AutoMirrored.Rounded.QueueMusic,
                    size = size,
                    modifier = mod,
                    onClick = { onOpen(playlist) },
                    onLongClick = { menu = playlist },
                )
            }
        }
    }
    menu?.let { playlist ->
        TvActionMenu(playlist.name, playlistSubtitle(playlist), playlistActions(playlist, onDelete = { deleting = playlist })) { menu = null }
    }
    deleting?.let { playlist -> DeleteConfirm(playlist, onDeleted = {}) { deleting = null } }
}

@Composable
private fun playlistSubtitle(playlist: HomePlaylist): String =
    stringResource(R.string.pl_count, playlist.entries.size, formatDuration(playlist.entries.sumOf { it.durationMs.toLong() }))

/** The first entry's cover: the file's own artwork when it is in the library, else TIDAL's. */
private fun playlistCover(playlist: HomePlaylist, find: (String) -> com.perseusoft.homefi.core.LibraryTrack?): Any? {
    val first = playlist.entries.firstOrNull { it.localPath != null || it.coverUrl != null } ?: return null
    return first.localPath?.let(find)?.let { EmbeddedCover(it.path) } ?: first.coverUrl
}

/** Play, shuffle, queue, Sound and delete: what a long OK on a playlist offers (and the page's "more"). */
@Composable
private fun playlistActions(playlist: HomePlaylist, onDelete: () -> Unit, onKeepChatting: (() -> Unit)? = null): List<TvAction> {
    val app = LocalContext.current.homefi
    val nav = LocalTvNav.current
    val find = app.library::find
    return buildList {
        add(TvAction(Icons.Rounded.PlayArrow, stringResource(R.string.player_play)) { app.player.playPlaylist(playlist, find); nav.openPlayer() })
        add(TvAction(Icons.Rounded.Shuffle, stringResource(R.string.library_shuffle)) { app.player.playPlaylist(playlist, find, shuffle = true); nav.openPlayer() })
        add(TvAction(Icons.Rounded.SkipNext, stringResource(R.string.queue_play_next)) { app.player.enqueuePlaylist(playlist, find, next = true) })
        add(TvAction(Icons.AutoMirrored.Rounded.QueueMusic, stringResource(R.string.queue_add)) { app.player.enqueuePlaylist(playlist, find, next = false) })
        add(TvAction(Icons.Rounded.Tune, stringResource(R.string.sound_menu_item)) {
            app.sound.openMenu(TasteScope.Playlist(playlist.id, playlist.name))
            nav.go(TvSection.SOUND)
        })
        if (onKeepChatting != null && playlist.origin is PlaylistOrigin.Chat) {
            add(TvAction(Icons.Rounded.AutoAwesome, stringResource(R.string.pl_chat_continue), onKeepChatting))
        }
        add(TvAction(Icons.Rounded.Delete, stringResource(R.string.pl_delete), onDelete))
    }
}

/** The delete question: it names what stays safe, and what goes with it when the TIDAL copy is removed too. */
@Composable
private fun DeleteConfirm(playlist: HomePlaylist, onDeleted: () -> Unit, onDismiss: () -> Unit) {
    val app = LocalContext.current.homefi
    val words = if (app.playlists.deletesTidalCopy(playlist)) R.string.pl_delete_confirm_tidal else R.string.pl_delete_confirm
    TvConfirmDialog(
        title = stringResource(R.string.pl_delete),
        text = stringResource(words, playlist.name),
        confirmLabel = stringResource(R.string.pl_delete),
        onDismiss = onDismiss,
        onConfirm = {
            // in the app scope: the delete finishes even if the screen is left
            app.appScope.launch {
                if (app.playlists.delete(playlist.id) == null) app.playlists.deleteTidalCopy(playlist)
            }
            onDeleted()
        },
    )
}

// ------------------------------------------------------------------ a HomeFi playlist

/**
 * A playlist HomeFi made: Play, Shuffle, "TIDAL" (save, or update the copy) and more on the left, its songs
 * on the right with the state of the last save to TIDAL. OK on a song plays the list from there.
 */
@Composable
private fun TvHomePlaylistPage(playlist: HomePlaylist, signedIn: Boolean, onDeleted: () -> Unit, onKeepChatting: () -> Unit) {
    val app = LocalContext.current.homefi
    val nav = LocalTvNav.current
    val library by app.library.state.collectAsStateWithLifecycle()
    val player by app.player.state.collectAsStateWithLifecycle()
    val tidalAll by app.playlists.tidal.collectAsStateWithLifecycle()
    val tidal = tidalAll?.takeIf { it.id == playlist.id }
    val find = app.library::find
    val playable = remember(playlist, library.tracks) { playlist.entries.any { it.tidalId != null || it.localPath?.let(find) != null } }
    val onTidal = playlist.tidalUuid != null
    val first = remember { FocusRequester() }
    val rowsEntry = remember { FocusRequester() }
    var more by remember { mutableStateOf(false) }
    var deleting by remember { mutableStateOf(false) }
    TvDetailPage(
        title = playlist.name,
        subtitle = playlist.description.ifBlank { playlistSubtitle(playlist) },
        cover = remember(playlist, library.tracks) { playlistCover(playlist, find) },
        coverIcon = Icons.AutoMirrored.Rounded.QueueMusic,
        tags = listOfNotNull(
            if (onTidal) stringResource(R.string.tv_pl_on_tidal) else null,
            playlist.unmatched.size.takeIf { it > 0 }?.let { stringResource(R.string.tv_pl_not_found, it) },
        ),
        pills = {
            val edge = Modifier.tvLeftToDrawer().tvRightTo(rowsEntry).width(DETAIL_PILL_WIDTH)
            TvPill(stringResource(R.string.library_play_all), { app.player.playPlaylist(playlist, find); nav.openPlayer() }, Modifier.tvInitialFocus(first, afterMs = PAGE_FOCUS_DELAY_MS).then(edge).alphaWhen(playable), icon = Icons.Rounded.PlayArrow, primary = true)
            TvPill(stringResource(R.string.library_shuffle), { app.player.playPlaylist(playlist, find, shuffle = true); nav.openPlayer() }, edge.alphaWhen(playable), icon = Icons.Rounded.Shuffle)
            if (signedIn) {
                val saving = tidal is TidalSave.Saving
                TvPill(
                    stringResource(
                        when {
                            saving -> R.string.pl_tidal_saving
                            onTidal -> R.string.pl_tidal_update
                            else -> R.string.pl_tidal_save
                        },
                    ),
                    { if (!saving) app.appScope.launch { app.playlists.saveToTidal(playlist.id) } },
                    edge,
                    icon = if (onTidal) Icons.Rounded.CloudSync else Icons.Rounded.CloudUpload,
                )
            }
            TvPill(stringResource(R.string.tv_pl_more), { more = true }, edge, icon = Icons.Rounded.MoreHoriz)
        },
    ) {
        LazyColumn(
            Modifier.fillMaxSize(),
            contentPadding = PaddingValues(start = 6.dp, end = 24.dp, top = 12.dp, bottom = 28.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            item(key = "tidal-state") { TidalState(tidal, onTidal, signedIn) }
            itemsIndexed(playlist.entries, key = { i, e -> "e$i-" + (e.tidalId ?: e.localPath ?: e.title) }) { index, entry ->
                val local = entry.localPath?.let(find)
                val canPlay = local != null || entry.tidalId != null
                val current = player.current?.mediaId?.let { it == local?.path || it == entry.tidalId?.let { id -> "tidal:$id" } } == true
                TvRow(
                    title = entry.title,
                    subtitle = entry.artist,
                    cover = local?.let { EmbeddedCover(it.path) } ?: entry.coverUrl,
                    playing = current,
                    dimmed = !canPlay,
                    tag = if (local != null) stringResource(R.string.pl_local) else null,
                    trailing = if (entry.durationMs > 0uL) formatDuration(entry.durationMs.toLong()) else null,
                    onClick = {
                        when {
                            current -> app.player.togglePlayPause()
                            canPlay -> { app.player.playPlaylist(playlist, find, entryIndex = index); nav.openPlayer() }
                        }
                    },
                    modifier = (if (index == 0) Modifier.focusRequester(rowsEntry) else Modifier).tvLeftTo(first),
                )
            }
        }
    }
    if (more) {
        TvActionMenu(
            playlist.name,
            playlistSubtitle(playlist),
            playlistActions(playlist, onDelete = { deleting = true }, onKeepChatting = onKeepChatting),
        ) { more = false }
    }
    if (deleting) DeleteConfirm(playlist, onDeleted = onDeleted) { deleting = false }
}

private fun Modifier.alphaWhen(enabled: Boolean): Modifier = if (enabled) this else this.then(Modifier.alpha(DISABLED_ALPHA))

private const val DISABLED_ALPHA = 0.5f

/** Where this playlist lives (HomeFi only, or also in TIDAL) and how the last save went, in words. */
@Composable
private fun TidalState(tidal: TidalSave?, onTidal: Boolean, signedIn: Boolean) {
    val look = LocalTvLook.current
    val text: Pair<String, androidx.compose.ui.graphics.Color> = when (tidal) {
        is TidalSave.Saving -> stringResource(R.string.pl_tidal_saving) to look.ink2
        is TidalSave.Saved -> tidal.summary to look.ok
        is TidalSave.Failed -> stringResource(R.string.pl_tidal_failed, tidal.message) to look.error
        null -> stringResource(if (onTidal) R.string.pl_in_homefi_and_tidal else R.string.pl_only_in_homefi) to look.ink2
    }
    if (!signedIn && tidal == null) return
    Row(Modifier.padding(start = 4.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        if (tidal is TidalSave.Saved) androidx.compose.material3.Icon(Icons.Rounded.Check, null, tint = look.ok, modifier = Modifier.width(18.dp))
        Text(text.first, style = TvType.secondary.copy(fontSize = 13.sp), color = text.second, maxLines = 2, overflow = TextOverflow.Ellipsis)
    }
}

// ------------------------------------------------------------------ the AI chat

/**
 * "Make a playlist with AI": the conversation on the left (the assistant's quick replies, the remote's mic, the
 * on-screen keyboard), what it understood and the result on the right. A built list can be played and saved
 * (in HomeFi only; "TIDAL" is then one pill on its page). Without an AI it only says how to set one up.
 */
@Composable
private fun TvPlaylistChat(
    replace: HomePlaylist?,
    seed: SongSeed?,
    generation: Int,
    entryKey: Int,
    onRestart: () -> Unit,
    onOpenSaved: (String) -> Unit,
) {
    val app = LocalContext.current.homefi
    val nav = LocalTvNav.current
    val look = LocalTvLook.current
    val safe = tvSafePadding()
    val vm: PartyViewModel = viewModel(key = "tv-chat-${replace?.id}-$generation")
    val ui by vm.ui.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { vm.open(replace, PartyMode.CHAT, seed) }
    // a build in progress is cancelled by BACK first
    BackHandler(enabled = ui is PartyUi.Building) { vm.cancel() }

    val state = ui
    val form = when (state) {
        is PartyUi.Asking -> state.form
        is PartyUi.Building -> state.form
        is PartyUi.Built -> state.form
        PartyUi.NeedsAi -> null
    }
    Row(
        Modifier.fillMaxSize().padding(top = safe.calculateTopPadding(), bottom = safe.calculateBottomPadding(), end = safe.calculateRightPadding(androidx.compose.ui.unit.LayoutDirection.Ltr)),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Column(Modifier.weight(1f).fillMaxHeight()) {
            TvScreenHeader(stringResource(R.string.pl_chat_title), stringResource(R.string.pl_chat_intro), Modifier.padding(start = 12.dp))
            if (form == null) {
                Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
                    TvEmptyState(
                        title = stringResource(R.string.pl_chat_title),
                        body = stringResource(R.string.pl_chat_needs_ai),
                        cta = stringResource(R.string.pl_party_set_up_ai),
                        ctaIcon = Icons.Rounded.AutoAwesome,
                        entryKey = entryKey,
                        onCta = { nav.openSettings(TvSettingsCategory.AI) },
                        icon = Icons.Rounded.AutoAwesome,
                    )
                }
            } else {
                val busy = state is PartyUi.Building
                TvChat(
                    lines = form.chat.lines.map { TvChatLine(it.id, it.fromUser, it.text) },
                    typing = form.chat.thinking,
                    replies = form.chat.replies,
                    canSend = !busy && !form.chat.closed && !form.chat.thinking,
                    onSend = vm::send,
                    entryKey = entryKey,
                    promptTitle = stringResource(R.string.pl_chat_title),
                    placeholder = stringResource(R.string.pl_chat_hint),
                    modifier = Modifier.weight(1f),
                    closedNote = if (form.chat.closed) stringResource(R.string.pl_party_chat_closing) else null,
                    actions = { TvPill(stringResource(R.string.assistant_restart), onRestart, icon = Icons.Rounded.Refresh, height = 34.dp) },
                )
            }
        }
        if (form != null) {
            TvGlass(Modifier.width(SIDE_PANE).fillMaxHeight().padding(bottom = 4.dp)) {
                Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    when (state) {
                        is PartyUi.Building -> BuildingPane(state, onCancel = vm::cancel)
                        is PartyUi.Built -> BuiltPane(state, vm, onOpenSaved)
                        else -> BriefPane(form, vm::build)
                    }
                    form.error?.let { Text(it, style = TvType.secondary.copy(fontSize = 13.sp), color = look.error, maxLines = 3, overflow = TextOverflow.Ellipsis) }
                }
            }
        }
    }
}

/** What the AI understood so far, and the Build pill (it waits for a first message). */
@Composable
private fun BriefPane(form: com.perseusoft.homefi.ui.playlists.PartyForm, onBuild: () -> Unit) {
    val look = LocalTvLook.current
    val ready = !form.chat.thinking && form.chat.userTurns > 0
    Text(stringResource(R.string.tv_pl_understood), style = TvType.caps, color = look.ink3)
    Text(
        form.chatBrief?.let { chatSummary(it) } ?: stringResource(R.string.pl_chat_summary_unset),
        style = TvType.body.copy(fontSize = 15.sp),
        color = look.ink,
        maxLines = 6,
        overflow = TextOverflow.Ellipsis,
    )
    TvPill(
        stringResource(R.string.pl_party_build),
        { if (ready) onBuild() },
        Modifier.fillMaxWidth().alphaWhen(ready),
        icon = Icons.Rounded.AutoAwesome,
        primary = true,
    )
    if (!ready) Text(stringResource(R.string.tv_pl_build_wait), style = TvType.secondary.copy(fontSize = 13.sp), color = look.ink3)
}

@Composable
private fun BuildingPane(state: PartyUi.Building, onCancel: () -> Unit) {
    val look = LocalTvLook.current
    val progress = state.progress
    val line = when {
        progress is PartyProgress.Resolving -> stringResource(R.string.pl_progress_resolving, progress.done.toInt(), progress.total.toInt())
        progress is PartyProgress.Asking && progress.round > 0u -> stringResource(R.string.pl_party_progress_more, state.aiName)
        else -> stringResource(R.string.pl_party_progress_asking, state.aiName)
    }
    Text(line, style = TvType.body.copy(fontSize = 15.sp), color = look.ink)
    val fraction = (progress as? PartyProgress.Resolving)?.takeIf { it.total > 0u }?.let { it.done.toFloat() / it.total.toFloat() }
    if (fraction != null) TvProgress(fraction) else TvTyping()
    TvPill(stringResource(R.string.pl_cancel), onCancel, Modifier.tvInitialFocus(), height = 34.dp)
}

/** The built list: Play, Save (HomeFi only), Build again, Discard; the songs under them, OK plays from one. */
@Composable
private fun androidx.compose.foundation.layout.ColumnScope.BuiltPane(state: PartyUi.Built, vm: PartyViewModel, onOpenSaved: (String) -> Unit) {
    val app = LocalContext.current.homefi
    val nav = LocalTvNav.current
    val look = LocalTvLook.current
    val library by app.library.state.collectAsStateWithLifecycle()
    val playlist = state.built.playlist
    val find = app.library::find
    val playable = remember(playlist, library.tracks) { playlist.entries.any { it.tidalId != null || it.localPath?.let(find) != null } }
    Text(playlist.name, style = TvType.section, color = look.ink, maxLines = 2, overflow = TextOverflow.Ellipsis)
    Text(stringResource(R.string.pl_count, playlist.entries.size, formatDuration(playlist.entries.sumOf { it.durationMs.toLong() })), style = TvType.secondary, color = look.ink2)
    state.built.note?.let { Text(it, style = TvType.secondary.copy(fontSize = 13.sp), color = look.ink2, maxLines = 3, overflow = TextOverflow.Ellipsis) }
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        TvPill(stringResource(R.string.player_play), { app.player.playPlaylist(playlist, find); nav.openPlayer() }, Modifier.tvInitialFocus(key = playlist.id).alphaWhen(playable), icon = Icons.Rounded.PlayArrow, primary = true, height = 32.dp, labelStyle = TvType.body.copy(fontSize = 14.sp))
        if (state.saved) {
            TvPill(stringResource(R.string.tv_pl_open), { onOpenSaved(playlist.id) }, icon = Icons.AutoMirrored.Rounded.QueueMusic, height = 32.dp, labelStyle = TvType.body.copy(fontSize = 14.sp))
        } else {
            TvPill(stringResource(R.string.pl_save), vm::save, icon = Icons.Rounded.Check, height = 32.dp, labelStyle = TvType.body.copy(fontSize = 14.sp))
        }
    }
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        TvPill(stringResource(R.string.pl_party_build_again), vm::build, icon = Icons.Rounded.Refresh, height = 32.dp, labelStyle = TvType.body.copy(fontSize = 14.sp))
        if (!state.saved) TvPill(stringResource(R.string.pl_discard), vm::discard, height = 32.dp, labelStyle = TvType.body.copy(fontSize = 14.sp))
    }
    if (state.saved) TvTag(stringResource(R.string.pl_saved), tone = look.ok)
    LazyColumn(Modifier.fillMaxWidth().weight(1f), contentPadding = PaddingValues(vertical = 6.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
        itemsIndexed(playlist.entries, key = { i, e -> "b$i-" + (e.tidalId ?: e.localPath ?: e.title) }) { index, entry ->
            val local = entry.localPath?.let(find)
            val canPlay = local != null || entry.tidalId != null
            TvRow(
                title = entry.title,
                subtitle = entry.artist,
                cover = local?.let { EmbeddedCover(it.path) } ?: entry.coverUrl,
                dimmed = !canPlay,
                onClick = { if (canPlay) { app.player.playPlaylist(playlist, find, entryIndex = index); nav.openPlayer() } },
            )
        }
    }
}
