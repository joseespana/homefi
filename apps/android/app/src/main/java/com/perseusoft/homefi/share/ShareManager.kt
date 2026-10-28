package com.perseusoft.homefi.share

import android.net.Uri
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.widget.Toast
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.Player
import com.perseusoft.homefi.HomefiApplication
import com.perseusoft.homefi.R
import com.perseusoft.homefi.player.AudioTap
import com.perseusoft.homefi.core.GuestEvent
import com.perseusoft.homefi.core.GuestListener
import com.perseusoft.homefi.core.HubCommand
import com.perseusoft.homefi.core.HubEvent
import com.perseusoft.homefi.core.HubInfo
import com.perseusoft.homefi.core.HubSettings
import com.perseusoft.homefi.core.ShareBrowseKind
import com.perseusoft.homefi.core.ShareGuest
import com.perseusoft.homefi.core.ShareHub
import com.perseusoft.homefi.core.ShareMember
import com.perseusoft.homefi.core.SharePage
import com.perseusoft.homefi.core.ShareQueueEntry
import com.perseusoft.homefi.core.ShareQueueItem
import com.perseusoft.homefi.core.ShareSearchResults
import com.perseusoft.homefi.core.ShareState
import com.perseusoft.homefi.core.ShareTrack
import com.perseusoft.homefi.core.joinHub
import com.perseusoft.homefi.core.parseJoinUri
import com.perseusoft.homefi.core.startHub as startShareHub
import com.perseusoft.homefi.data.ShareSettings
import com.perseusoft.homefi.data.userMessage
import com.perseusoft.homefi.player.isFilePath
import com.perseusoft.homefi.player.queueId
import com.perseusoft.homefi.player.tidalId
import com.perseusoft.homefi.player.withShareInfo
import com.perseusoft.homefi.share.net.HostNetwork
import com.perseusoft.homefi.share.net.HubAddress
import com.perseusoft.homefi.share.net.ShareFinder
import com.perseusoft.homefi.share.net.ShareHost
import com.perseusoft.homefi.share.net.ShareNetworkMode
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicLong
import java.util.concurrent.atomic.AtomicReference

/** This phone as the hub (shared control / party). */
data class HubUi(
    val starting: Boolean = false,
    val running: Boolean = false,
    /** Streaming the audio to guests that play along. */
    val party: Boolean = false,
    /** Sound on this phone too; off, the hub is only the DJ and the guests' speakers play. */
    val playHere: Boolean = true,
    val info: HubInfo? = null,
    /** QR payload: the join link plus the Wi-Fi Direct group, when there is one. */
    val qr: String? = null,
    val network: HostNetwork? = null,
    val guests: List<ShareMember> = emptyList(),
    val pending: List<ShareQueueItem> = emptyList(),
    val state: ShareState? = null,
    val error: String? = null,
)

enum class GuestStatus { IDLE, CONNECTING, CONNECTED, DISCONNECTED }

/** This phone as a guest of someone else's hub. */
data class GuestUi(
    val status: GuestStatus = GuestStatus.IDLE,
    val sessionName: String = "",
    val guestId: String = "",
    val state: ShareState? = null,
    val results: ShareSearchResults? = null,
    val page: SharePage? = null,
    val searching: Boolean = false,
    /** Why joining failed or the hub closed the connection. */
    val error: String? = null,
    /** "Play on this phone" (party mode). */
    val playAlong: Boolean = true,
    val playingHere: Boolean = false,
    val roundTripMs: Long? = null,
)

/** A discreet line on the hub ("Ana added …"); disappears by itself. */
data class ShareNotice(val id: Long, val text: String, val color: String?)

/** Asks for the join dialog: a found / scanned hub, or null for the manual form. */
data class JoinRequest(val address: HubAddress?)

/**
 * Shared control and party mode on Android: runs the hub (mirrors the player
 * into it, applies the queue it decides) or joins one as a guest (remote
 * control, and plays the hub's stream in sync on this phone).
 */
class ShareManager(private val app: HomefiApplication) {
    private val scope = app.appScope
    private val main = Handler(Looper.getMainLooper())
    private val host = ShareHost(app)

    /** Hubs nearby, for the Home banner. */
    val finder = ShareFinder(app)

