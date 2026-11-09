package com.perseusoft.homefi.player.exercise

import android.content.Context
import android.net.wifi.WifiManager
import com.perseusoft.homefi.HomefiApplication
import com.perseusoft.homefi.core.RideListen
import com.perseusoft.homefi.core.RideTvEndpoint
import com.perseusoft.homefi.core.TvAudio
import com.perseusoft.homefi.core.exerciseDefaultSettings
import com.perseusoft.homefi.core.exerciseTvDiscover
import com.perseusoft.homefi.core.exerciseTvParse
import com.perseusoft.homefi.share.GuestStatus
import com.perseusoft.homefi.share.net.HubAddress
import com.perseusoft.homefi.share.net.ShareNetworkMode
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.net.HttpURLConnection
import java.net.URL

/** What the TV app shows. */
sealed interface TvClientUi {
    /** Looking for a ride on the network ("Waiting for a ride"). */
    data object Searching : TvClientUi

    data class Choose(val rides: List<RideTvEndpoint>) : TvClientUi

    /**
     * Showing [endpoint]'s page. [listening]: this TV plays the ride's music as a listen-only guest of
     * the phone's hub and tells the page where its own playback is.
     */
    data class Watching(val endpoint: RideTvEndpoint, val listening: Boolean = false) : TvClientUi
}

/**
 * HomeFi on an Android TV / Chromecast with Google TV: a pure client of a phone's ride. It finds rides
 * (mDNS `_homefi-ride._tcp`, with a multicast lock), and while one is shown it polls `/ride.json` at 2 Hz
 * only to learn whether the music plays here: with `listen` present it joins the ride's hub as a guest
 * with the one-ride code and plays on the shared clock. The picture is the core's own page, in a WebView.
 */
class RideTvClient(private val app: HomefiApplication) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val _ui = MutableStateFlow<TvClientUi>(TvClientUi.Searching)
    val ui: StateFlow<TvClientUi> = _ui.asStateFlow()

    private val lock = (app.getSystemService(Context.WIFI_SERVICE) as? WifiManager)
        ?.createMulticastLock(LOCK_TAG)?.apply { setReferenceCounted(false) }
    private var job: Job? = null
    private var joinedCode: String? = null

    /** Starts looking (the activity is in front). */
    fun start() {
        if (job?.isActive == true) return
        runCatching { lock?.acquire() }
        job = scope.launch {
            while (isActive) {
                when (val state = _ui.value) {
                    is TvClientUi.Watching -> watch(state.endpoint)
                    else -> discover()
                }
            }
        }
    }

    fun stop() {
        job?.cancel()
        job = null
        runCatching { if (lock?.isHeld == true) lock.release() }
        leaveHub()
    }

    fun pick(endpoint: RideTvEndpoint) {
        _ui.value = TvClientUi.Watching(endpoint)
    }

    private val prefs = app.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    /** The address typed last time ("ip:port"), offered again when the network cannot find rides. */
    fun savedAddress(): String? = prefs.getString(KEY_ADDRESS, null)

    /** Opens a ride at an address typed by hand and remembers it; false when [text] is not an address. */
    fun openAddress(text: String): Boolean {
        val endpoint = parseRideAddress(text, exerciseDefaultSettings().tvPort.toInt()) ?: return false
        prefs.edit().putString(KEY_ADDRESS, "${endpoint.host}:${endpoint.port}").apply()
        pick(endpoint)
        return true
    }

    /** Back from a ride: the list again. */
    fun stopWatching() {
        leaveHub()
        _ui.value = TvClientUi.Searching
    }

    /** Where this TV's own playback is in the ride's song (ms), or -1 when it does not play the music. */
    fun songPositionMs(): Long = if (joinedCode != null && app.share.guest.value.status == GuestStatus.CONNECTED) app.share.guestPositionMs() else -1L

    private suspend fun discover() {
        val found = runCatching { exerciseTvDiscover(DISCOVER_MS.toUInt()) }.getOrDefault(emptyList())
            .distinctBy { "${it.host}:${it.port}" }
        // a ride picked (by hand or by a phone's pairing) while we looked stays picked
        _ui.update { if (it is TvClientUi.Watching) it else if (found.isEmpty()) TvClientUi.Searching else TvClientUi.Choose(found) }
        delay(DISCOVER_EVERY_MS)
    }

    private suspend fun watch(endpoint: RideTvEndpoint) {
        var failures = 0
        // another ride picked meanwhile (a phone paired mid-ride): the loop above starts watching that one
        while ((_ui.value as? TvClientUi.Watching)?.endpoint == endpoint) {
            val view = withContext(Dispatchers.IO) {
                runCatching { exerciseTvParse(fetch(endpoint.rideUrl)) }.getOrNull()
            }
            if (view == null) {
                if (++failures >= GIVE_UP_AFTER) {
                    stopWatching()
                    return
                }
            } else {
                failures = 0
                val listen = view.listen.takeIf { view.audio == TvAudio.TV && view.ride != null }
                if (listen != null) joinHub(endpoint, listen) else if (joinedCode != null) leaveHub()
                val listening = listen != null && app.share.guest.value.status == GuestStatus.CONNECTED
                _ui.update { if (it is TvClientUi.Watching && it.listening != listening) it.copy(listening = listening) else it }
            }
            delay(POLL_MS)
        }
    }

    private fun joinHub(endpoint: RideTvEndpoint, listen: RideListen) {
        if (joinedCode == listen.code) return
        joinedCode = listen.code
        app.share.join(
            HubAddress(
                sessionName = listen.name, host = endpoint.host, port = listen.port.toInt(),
                mode = ShareNetworkMode.LAN, networkName = null, passphrase = null, code = listen.code,
            ),
            listen.code, TV_NAME, TV_COLOR,
        )
        scope.launch {
            // a TV always plays what the ride plays, whatever the phone setting for guests says
            while (isActive && joinedCode == listen.code) {
                if (app.share.guest.value.status == GuestStatus.CONNECTED) {
                    app.share.setPlayAlong(true)
                    return@launch
                }
                delay(JOIN_POLL_MS)
            }
        }
    }

    private fun leaveHub() {
        if (joinedCode != null) {
            joinedCode = null
            app.share.leave()
        }
    }

    private fun fetch(url: String): String {
        val connection = URL(url).openConnection() as HttpURLConnection
        return try {
            connection.connectTimeout = HTTP_TIMEOUT_MS
            connection.readTimeout = HTTP_TIMEOUT_MS
            connection.inputStream.bufferedReader().use { it.readText() }
        } catch (e: CancellationException) {
            throw e
        } finally {
            connection.disconnect()
        }
    }

    private companion object {
        const val PREFS = "tv_client"
        const val KEY_ADDRESS = "address"
        const val LOCK_TAG = "homefi-ride-tv-client"
        const val TV_NAME = "TV"
        const val TV_COLOR = "#0891B2"
        const val DISCOVER_MS = 2_500
        const val DISCOVER_EVERY_MS = 1_500L
        const val POLL_MS = 500L
        const val JOIN_POLL_MS = 250L
        const val HTTP_TIMEOUT_MS = 1_500
        /** 60 polls at 2 Hz: the phone is gone, look again. */
        const val GIVE_UP_AFTER = 60
    }
}

/** The page the endpoint serves (the core's `ride.html`), as a TV browser would open it. */
val RideTvEndpoint.pageUrl: String get() = rideUrl.removeSuffix("ride.json")
