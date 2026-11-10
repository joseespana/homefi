package com.perseusoft.homefi.tv

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.DirectionsCar
import androidx.compose.material.icons.rounded.Groups
import androidx.compose.material.icons.rounded.HowToVote
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PersonRemove
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.PowerSettingsNew
import androidx.compose.material.icons.rounded.SkipNext
import androidx.compose.material.icons.rounded.Speaker
import androidx.compose.material.icons.rounded.Wifi
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.ShareMember
import com.perseusoft.homefi.core.ShareSkipMode
import com.perseusoft.homefi.core.ShareState
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.share.GuestStatus
import com.perseusoft.homefi.share.HubUi
import com.perseusoft.homefi.share.GuestUi
import com.perseusoft.homefi.share.net.HubAddress
import com.perseusoft.homefi.share.net.ShareNetworkMode
import com.perseusoft.homefi.ui.login.QrCode
import com.perseusoft.homefi.ui.player.coverModel
import com.perseusoft.homefi.ui.theme.LocalMotion

private val QR_SIZE = 168.dp
private val CARD_WIDTH = 262.dp
private val MODE_CARD_WIDTH = 300.dp
private const val FALLBACK_COLOR = "#0891B2"
private const val MAX_NOTICES = 2

/** The three things the Party section can be: starting one, hosting it, or being in someone else's. */
private enum class PartyView { START, HOST, GUEST }

/**
 * Party (docs/TV_UI.md 6.10): the TV is the hub with the big speakers, on the home Wi-Fi only (no radios on a TV).
 * Hosting shows the QR and the code big (a phone's HomeFi scans it, or the system camera opens HomeFi by the same
 * link), the guests, the songs waiting for the user's OK, now playing and the queue. Or the TV joins someone
 * else's party as a guest. All of it is the phone's `ShareManager`.
 */
@Composable
fun TvParty(entryKey: Int) {
    val share = LocalContext.current.homefi.share
    val hub by share.hub.collectAsStateWithLifecycle()
    val guest by share.guest.collectAsStateWithLifecycle()
    val view = when {
        hub.running || hub.starting -> PartyView.HOST
        guest.status == GuestStatus.CONNECTED -> PartyView.GUEST
        else -> PartyView.START
    }
    val motion = LocalMotion.current
    AnimatedContent(
        targetState = view,
        transitionSpec = { fadeIn(motion.tween(220, delay = 90)) togetherWith fadeOut(motion.tween(90)) },
        label = "tv-party",
    ) { shown ->
        when (shown) {
            PartyView.HOST -> TvPartyHost(hub, entryKey)
            PartyView.GUEST -> TvPartyGuest(guest, entryKey)
            PartyView.START -> TvPartyStart(hub, guest, entryKey)
        }
    }
}

// ------------------------------------------------------------------ start / join