    private val _hub = MutableStateFlow(HubUi())
    val hub: StateFlow<HubUi> = _hub.asStateFlow()

    private val _guest = MutableStateFlow(GuestUi())
    val guest: StateFlow<GuestUi> = _guest.asStateFlow()

    private val _notices = MutableStateFlow<List<ShareNotice>>(emptyList())
    val notices: StateFlow<List<ShareNotice>> = _notices.asStateFlow()

    /** Notices and refusals from the hub, for the guest's snackbar. */
    private val _guestMessages = MutableSharedFlow<String>(extraBufferCapacity = 16)
    val guestMessages: SharedFlow<String> = _guestMessages.asSharedFlow()

    private val _joinRequest = MutableStateFlow<JoinRequest?>(null)
    val joinRequest: StateFlow<JoinRequest?> = _joinRequest.asStateFlow()

    private val _guestOpen = MutableStateFlow(false)
    val guestOpen: StateFlow<Boolean> = _guestOpen.asStateFlow()

    @Volatile private var shareHub: ShareHub? = null
    @Volatile private var shareGuest: ShareGuest? = null
    @Volatile private var currentPath: String? = null

    /** Queue commands posted to the main thread and not applied yet. */
    private val pendingQueue = AtomicInteger(0)
    private val latestQueue = AtomicReference<List<ShareQueueItem>?>(null)
    private val noticeIds = AtomicLong(0)
    private val guestSession = AtomicLong(0)

    /** The player's volume while the hub plays only on the guests' speakers (null: not muted). */
    private var mutedVolume: Float? = null

    private var hubJobs = emptyList<Job>()
    private var partyJob: Job? = null
    private val party = PartyPlayer(app)

    private val streamDir get() = File(app.cacheDir, "stream").also { it.mkdirs() }.absolutePath
    private val share: ShareSettings get() = app.settings.current.share

    val isHub: Boolean get() = shareHub != null
    val isGuest: Boolean get() = shareGuest != null

    fun displayName(s: ShareSettings = share): String = s.name.trim().ifBlank { Build.MODEL }

    fun sessionName(s: ShareSettings = share): String =
        s.sessionName.trim().ifBlank { app.getString(R.string.sharing_session_default, displayName(s)) }

    // ------------------------------------------------------------------ hub

    fun startHub(party: Boolean, mode: ShareNetworkMode) {
        val now = _hub.value
        if (now.running || now.starting || isGuest) return
        _hub.value = HubUi(starting = true, party = party)
        scope.launch {
            var network: HostNetwork? = null
            var started: ShareHub? = null
            try {
                val s = share
                // the same Wi-Fi Direct group every time: a player without a camera remembers it
                val groupName = s.groupName.takeIf(ShareHost::validGroupName) ?: ShareHost.newGroupName()
                val groupPass = s.groupPass.takeIf { it.length in 8..63 } ?: ShareHost.newGroupPass()
                if (groupName != s.groupName || groupPass != s.groupPass) {
                    app.settings.updateShare { it.copy(groupName = groupName, groupPass = groupPass) }
                }
                network = host.start(mode, groupName, groupPass)
                started = startShareHub(
                    HubSettings(
                        sessionName = sessionName(s),
                        hostName = displayName(s),
                        hostColor = s.color,
                        code = if (s.fixedCode) s.code.filter(Char::isDigit) else "",
                        port = s.port.coerceIn(0, 65535).toUShort(),
                        advertiseHost = network.host,
                        permissions = s.permissions(),
                        streamAudio = party,
                    ),
                    HubHost(app, this@ShareManager),
                )
                val info = started.info()
                host.announce(info.sessionName, info.port.toInt(), alsoLan = s.announceLan, bluetooth = s.bluetooth)
                shareHub = started
                _hub.value = HubUi(running = true, party = party, info = info, qr = qrPayload(info, network), network = network, state = started.state())
                withContext(Dispatchers.Main) {
                    attach(started)
                    ShareService.onStopRequested = { stopHub() }
                    ShareService.start(app, app.getString(R.string.sharing_service_hub, info.sessionName), hubServiceText())
                }
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                shareHub = null
                started?.let { runCatching { it.stop() }; it.destroy() }
                if (network != null) runCatching { host.stop() }
                _hub.value = HubUi(party = party, error = e.userMessage())
            }
        }
    }

