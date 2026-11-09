package com.perseusoft.homefi.tv.train

import android.os.Build
import android.provider.Settings
import com.perseusoft.homefi.HomefiApplication
import com.perseusoft.homefi.core.RideTvEndpoint
import com.perseusoft.homefi.core.exerciseDefaultSettings
import com.perseusoft.homefi.core.exerciseTvParse
import com.perseusoft.homefi.pair.PairReply
import com.perseusoft.homefi.pair.PairRequest
import com.perseusoft.homefi.pair.TvPairLink
import com.perseusoft.homefi.pair.TvPairProtocol
import com.perseusoft.homefi.player.exercise.RideTvClient
import com.perseusoft.homefi.player.exercise.TvClientUi
import com.perseusoft.homefi.player.exercise.parseRideAddress
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
import java.net.InetAddress
import java.net.URL
import java.security.SecureRandom

/** What the waiting screen shows: the QR's content, the short code, who is paired and who just paired. */
data class PairPanel(
    val host: String? = null,
    val port: Int = 0,
    val code: String = "",
    val phones: List<PairedPhone> = emptyList(),
    /** The phone that paired last ("Pixel 10"), shown once. */
    val justPaired: String? = null,
) {
    /** The link the QR holds; null while the TV has no network or the endpoint is not up. */
    val link: String? get() = if (host != null && port > 0 && code.isNotEmpty()) TvPairLink(host, port, code).toLink() else null
}

/**
 * The Train section's brain on the TV: the ride client (mDNS + typed address), the LAN pairing
 * endpoint with its one-time code, the remembered phones, and the automatic opening of rides (a phone
 * that pairs, a remembered phone found by mDNS, a remembered address that answers). Main thread; it
 * runs only while the section is in front ([start] / [stop]).
 */
class TvTrainController(private val app: HomefiApplication) {
    val client = RideTvClient(app)
    private val phones = PairedPhones(app)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val random = SecureRandom()
    private val server = PairServer(::onPair)
    private val _panel = MutableStateFlow(PairPanel(phones = phones.all()))
    val panel: StateFlow<PairPanel> = _panel.asStateFlow()
    private var job: Job? = null
    private var badAttempts = 0

    /** When the user last left a ride on purpose: auto-connect waits, or Back would reopen it at once. */
    private var autoPausedUntilMs = 0L

    fun start() {
        if (job?.isActive == true) return
        client.start()
        val port = server.start(TvPairProtocol.DEFAULT_PORT)
        _panel.update { it.copy(host = lanAddress(), port = port, code = newCode(), phones = phones.all()) }
        job = scope.launch {
            launch { pauseAfterLeaving() }
            autoConnect()
        }
    }

    fun stop() {
        job?.cancel()
        job = null
        server.stop()
        client.stop()
    }

    fun forgetPhones() {
        phones.clear()
        _panel.update { it.copy(phones = emptyList(), justPaired = null) }
    }

    /** The address may change (Wi-Fi came back): refresh what the QR says. */
    fun refreshAddress() {
        _panel.update { it.copy(host = lanAddress() ?: it.host) }
    }

    // ------------------------------------------------------------ pairing (runs on a server thread)

    private fun onPair(request: PairRequest): PairReply? {
        val tvName = tvName()
        val phone = synchronized(this) {
            val known = request.token?.let(phones::byToken)
            when {
                known != null -> known.also { phones.update(it.token, request.rideUrl.ifEmpty { it.rideUrl }.let(::pageOrEmpty)) }
                request.code != null && consumeCode(request.code) -> phones.add(request.phoneName, pageOrEmpty(request.rideUrl))
                else -> null
            }
        } ?: return null
        val endpoint = if (request.rideUrl.isNotEmpty() && isLanUrl(request.rideUrl)) {
            parseRideAddress(request.rideUrl, exerciseDefaultSettings().tvPort.toInt())?.copy(name = phone.name)
        } else {
            null
        }
        scope.launch {
            _panel.update { it.copy(phones = phones.all(), justPaired = if (request.code != null) phone.name else it.justPaired) }
            if (endpoint != null) client.pick(endpoint)
        }
        return PairReply(tvName, phone.token)
    }