@Composable
private fun TvPartyStart(hub: HubUi, guest: GuestUi, entryKey: Int) {
    val app = LocalContext.current.homefi
    val share = app.share
    val look = LocalTvLook.current
    val safe = tvSafePadding()
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val found by share.finder.hubs.collectAsStateWithLifecycle()
    val request by share.joinRequest.collectAsStateWithLifecycle()
    var party by rememberSaveable { mutableStateOf(settings.share.partyDefault) }
    var picking by remember { mutableStateOf<HubAddress?>(null) }
    val connecting = guest.status == GuestStatus.CONNECTING
    // the TV has no radios: only the home Wi-Fi is searched (no Bluetooth, no Wi-Fi Direct)
    val search = settings.share.discover && guest.status == GuestStatus.IDLE
    LifecycleResumeEffect(search) {
        if (search) share.finder.start(bluetooth = false)
        onPauseOrDispose { if (search) share.finder.stop() }
    }
    // a `homefi://join` link that reached the TV: the same join card
    LaunchedEffect(request) {
        request?.address?.let { picking = it }
        if (request != null) share.dismissJoin()
    }
    val lanOnly = hub.error ?: guest.error.takeIf { guest.status == GuestStatus.DISCONNECTED }

    Row(
        Modifier.fillMaxSize().padding(top = safe.calculateTopPadding(), end = safe.calculateRightPadding(androidx.compose.ui.unit.LayoutDirection.Ltr), bottom = safe.calculateBottomPadding(), start = 12.dp),
        horizontalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        Column(Modifier.width(MODE_CARD_WIDTH).fillMaxHeight(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(stringResource(R.string.tv_nav_party), style = TvType.title, color = look.ink, maxLines = 1)
            Text(stringResource(R.string.tv_party_start_body), style = TvType.secondary.copy(fontSize = 14.sp), color = look.ink2)
            TvTag(stringResource(R.string.tv_party_lan))
            ModeOption(!party, Icons.Rounded.DirectionsCar, R.string.sharing_mode_control, R.string.sharing_mode_control_hint, Modifier.tvInitialFocus(key = entryKey).tvLeftToDrawer()) { party = false }
            ModeOption(party, Icons.Rounded.Speaker, R.string.sharing_mode_party, R.string.sharing_mode_party_hint, Modifier.tvLeftToDrawer()) { party = true }
            lanOnly?.let { Text(it, style = TvType.secondary.copy(fontSize = 13.sp), color = look.error, maxLines = 3, overflow = TextOverflow.Ellipsis) }
            TvPill(
                stringResource(if (hub.starting) R.string.sharing_starting else R.string.sharing_start),
                {
                    if (!hub.starting) {
                        share.clearHubError()
                        share.startHub(party, ShareNetworkMode.LAN)
                    }
                },
                Modifier.fillMaxWidth().tvLeftToDrawer(),
                icon = Icons.Rounded.Groups,
                primary = true,
            )
        }
        TvGlass(Modifier.weight(1f).fillMaxHeight()) {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Text(stringResource(R.string.tv_party_join_title), style = TvType.section, color = look.ink)
                Text(stringResource(R.string.tv_party_join_body), style = TvType.secondary.copy(fontSize = 14.sp), color = look.ink2)
                if (connecting) {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        TvTyping(Modifier.padding(start = 0.dp))
                        Text(stringResource(R.string.sharing_joining), style = TvType.body, color = look.ink2)
                    }
                }
                if (guest.status == GuestStatus.DISCONNECTED && guest.error != null) {
                    TvPill(stringResource(R.string.sharing_ok), share::clearGuest, height = 30.dp)
                }
                if (found.isEmpty() && !connecting) {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp), modifier = Modifier.padding(top = 8.dp)) {
                        TvTyping()
                        Text(stringResource(R.string.tv_party_searching), style = TvType.secondary.copy(fontSize = 14.sp), color = look.ink3)
                    }
                }
                LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(top = 6.dp, bottom = 12.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    items(found, key = { it.mode.name + ":" + it.sessionName }) { address ->
                        TvRow(
                            title = address.sessionName,
                            subtitle = stringResource(R.string.tv_party_nearby),
                            coverIcon = Icons.Rounded.Wifi,
                            tag = stringResource(R.string.sharing_join),
                            onClick = { if (!connecting) picking = address },
                        )
                    }
                }
            }
        }
    }
    picking?.let { address ->
        JoinPrompt(address, onDone = { picking = null })
    }
}