    fun stopHub() {
        val hub = shareHub
        shareHub = null
        cancelHold()
        nextPushed = null
        setHubPlayHere(true)
        _hub.value = HubUi()
        _notices.value = emptyList()
        onMain { detach() }
        ShareService.onStopRequested = null
        ShareService.stop(app)
        if (hub != null) {
            scope.launch {
                runCatching { hub.stop() }
                hub.destroy()
                runCatching { host.stop() }
            }
        }
    }

    fun clearHubError() = _hub.update { it.copy(error = null) }

    /**
     * Party mode: play on this phone too, or only on the guests' speakers. The player
     * keeps running muted (the guests follow its clock); guests still see the real volume.
     */
    fun setHubPlayHere(on: Boolean) = onMain {
        val player = app.player
        if (on) {
            mutedVolume?.let { player.setVolume(it) }
            mutedVolume = null
        } else if (mutedVolume == null && shareHub != null) {
            mutedVolume = player.player?.volume ?: 1f
            player.setVolume(0f)
        }
        _hub.update { if (it.running || it.starting) it.copy(playHere = on) else it }
    }

    /** Party mode on / off while sharing. */
    fun setParty(on: Boolean) {
        val hub = shareHub ?: return
        hub.setStreaming(on)
        if (!on) setHubPlayHere(true) // nobody else plays: back on this phone
        _hub.update { it.copy(party = on) }
        app.player.player?.currentMediaItem?.let { pushAudio(it) }
        refreshHub()
    }

    fun kick(guestId: String, ban: Boolean) {
        shareHub?.kick(guestId, ban)
        refreshHub()
    }

    fun approve(queueId: ULong) {
        shareHub?.approve(queueId)
        refreshHub()
    }

    fun reject(queueId: ULong) {
        shareHub?.reject(queueId)
        refreshHub()
    }

    fun dismissNotice(id: Long) = _notices.update { list -> list.filterNot { it.id == id } }

    internal fun isPlaying(path: String): Boolean = currentPath == path

    /** Main thread: wire the player to the hub. */
    private fun attach(hub: ShareHub) {
        val player = app.player
        // the hub decides the order of what comes next
        player.player?.let { if (it.shuffleModeEnabled) it.shuffleModeEnabled = false }
        player.addListener(mirror)
        player.playOverride = ::hostPlay
        player.onQueueEdited = { reconcile() }
        player.player?.let { p ->
            hub.setVolume(p.volume)
            pushNowPlaying(p)
        }
        reconcile()
        hubJobs = listOf(
            // the music's levels, so guests without the audio still see the meters move
            scope.launch(Dispatchers.Main) {
                var watching = false
                try {
                    while (isActive) {
                        val want = share.sendLevels && _hub.value.guests.isNotEmpty() && app.player.state.value.isPlaying
                        if (want != watching) {
                            watching = want
                            if (want) AudioTap.watchers++ else AudioTap.watchers--
                        }
                        if (!want) {
                            delay(500)
                            continue
                        }
                        val l = AudioTap.levels.value
                        shareHub?.sendLevels(byte(l.left).toUByte(), byte(l.right).toUByte(), ByteArray(l.bands.size) { byte(l.bands[it]).toByte() })
                        delay(LEVELS_EVERY_MS)
                    }
                } finally {
                    if (watching) AudioTap.watchers--
                }
            },
            // positions while playing, so guests that play along correct their drift
            scope.launch(Dispatchers.Main) {
                while (isActive) {
                    delay(POSITION_EVERY_MS)
                    val p = app.player.player ?: continue
                    if (p.isPlaying) shareHub?.setPosition(reportedPosition(p), true)
                    refreshHub()
                }
            },
            // the permissions in Settings apply right away
            scope.launch {
                app.settings.settings.map { it.share.permissions() }.distinctUntilChanged().drop(1).collect {
                    shareHub?.setPermissions(it)
                    refreshHub()
                }
            },
        )
    }