    private fun pageOrEmpty(rideUrl: String): String = if (isLanUrl(rideUrl)) rideUrl else ""

    /** One-time: a right code is spent (a new one shows), wrong ones are counted and the code changes after too many. */
    private fun consumeCode(code: String): Boolean {
        if (code == _panel.value.code) {
            badAttempts = 0
            rotateCode()
            return true
        }
        if (++badAttempts >= MAX_BAD_ATTEMPTS) {
            badAttempts = 0
            rotateCode()
        }
        return false
    }

    private fun rotateCode() {
        _panel.update { it.copy(code = newCode()) }
    }

    private fun newCode(): String = (0 until TvPairProtocol.CODE_DIGITS).joinToString("") { random.nextInt(DIGIT_RADIX).toString() }

    private fun tvName(): String =
        Settings.Global.getString(app.contentResolver, "device_name")?.takeIf { it.isNotBlank() } ?: Build.MODEL

    // ------------------------------------------------------------ a remembered phone starts a ride

    private suspend fun pauseAfterLeaving() {
        var wasWatching = false
        client.ui.collect { ui ->
            val watching = ui is TvClientUi.Watching
            if (watching) _panel.update { it.copy(justPaired = null) }
            if (wasWatching && !watching) autoPausedUntilMs = System.currentTimeMillis() + LEFT_PAUSE_MS
            wasWatching = watching
        }
    }

    private suspend fun autoConnect() {
        while (scope.isActive) {
            delay(AUTO_EVERY_MS)
            val known = phones.all()
            val ui = client.ui.value
            if (known.isEmpty() || ui is TvClientUi.Watching || System.currentTimeMillis() < autoPausedUntilMs) continue
            // mDNS first: a found ride whose host is a remembered phone's
            val byMdns = (ui as? TvClientUi.Choose)?.rides?.firstOrNull { ride -> known.any { it.host == ride.host } }
            if (byMdns != null) {
                client.pick(byMdns)
                continue
            }
            // then the remembered address, when the router hides mDNS
            val byAddress = known.firstNotNullOfOrNull { phone -> probe(phone) }
            if (byAddress != null) client.pick(byAddress)
        }
    }

    /** The ride at the phone's remembered address, if one is running there right now. */
    private suspend fun probe(phone: PairedPhone): RideTvEndpoint? = withContext(Dispatchers.IO) {
        val endpoint = parseRideAddress(phone.rideUrl, exerciseDefaultSettings().tvPort.toInt()) ?: return@withContext null
        val connection = runCatching { URL(endpoint.rideUrl).openConnection() as HttpURLConnection }.getOrNull() ?: return@withContext null
        try {
            connection.connectTimeout = PROBE_TIMEOUT_MS
            connection.readTimeout = PROBE_TIMEOUT_MS
            val view = exerciseTvParse(connection.inputStream.bufferedReader().use { it.readText() })
            endpoint.copy(name = phone.name).takeIf { view.ride != null }
        } catch (e: CancellationException) {
            throw e
        } catch (_: Exception) {
            null
        } finally {
            connection.disconnect()
        }
    }

    /** A phone may only point the TV at its own LAN: a private IPv4 literal, never a name or the internet. */
    private fun isLanUrl(url: String): Boolean {
        val host = url.removePrefix("http://").substringBefore('/').substringBefore(':')
        if (!IPV4.matches(host)) return false
        return runCatching { InetAddress.getByName(host).let { it.isSiteLocalAddress || it.isLoopbackAddress || it.isLinkLocalAddress } }
            .getOrDefault(false)
    }

    private companion object {
        val IPV4 = Regex("""^\d{1,3}(\.\d{1,3}){3}$""")
        const val DIGIT_RADIX = 10
        const val MAX_BAD_ATTEMPTS = 5
        const val AUTO_EVERY_MS = 3_000L
        const val LEFT_PAUSE_MS = 30_000L
        const val PROBE_TIMEOUT_MS = 1_000
    }
}