/** A mode of hosting as a big option: the picked one is lit, never colour alone (it says "Selected"). */
@Composable
private fun ModeOption(selected: Boolean, icon: ImageVector, title: Int, hint: Int, modifier: Modifier, onClick: () -> Unit) {
    val look = LocalTvLook.current
    val shape = RoundedCornerShape(TvDimens.panelRadius)
    TvFocusable(onClick = onClick, modifier = modifier.fillMaxWidth(), shape = shape, focusedScale = 1.03f) { focused ->
        val fill = when {
            focused -> Modifier.background(Color.White, shape)
            selected -> Modifier.background(look.accent.copy(alpha = 0.2f), shape)
            else -> Modifier.background(Color.White.copy(alpha = 0.08f), shape)
        }
        Row(fill.padding(12.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Icon(icon, null, tint = if (focused) look.onFocus else look.accent2, modifier = Modifier.size(26.dp))
            Column(Modifier.weight(1f)) {
                Text(stringResource(title), style = TvType.body, color = if (focused) look.onFocus else look.ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(stringResource(hint), style = TvType.secondary, color = if (focused) ROW_SUB_FOCUSED else look.ink2, maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
            if (selected) Icon(Icons.Rounded.Check, stringResource(R.string.tv_party_selected), tint = if (focused) look.onFocus else look.accent2, modifier = Modifier.size(20.dp))
        }
    }
}

/** The code of a party found nearby (a link brings it with it), typed on the TV keyboard, then Join. */
@Composable
private fun JoinPrompt(address: HubAddress, onDone: () -> Unit) {
    val app = LocalContext.current.homefi
    val share = app.share
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val join = { code: String ->
        val name = share.displayName(settings.share)
        share.join(address, code, name, settings.share.color.ifBlank { FALLBACK_COLOR })
    }
    // a link already carries the code: no keyboard
    LaunchedEffect(address) {
        address.code?.let { join(it); onDone() }
    }
    if (address.code != null) return
    TvTextPrompt(
        title = address.sessionName,
        initial = "",
        placeholder = stringResource(R.string.sharing_code),
        doneLabel = stringResource(R.string.sharing_join),
        onDone = { code ->
            if (code.isNotBlank()) join(code.filter(Char::isLetterOrDigit))
            onDone()
        },
        onDismiss = onDone,
    )
}

// ------------------------------------------------------------------ hosting

@Composable
private fun TvPartyHost(hub: HubUi, entryKey: Int) {
    val app = LocalContext.current.homefi
    val share = app.share
    val look = LocalTvLook.current
    val safe = tvSafePadding()
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val player by app.player.state.collectAsStateWithLifecycle()
    val notices by share.notices.collectAsStateWithLifecycle()
    val autoApprove = settings.tv.partyAutoApprove
    var ending by remember { mutableStateOf(false) }
    var removing by remember { mutableStateOf<ShareMember?>(null) }
    // "let guests' songs in without asking": the waiting ones go in as they arrive
    LaunchedEffect(hub.pending, autoApprove) {
        if (autoApprove) hub.pending.forEach { share.approve(it.queueId) }
    }
    val info = hub.info
    val state = hub.state

    Row(
        Modifier.fillMaxSize().padding(top = safe.calculateTopPadding(), end = safe.calculateRightPadding(androidx.compose.ui.unit.LayoutDirection.Ltr), bottom = safe.calculateBottomPadding(), start = 12.dp),
        horizontalArrangement = Arrangement.spacedBy(18.dp),
    ) {
        Column(Modifier.weight(1f).fillMaxHeight()) {
            TvScreenHeader(stringResource(R.string.tv_party_title), info?.sessionName ?: stringResource(R.string.sharing_starting), titleStyle = TvType.title.copy(fontSize = 26.sp))
            Row(Modifier.padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TvPill(
                    stringResource(R.string.tv_party_stream),
                    { share.setParty(!hub.party) },
                    Modifier.tvInitialFocus(key = entryKey).tvLeftToDrawer(),
                    icon = Icons.Rounded.Speaker,
                    selected = hub.party,
                    height = 32.dp,
                    labelStyle = TvType.body.copy(fontSize = 14.sp),
                )
                TvPill(
                    stringResource(R.string.tv_party_auto_approve),
                    { app.settings.updateTv { it.copy(partyAutoApprove = !autoApprove) } },
                    icon = if (autoApprove) Icons.Rounded.Check else Icons.Rounded.HowToVote,
                    selected = autoApprove,
                    height = 32.dp,
                    labelStyle = TvType.body.copy(fontSize = 14.sp),
                )
                TvPill(stringResource(R.string.tv_party_end), { ending = true }, icon = Icons.Rounded.PowerSettingsNew, height = 32.dp, labelStyle = TvType.body.copy(fontSize = 14.sp))
            }
            Text(
                stringResource(if (hub.party) R.string.sharing_party_toggle_hint else R.string.sharing_mode_control_hint),
                style = TvType.secondary,
                color = look.ink3,
                modifier = Modifier.padding(top = 4.dp),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            LazyColumn(Modifier.weight(1f).fillMaxWidth(), contentPadding = PaddingValues(top = 6.dp, end = 6.dp, bottom = 28.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                // now playing, from the TV's own player (the hub plays here)
                player.current?.let { current ->
                    item(key = "now") {
                        SectionLabel(stringResource(R.string.sound_now_playing))
                        TvRow(
                            title = current.title,
                            subtitle = current.artist,
                            cover = current.coverModel(),
                            playing = player.isPlaying,
                            onClick = { app.player.togglePlayPause() },
                            modifier = Modifier.tvLeftToDrawer(),
                        )
                    }
                }
                if (hub.pending.isNotEmpty()) {
                    item(key = "pending-head") { SectionLabel(stringResource(R.string.sharing_pending, hub.pending.size)) }
                    items(hub.pending, key = { "pending-${it.queueId}" }) { item ->
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                            Box(Modifier.weight(1f)) {
                                TvRow(
                                    title = item.track.title,
                                    subtitle = stringResource(R.string.sharing_by, item.track.artist, item.addedBy.name),
                                    cover = item.track.coverUrl?.takeIf { it.startsWith("http") },
                                    onClick = { share.approve(item.queueId) },
                                    modifier = Modifier.tvLeftToDrawer(),
                                )
                            }
                            TvPill(stringResource(R.string.sharing_approve), { share.approve(item.queueId) }, icon = Icons.Rounded.Check, primary = true, height = 32.dp, labelStyle = TvType.body.copy(fontSize = 14.sp))
                            TvPill(stringResource(R.string.sharing_reject), { share.reject(item.queueId) }, icon = Icons.Rounded.Close, height = 32.dp, labelStyle = TvType.body.copy(fontSize = 14.sp))
                        }
                    }
                }
                item(key = "guests-head") { SectionLabel(stringResource(R.string.sharing_guests, hub.guests.size)) }
                if (hub.guests.isEmpty()) {
                    item(key = "guests-none") {
                        Text(stringResource(R.string.tv_party_empty), style = TvType.secondary.copy(fontSize = 14.sp), color = look.ink2, modifier = Modifier.padding(start = 4.dp, top = 2.dp))
                    }
                }
                items(hub.guests, key = { "guest-${it.guestId}" }) { guest ->
                    TvRow(
                        title = guest.name,
                        subtitle = guestActivity(guest, state),
                        coverIcon = Icons.Rounded.Groups,
                        tag = stringResource(R.string.tv_party_connected),
                        onClick = { removing = guest },
                        modifier = Modifier.tvLeftToDrawer(),
                    )
                }
                val upNext = state?.queue.orEmpty()
                if (upNext.isNotEmpty()) {
                    item(key = "queue-head") { SectionLabel(stringResource(R.string.sharing_up_next, upNext.size)) }
                    items(upNext, key = { "queue-${it.queueId}" }) { item ->
                        TvRow(
                            title = item.track.title,
                            subtitle = stringResource(R.string.sharing_by, item.track.artist, item.addedBy.name),
                            cover = item.track.coverUrl?.takeIf { it.startsWith("http") },
                            onClick = {},
                            modifier = Modifier.tvLeftToDrawer(),
                        )
                    }
                }
                if (notices.isNotEmpty()) {
                    item(key = "notices") {
                        Column(Modifier.padding(start = 4.dp, top = 6.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                            notices.takeLast(MAX_NOTICES).forEach { n -> Text(n.text, style = TvType.secondary, color = look.ink3, maxLines = 1, overflow = TextOverflow.Ellipsis) }
                        }
                    }
                }
            }
        }
        // the QR and the code, big: the way phones come in
        TvGlass(Modifier.width(CARD_WIDTH).fillMaxHeight()) {
            Column(Modifier.fillMaxSize().padding(16.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center) {
                if (hub.qr != null && info != null) {
                    Box(Modifier.clip(RoundedCornerShape(10.dp)).background(Color.White).padding(6.dp)) { QrCode(hub.qr, QR_SIZE) }
                    Text(stringResource(R.string.sharing_code), style = TvType.secondary, color = look.ink2, modifier = Modifier.padding(top = 10.dp))
                    Text(
                        info.code,
                        style = TvType.display.copy(fontSize = 34.sp, fontFamily = FontFamily.Monospace, letterSpacing = 4.sp),
                        color = look.ink,
                        maxLines = 1,
                    )
                    Text(stringResource(R.string.tv_party_how), style = TvType.secondary.copy(fontSize = 13.sp), color = look.ink2, textAlign = TextAlign.Center, modifier = Modifier.padding(top = 4.dp))
                    Text(stringResource(R.string.sharing_address, info.host, info.port.toInt()), style = TvType.secondary, color = look.ink3, modifier = Modifier.padding(top = 4.dp))
                } else {
                    TvTyping()
                    Text(stringResource(R.string.sharing_starting), style = TvType.body, color = look.ink2, modifier = Modifier.padding(top = 8.dp))
                }
            }
        }
    }
    if (ending) {
        TvConfirmDialog(
            title = stringResource(R.string.sharing_stop_title),
            text = stringResource(R.string.sharing_stop_text),
            confirmLabel = stringResource(R.string.sharing_stop),
            onConfirm = share::stopHub,
            onDismiss = { ending = false },
        )
    }
    removing?.let { guest ->
        TvActionMenu(
            guest.name,
            null,
            listOf(
                TvAction(Icons.Rounded.PersonRemove, stringResource(R.string.sharing_kick)) { share.kick(guest.guestId, ban = false) },
                TvAction(Icons.Rounded.Close, stringResource(R.string.sharing_block)) { share.kick(guest.guestId, ban = true) },
            ),
        ) { removing = null }
    }
}

/** What a guest last did, from the hub's queue ("Added Dreams"), or when it joined. */
@Composable
private fun guestActivity(guest: ShareMember, state: ShareState?): String {
    val added = state?.queue.orEmpty().lastOrNull { it.addedBy.guestId == guest.guestId }
    return if (added != null) stringResource(R.string.tv_party_added, added.track.title) else stringResource(R.string.tv_party_joined)
}

@Composable
private fun SectionLabel(text: String) {
    Text(text.uppercase(), style = TvType.caps, color = LocalTvLook.current.ink3, modifier = Modifier.padding(start = 4.dp, top = 10.dp, bottom = 4.dp))
}

// ------------------------------------------------------------------ as a guest

/** The TV in someone else's party: what plays, the queue, and the controls the hub allows; Leave. */
@Composable
private fun TvPartyGuest(guest: GuestUi, entryKey: Int) {
    val share = LocalContext.current.homefi.share
    val look = LocalTvLook.current
    val safe = tvSafePadding()
    val state = guest.state
    val now = state?.nowPlaying
    var leaving by remember { mutableStateOf(false) }
    val playback = state?.permissions?.playback == true
    Row(
        Modifier.fillMaxSize().padding(top = safe.calculateTopPadding(), end = safe.calculateRightPadding(androidx.compose.ui.unit.LayoutDirection.Ltr), bottom = safe.calculateBottomPadding(), start = 12.dp),
        horizontalArrangement = Arrangement.spacedBy(18.dp),
    ) {
        Column(Modifier.width(MODE_CARD_WIDTH).fillMaxHeight(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(guest.sessionName, style = TvType.title.copy(fontSize = 26.sp), color = look.ink, maxLines = 2, overflow = TextOverflow.Ellipsis)
            TvTag(stringResource(if (guest.state?.audio != null && guest.playAlong) R.string.sharing_guest_status_party else R.string.sharing_guest_status_remote), tone = look.accent2)
            if (now != null) {
                TvCoverCard(
                    cover = share.resolveUrl(now.track.coverUrl),
                    title = now.track.title,
                    subtitle = now.track.artist,
                    size = 118.dp,
                    onClick = {},
                )
            } else {
                Text(stringResource(R.string.sharing_nothing_playing), style = TvType.body, color = look.ink2)
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TvPill(
                    stringResource(if (state?.playing == true) R.string.player_pause else R.string.player_play),
                    { if (playback) share.toggle() },
                    Modifier.tvInitialFocus(key = entryKey).tvLeftToDrawer().alpha(if (playback) 1f else 0.5f),
                    icon = if (state?.playing == true) Icons.Rounded.Pause else Icons.Rounded.PlayArrow,
                    primary = true,
                    height = 32.dp,
                    labelStyle = TvType.body.copy(fontSize = 14.sp),
                )
                when (state?.permissions?.skip) {
                    ShareSkipMode.VOTE -> TvPill(
                        stringResource(R.string.sharing_vote_skip, state.skipVotes.toInt(), state.skipVotesNeeded.toInt()),
                        share::voteSkip,
                        icon = Icons.Rounded.HowToVote,
                        height = 32.dp,
                        labelStyle = TvType.body.copy(fontSize = 14.sp),
                    )
                    ShareSkipMode.EVERYONE -> TvPill(stringResource(R.string.player_next), share::skip, icon = Icons.Rounded.SkipNext, height = 32.dp, labelStyle = TvType.body.copy(fontSize = 14.sp))
                    else -> Unit
                }
            }
            TvPill(
                stringResource(if (guest.playAlong) R.string.tv_party_play_here_on else R.string.tv_party_play_here_off),
                { share.setPlayAlong(!guest.playAlong) },
                Modifier.tvLeftToDrawer(),
                icon = Icons.Rounded.Speaker,
                selected = guest.playAlong,
                height = 32.dp,
                labelStyle = TvType.body.copy(fontSize = 14.sp),
            )
            TvPill(stringResource(R.string.sharing_leave), { leaving = true }, Modifier.tvLeftToDrawer(), icon = Icons.Rounded.PowerSettingsNew, height = 32.dp, labelStyle = TvType.body.copy(fontSize = 14.sp))
        }
        TvGlass(Modifier.weight(1f).fillMaxHeight()) {
            Column(Modifier.padding(16.dp)) {
                val queue = state?.queue.orEmpty()
                Text(stringResource(R.string.sharing_up_next, queue.size), style = TvType.section, color = look.ink)
                if (queue.isEmpty()) Text(stringResource(R.string.sharing_queue_empty), style = TvType.secondary.copy(fontSize = 14.sp), color = look.ink2, modifier = Modifier.padding(top = 6.dp))
                LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(top = 8.dp, bottom = 12.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    itemsIndexed(queue, key = { _, it -> "q-${it.queueId}" }) { _, item ->
                        TvRow(
                            title = item.track.title,
                            subtitle = stringResource(R.string.sharing_by, item.track.artist, item.addedBy.name),
                            cover = share.resolveUrl(item.track.coverUrl),
                            onClick = {},
                        )
                    }
                }
            }
        }
    }
    if (leaving) {
        TvConfirmDialog(
            title = stringResource(R.string.sharing_leave_title, guest.sessionName),
            text = stringResource(R.string.sharing_leave_text),
            confirmLabel = stringResource(R.string.sharing_leave),
            onConfirm = share::leave,
            onDismiss = { leaving = false },
        )
    }
}