    private fun detach() {
        hubJobs.forEach { it.cancel() }
        hubJobs = emptyList()
        app.player.removeListener(mirror)
        app.player.playOverride = null
        app.player.onQueueEdited = null
        currentPath = null
        pendingQueue.set(0)
        latestQueue.set(null)
    }

    private val mirror = object : Player.Listener {
        override fun onEvents(player: Player, events: Player.Events) {
            val hub = shareHub ?: return
            if (events.contains(Player.EVENT_MEDIA_ITEM_TRANSITION)) {
                cancelHold()
                pushNowPlaying(player)
                holdForGuests(player)
            } else if (events.containsAny(Player.EVENT_IS_PLAYING_CHANGED, Player.EVENT_POSITION_DISCONTINUITY)) {
                // pressing play during the wait starts right away
                if (holdJob != null && player.isPlaying) cancelHold()
                if (holdJob == null) hub.setPosition(reportedPosition(player), player.isPlaying)
            }
            if (events.contains(Player.EVENT_VOLUME_CHANGED)) hub.setVolume(mutedVolume ?: player.volume)
            refreshHub()
        }
    }

    private var holdJob: Job? = null

    /**
     * Party mode, a new song: hold it at 0 until every guest has it loaded (or the wait in
     * Settings runs out), then everyone starts at the same hub-clock instant. With the songs
     * downloaded ahead this is usually well under a second.
     */
    private fun holdForGuests(player: Player) {
        val hub = shareHub ?: return
        val waitMs = share.partyStartWaitSec.coerceIn(0, 10) * 1000L
        val queueId = hub.state().nowPlaying?.queueId ?: return
        if (!_hub.value.party || waitMs == 0L || !player.playWhenReady) return
        if (hub.readyGuests(queueId).connected == 0u) return
        player.pause()
        player.seekTo(0)
        val deadline = hub.scheduleStart(waitMs.toULong()).toLong()
        holdJob = scope.launch(Dispatchers.Main) {
            var start = deadline
            while (hub.nowMs().toLong() < deadline - START_MARGIN_MS) {
                val ready = hub.readyGuests(queueId)
                if (ready.ready >= ready.connected) {
                    // everyone has it: start shortly, so the new start time reaches them first
                    start = hub.scheduleStart(START_MARGIN_MS.toULong()).toLong()
                    break
                }
                delay(READY_POLL_MS)
            }
            delay((start - hub.nowMs().toLong()).coerceAtLeast(0))
            holdJob = null
            if (shareHub === hub) app.player.player?.play()
        }
    }

    private fun cancelHold() {
        holdJob?.cancel()
        holdJob = null
    }

    private fun byte(v: Float): Int = (v.coerceIn(0f, 1f) * 255).toInt()

    /** What guests hear: the player's position minus this phone's speaker delay. */
    private fun reportedPosition(p: Player): ULong =
        (p.currentPosition - share.speakerDelayMs).coerceAtLeast(0).toULong()

    private fun pushNowPlaying(p: Player) {
        val hub = shareHub ?: return
        val item = p.currentMediaItem
        currentPath = item?.mediaId?.takeIf(::isFilePath)
        if (item == null) {
            hub.setNowPlaying(null, null, 0u, false)
            hub.setAudio(null)
            return
        }
        val track = item.toShareTrack(app.library::find, p.duration.takeIf { it != C.TIME_UNSET })
        // the host's own songs got their ids when the queue was mirrored
        val queueId = item.queueId?.toULong() ?: hub.state().queue.firstOrNull { it.track.source == track.source }?.queueId
        hub.setNowPlaying(queueId, track, reportedPosition(p), p.isPlaying)
        pushAudio(item)
        reconcile()
    }

    /** The file guests that play along fetch: the library file, or the cached Tidal stream. */
    private fun pushAudio(item: MediaItem) {
        val hub = shareHub ?: return
        if (!_hub.value.party) return
        val tidal = tidalId(item.mediaId)
        when {
            isFilePath(item.mediaId) -> hub.setAudio(item.mediaId)
            tidal != null -> scope.launch {
                val path = runCatching { app.engine.fetchStream(tidal, streamDir) }.getOrNull()
                withContext(Dispatchers.Main) {
                    if (app.player.player?.currentMediaItem?.mediaId == item.mediaId) shareHub?.setAudio(path)
                }
            }
            else -> hub.setAudio(null)
        }
    }

    private var nextPushed: ULong? = null

    /** Main thread: the next song's file, so guests download it ahead and the change is seamless. */
    private fun pushNextAudio(state: ShareState) {
        val hub = shareHub ?: return
        val next = state.queue.firstOrNull()?.takeIf { _hub.value.party }
        if (next?.queueId == nextPushed) return
        nextPushed = next?.queueId
        if (next == null) return
        val source = next.track.source
        when {
            source.startsWith(HUB_SOURCE) -> hub.setNextAudio(next.queueId, source.removePrefix(HUB_SOURCE).takeIf { File(it).isFile })
            source.startsWith(TIDAL_SOURCE) -> scope.launch {
                val path = runCatching { app.engine.fetchStream(source.removePrefix(TIDAL_SOURCE), streamDir) }.getOrNull()
                withContext(Dispatchers.Main) { if (nextPushed == next.queueId) shareHub?.setNextAudio(next.queueId, path) }
            }
            else -> hub.setNextAudio(next.queueId, null)
        }
    }

    /**
     * After the host changed the player's list (a jump, a new album): tell
     * the hub, then tag the host's songs with the ids the hub gave them.
     */
    private fun reconcile() {
        val hub = shareHub ?: return
        if (pendingQueue.get() > 0) return // the hub's own change is on its way to the player
        val upcoming = app.player.upcoming()
        if (upcoming.map { it.queueId?.toULong() } == hub.state().queue.map { it.queueId }) return
        hub.setQueue(upcoming.map { ShareQueueEntry(it.queueId?.toULong(), it.toShareTrack(app.library::find)) })
        val ids = hub.state().queue.map { it.queueId }
        if (ids.size == upcoming.size) {
            app.player.retagUpcoming { i, item ->
                ids.getOrNull(i)?.toLong()?.takeIf { it != item.queueId }?.let { item.withShareInfo(it) }
            }
        }
        refreshHub()
    }

    /** "Play" / "Play next" / "Add to queue" in the app while sharing. */
    private fun hostPlay(items: List<MediaItem>, startIndex: Int?, next: Boolean): Boolean {
        val hub = shareHub ?: return false
        val p = app.player.player ?: return false
        val lookup = app.library::find
        if (startIndex == null) {
            // host songs take turns like anyone's; "next" pins them first
            items.forEach { hub.add(it.toShareTrack(lookup), next) }
            return true
        }
        if (p.mediaItemCount == 0) return false
        // the chosen song plays now; the rest of the list joins the shared queue
        val at = p.currentMediaItemIndex + 1
        p.addMediaItem(at, items[startIndex])
        p.seekToDefaultPosition(at)
        p.play()
        items.drop(startIndex + 1).forEach { hub.add(it.toShareTrack(lookup), false) }
        return true
    }

    internal fun onHubCommand(command: HubCommand) {
        if (command is HubCommand.Queue) {
            latestQueue.set(command.items)
            pendingQueue.incrementAndGet()
        }
        main.post {
            if (shareHub == null) return@post
            val player = app.player
            when (command) {
                HubCommand.Skip -> player.next()
                HubCommand.Toggle -> player.togglePlayPause()
                is HubCommand.Volume -> if (mutedVolume != null) {
                    mutedVolume = command.value.coerceIn(0f, 1f)
                    shareHub?.setVolume(command.value)
                } else {
                    player.setVolume(command.value)
                }
                is HubCommand.Queue -> {
                    // several changes in a row: only the newest list matters
                    latestQueue.getAndSet(null)?.let { list ->
                        player.replaceUpcoming(list.mapNotNull { it.toMediaItem(app.library::find) }, ::sameEntry)
                    }
                    pendingQueue.decrementAndGet()
                }
            }
            refreshHub()
        }
    }

    internal fun onHubEvent(event: HubEvent) {
        val s = share
        val notice: Pair<String, String?>? = when (event) {
            is HubEvent.GuestJoined -> if (s.noticeJoined) app.getString(R.string.sharing_notice_joined, event.guest.name) to event.guest.color else null
            is HubEvent.GuestLeft -> if (s.noticeLeft) app.getString(R.string.sharing_notice_left, event.guest.name) to event.guest.color else null
            is HubEvent.Added -> event.item.takeIf { s.noticeAdded && it.addedBy.guestId != HOST_ID }?.let {
                app.getString(R.string.sharing_notice_added, it.addedBy.name, it.track.title) to it.addedBy.color
            }
            is HubEvent.ApprovalRequested -> if (s.noticeApproval) {
                app.getString(R.string.sharing_notice_approval, event.item.addedBy.name, event.item.track.title) to event.item.addedBy.color
            } else {
                null
            }
        }
        refreshHub()
        if (event is HubEvent.GuestJoined || event is HubEvent.GuestLeft) {
            onMain { if (isHub) ShareService.update(app, app.getString(R.string.sharing_service_hub, _hub.value.info?.sessionName.orEmpty()), hubServiceText()) }
        }
        notice?.let { (text, color) -> notice(text, color) }
    }

    private fun notice(text: String, color: String?) {
        val item = ShareNotice(noticeIds.incrementAndGet(), text, color)
        _notices.update { (it + item).takeLast(MAX_NOTICES) }
        scope.launch {
            delay(share.noticeSeconds.coerceIn(1, 30) * 1000L)
            dismissNotice(item.id)
        }
    }

    private fun refreshHub() {
        val hub = shareHub ?: return
        scope.launch {
            val state = hub.state()
            val guests = hub.guests()
            val pending = hub.pending()
            _hub.update { if (it.running) it.copy(state = state, guests = guests, pending = pending) else it }
            withContext(Dispatchers.Main) { pushNextAudio(state) }
            // in a room: stop scanning, the radio is for the music now
            host.setQuiet(guests.isNotEmpty() && !share.visibleWithGuests)
        }
    }

    private fun hubServiceText(): String {
        val ui = _hub.value
        return app.getString(R.string.sharing_service_hub_text, ui.info?.code.orEmpty(), ui.guests.size)
    }

    private fun qrPayload(info: HubInfo, network: HostNetwork): String {
        val uri = Uri.parse(info.joinUri).buildUpon()
        network.networkName?.let { uri.appendQueryParameter("net", it) }
        network.passphrase?.let { uri.appendQueryParameter("pass", it) }
        uri.appendQueryParameter("mode", network.mode.name)
        return uri.build().toString()
    }

    // ---------------------------------------------------------------- guest

    /** A `homefi://join?...` link (QR code, deep link); null when it is not one. */
    fun parseLink(link: String): HubAddress? {
        val info = parseJoinUri(link) ?: return null
        val uri = Uri.parse(link.trim())
        val net = uri.getQueryParameter("net")?.ifBlank { null }
        val mode = uri.getQueryParameter("mode")?.let { m -> ShareNetworkMode.entries.firstOrNull { it.name == m } }
            ?: if (net != null) ShareNetworkMode.WIFI_DIRECT else ShareNetworkMode.LAN
        return HubAddress(
            sessionName = info.sessionName,
            host = info.host,
            port = info.port.toInt(),
            mode = mode,
            networkName = net,
            passphrase = uri.getQueryParameter("pass")?.ifBlank { null },
            code = info.code.ifBlank { null },
        )
    }

    fun requestJoin(address: HubAddress?) {
        _joinRequest.value = JoinRequest(address)
    }

    fun dismissJoin() {
        _joinRequest.value = null
    }

    fun openGuest(open: Boolean) {
        _guestOpen.value = open && isGuest
    }

    fun join(address: HubAddress, code: String, name: String, color: String) {
        if (_guest.value.status == GuestStatus.CONNECTING) return
        if (isHub) {
            _guest.update { it.copy(status = GuestStatus.DISCONNECTED, error = app.getString(R.string.sharing_busy_hub)) }
            return
        }
        if (isGuest) leave()
        val session = guestSession.incrementAndGet()
        _guest.value = GuestUi(status = GuestStatus.CONNECTING, sessionName = address.sessionName, playAlong = share.playAlong)
        scope.launch {
            try {
                val hostAddress = finder.connect(address)
                val guest = joinHub(hostAddress, address.port.toUShort(), name.trim().ifBlank { displayName() }, color, code.trim(), Listener(session))
                if (session != guestSession.get()) {
                    guest.disconnect()
                    guest.destroy()
                    return@launch
                }
                shareGuest = guest
                val welcome = guest.welcome()
                _guest.update {
                    it.copy(status = GuestStatus.CONNECTED, sessionName = welcome.sessionName, guestId = welcome.guestId, state = it.state ?: welcome.state, error = null)
                }
                _guestOpen.value = true
                // in the room: nothing left to look for (the screens stop it too; this is the guarantee)
                finder.stop()
                withContext(Dispatchers.Main) {
                    startPartyLoop()
                    ShareService.onStopRequested = { leave() }
                    ShareService.start(app, app.getString(R.string.sharing_service_guest, welcome.sessionName), app.getString(R.string.sharing_service_guest_text))
                }
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                runCatching { finder.leave() }
                if (session == guestSession.get()) _guest.update { it.copy(status = GuestStatus.DISCONNECTED, error = e.userMessage()) }
            }
        }
    }

    /** Leaves the session (also from the notification). */
    fun leave() {
        guestSession.incrementAndGet() // late events of this session are ignored
        onMain { teardownGuest() }
        _guest.value = GuestUi(playAlong = share.playAlong)
        _guestOpen.value = false
    }

    /** Forgets a failed / ended session. */
    fun clearGuest() {
        if (!isGuest) _guest.value = GuestUi(playAlong = share.playAlong)
    }

    private fun teardownGuest() {
        partyJob?.cancel()
        partyJob = null
        party.release()
        val guest = shareGuest
        shareGuest = null
        if (guest != null) {
            ShareService.onStopRequested = null
            ShareService.stop(app)
            runCatching { guest.disconnect() }
            guest.destroy()
            scope.launch { runCatching { finder.leave() } }
        }
    }

    private inner class Listener(private val session: Long) : GuestListener {
        override fun onEvent(event: GuestEvent) {
            if (session != guestSession.get()) return
            when (event) {
                is GuestEvent.State -> _guest.update { it.copy(state = event.state) }
                is GuestEvent.Results -> _guest.update { it.copy(results = event.results, searching = false) }
                is GuestEvent.Page -> _guest.update { it.copy(page = event.page, searching = false) }
                is GuestEvent.Notice -> _guestMessages.tryEmit(event.text)
                // the hub's meters, unless this phone plays the audio itself (then it reads its own)
                is GuestEvent.Levels -> if (!_guest.value.playingHere) {
                    AudioTap.feed(
                        event.left.toFloat() / 255f,
                        event.right.toFloat() / 255f,
                        FloatArray(event.bands.size) { (event.bands[it].toInt() and 0xff) / 255f },
                    )
                }
                is GuestEvent.Denied -> {
                    _guest.update { it.copy(searching = false) }
                    _guestMessages.tryEmit(event.reason)
                }
                is GuestEvent.Disconnected -> {
                    guestSession.incrementAndGet()
                    onMain { teardownGuest() }
                    _guest.update {
                        GuestUi(
                            status = GuestStatus.DISCONNECTED,
                            sessionName = it.sessionName,
                            error = event.reason ?: app.getString(R.string.sharing_guest_lost),
                            playAlong = it.playAlong,
                        )
                    }
                    _guestOpen.value = false
                }
            }
        }
    }

    private fun guestCall(block: (ShareGuest) -> Unit) {
        val guest = shareGuest ?: return
        try {
            block(guest)
        } catch (e: Exception) {
            _guestMessages.tryEmit(e.userMessage())
        }
    }

    fun search(q: String) {
        if (q.isBlank()) return
        _guest.update { it.copy(searching = true, page = null) }
        guestCall { it.search(q.trim()) }
    }

    fun browse(kind: ShareBrowseKind, id: String) {
        _guest.update { it.copy(searching = true) }
        guestCall { it.browse(kind, id) }
    }

    fun closePage() = _guest.update { it.copy(page = null) }

    fun add(track: ShareTrack, next: Boolean) = guestCall { it.add(track, next) }

    /** Local confirmation after a request (a refusal from the hub follows if any). */
    fun notifySent(title: String) {
        _guestMessages.tryEmit(app.getString(R.string.sharing_guest_sent, title))
    }

    fun remove(queueId: ULong) = guestCall { it.remove(queueId) }

    /** Moves a queued song to [to] among the songs waiting (0 = next). */
    fun move(queueId: ULong, to: Int) = guestCall { it.moveItem(queueId, to.coerceAtLeast(0).toUInt()) }

    /** Plays a queued song now (the hub checks the rules too). */
    fun playNow(queueId: ULong) = guestCall { it.playNow(queueId) }

    fun skip() = guestCall { it.skip() }

    fun voteSkip() = guestCall { it.voteSkip() }

    fun toggle() = guestCall { it.toggle() }

    fun volume(value: Float) = guestCall { it.volume(value) }

    /** Makes hub paths (covers) absolute. */
    fun resolveUrl(url: String?): String? = url?.let { u -> shareGuest?.resolveUrl(u) ?: u }

    /** Where the hub's song is now (interpolated on the hub's clock). */
    fun guestPositionMs(): Long {
        val state = _guest.value.state ?: return 0
        val guest = shareGuest ?: return state.positionMs.toLong()
        return guest.expectedPositionMs(state.positionMs, state.clockMs, state.playing).toLong()
    }

    fun setPlayAlong(on: Boolean) = _guest.update { it.copy(playAlong = on) }

    /** Main thread: party playback follows the hub. */
    private var partyStartedAt = 0L
    private var readySent: ULong? = null

    private fun startPartyLoop() {
        partyJob?.cancel()
        partyJob = scope.launch(Dispatchers.Main) {
            while (isActive) {
                val guest = shareGuest
                // the user put on their own music: stop playing along rather than both at once
                // (not right after the party started: pausing the app's player takes a moment)
                val settled = SystemClock.elapsedRealtime() - partyStartedAt > OWN_MUSIC_GRACE_MS
                if (party.active && settled && app.player.state.value.isPlaying) {
                    party.release()
                    setPlayAlong(false)
                    Toast.makeText(app, R.string.sharing_guest_own_music, Toast.LENGTH_SHORT).show()
                }
                val ui = _guest.value
                val state = ui.state
                // not playing along: never make the others wait for this phone
                val nowId = state?.nowPlaying?.queueId
                if (guest != null && nowId != null && nowId != readySent && (!ui.playAlong || state.audio == null)) {
                    readySent = nowId
                    runCatching { guest.ready(nowId) }
                }
                when {
                    guest == null -> party.release()
                    !ui.playAlong -> party.release()
                    state?.audio == null -> party.pause()
                    party.tick(guest, state, share.speakerDelayMs, syncBySpeed = share.syncBySpeed, preload = share.preloadSongs) -> {
                        partyStartedAt = SystemClock.elapsedRealtime()
                        app.player.pause()
                        Toast.makeText(app, R.string.sharing_guest_playing_here, Toast.LENGTH_SHORT).show()
                    }
                }
                val here = party.active && ui.playAlong && state?.audio != null && state.playing
                val rtt = guest?.clockRoundTripMs()?.toLong()
                if (here != ui.playingHere || rtt != ui.roundTripMs) _guest.update { it.copy(playingHere = here, roundTripMs = rtt) }
                delay(PARTY_TICK_MS)
            }
        }
    }

    private fun onMain(block: () -> Unit) {
        if (Looper.myLooper() == Looper.getMainLooper()) block() else main.post(block)
    }

    companion object {
        /** `guest_id` of the hub's own songs. */
        const val HOST_ID = "hub"
        private const val POSITION_EVERY_MS = 3_000L
        private const val PARTY_TICK_MS = 250L
        /** A scheduled start is this far ahead, so the start time reaches every guest in time. */
        private const val START_MARGIN_MS = 350L
        private const val READY_POLL_MS = 100L
        /** ~15 frames a second: smooth enough for the meters, a few hundred bytes a second. */
        private const val LEVELS_EVERY_MS = 66L
        private const val OWN_MUSIC_GRACE_MS = 2_000L
        private const val MAX_NOTICES = 3
    }
}
