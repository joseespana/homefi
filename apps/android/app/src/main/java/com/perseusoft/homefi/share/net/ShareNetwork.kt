package com.perseusoft.homefi.share.net

import android.annotation.SuppressLint
import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.net.wifi.WpsInfo
import android.net.wifi.p2p.WifiP2pConfig
import android.net.wifi.p2p.WifiP2pManager
import android.net.wifi.p2p.nsd.WifiP2pDnsSdServiceInfo
import android.net.wifi.p2p.nsd.WifiP2pDnsSdServiceRequest
import android.os.Build
import android.os.SystemClock
import android.util.Log
import com.perseusoft.homefi.HomefiApplication
import com.perseusoft.homefi.R
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import java.net.Inet4Address
import java.security.SecureRandom

// How phones find and reach each other for shared control / party mode.
// CONTRACT shared by the share manager and the network layer: keep these
// signatures; bodies are filled in by the network layer.

/** How the hub is reached. */
enum class ShareNetworkMode {
    /** Its own Wi-Fi Direct group: no router or internet needed (beach, car). */
    WIFI_DIRECT,

    /** The Wi-Fi everyone is already on (home). */
    LAN,
}

/** Thrown with a message the user can read (already localized). */
class ShareNetworkException(message: String, cause: Throwable? = null) : Exception(message, cause)

/** What the hub's network looks like once it is up. */
data class HostNetwork(
    val mode: ShareNetworkMode,
    /** Address guests connect to (the Wi-Fi Direct group owner, or this phone's LAN address). */
    val host: String,
    /** Wi-Fi Direct group name and passphrase (null on LAN): go into the QR code and the discovery record. */
    val networkName: String?,
    val passphrase: String?,
)

/** A hub a guest can join: found nearby, or read from a QR code / link. */
data class HubAddress(
    val sessionName: String,
    /** Known on LAN or from a QR code; null until connected for a Wi-Fi Direct hub found nearby. */
    val host: String?,
    val port: Int,
    val mode: ShareNetworkMode,
    val networkName: String?,
    val passphrase: String?,
    /** Join code when it came from a QR code / link (discovery never carries it). */
    val code: String? = null,
)

/** Port the hub listens on when a discovery record does not say. */
private const val DEFAULT_PORT = 47800

/** The hub's side: sets up the network and announces the session. */
@SuppressLint("MissingPermission")
class ShareHost(private val context: Context) {
    private val app = context.applicationContext
    private val p2p by lazy { P2p(app) }
    private val nsd: NsdManager? by lazy { app.getSystemService(Context.NSD_SERVICE) as NsdManager? }
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

    private var network: HostNetwork? = null
    private var ownsGroup = false
    private var announceJob: Job? = null
    private var registration: NsdManager.RegistrationListener? = null
    private val ble by lazy { BleAnnouncer(app) }
    private var bleJob: Job? = null

    /**
     * Creates the Wi-Fi Direct group (or uses the LAN). Throws [ShareNetworkException].
     * [groupName] / [groupPass] keep the group the same between parties (Android 10+;
     * see [newGroupName]); null makes up new ones.
     */
    suspend fun start(
        mode: ShareNetworkMode,
        groupName: String? = null,
        groupPass: String? = null,
    ): HostNetwork = withContext(Dispatchers.Main.immediate) {
        network?.let { current ->
            if (current.mode == mode && stillUp(current)) return@withContext current
            stopNow()
        }
        val started = when (mode) {
            ShareNetworkMode.WIFI_DIRECT -> startGroup(groupName, groupPass)
            ShareNetworkMode.LAN -> HostNetwork(
                mode = ShareNetworkMode.LAN,
                host = Lan.wifiAddress(app) ?: throw ShareNetworkException(app.getString(R.string.share_net_not_on_wifi)),
                networkName = null,
                passphrase = null,
            )
        }
        network = started
        started
    }

    /**
     * Announces "<sessionName>" on [port] (Wi-Fi Direct service discovery, or NSD on the LAN).
     * With [alsoLan], a Wi-Fi Direct hub is announced on the home Wi-Fi too when this phone
     * is on one (the hub listens on every interface), so devices at home find it as well.
     * With [bluetooth], it is also announced over Bluetooth LE (needs no location services:
     * players without GPS can be found that way); skipped quietly when Bluetooth is off,
     * missing or not permitted.
     */
    fun announce(sessionName: String, port: Int, alsoLan: Boolean = false, bluetooth: Boolean = false) {
        scope.launch {
            val net = network
            if (net == null) {
                Log.w(TAG, "announce before start")
                return@launch
            }
            stopAnnouncing()
            when (net.mode) {
                ShareNetworkMode.WIFI_DIRECT -> {
                    announceJob = scope.launch { announceP2p(net, sessionName, port) }
                    if (alsoLan && Lan.wifiAddress(app) != null) registerNsd(sessionName, port)
                }
                ShareNetworkMode.LAN -> registerNsd(sessionName, port)
            }
            if (bluetooth) bleJob = scope.launch { announceBle(net, sessionName, port, alsoLan) }
        }
    }

    /** Stops announcing and removes the group. */
    suspend fun stop() {
        withContext(NonCancellable + Dispatchers.Main.immediate) { stopNow() }
    }

    private suspend fun stillUp(net: HostNetwork): Boolean = when (net.mode) {
        ShareNetworkMode.LAN -> Lan.wifiAddress(app) == net.host
        ShareNetworkMode.WIFI_DIRECT -> p2p.connectionInfo()?.let { it.groupFormed && it.isGroupOwner } == true
    }

    private suspend fun startGroup(groupName: String?, groupPass: String?): HostNetwork {
        p2p.ensureReady(scanning = false)
        try {
            // A group left over (ours or another app's) makes createGroup fail with BUSY.
            if (p2p.connectionInfo()?.groupFormed == true) {
                p2p.removeGroup()
                p2p.awaitConnection(3_000) { !it.groupFormed }
            }
            val name = groupName?.takeIf { validGroupName(it) } ?: newGroupName()
            val pass = groupPass?.takeIf { it.length in 8..63 } ?: newGroupPass()
            var custom = false
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                val config = runCatching {
                    WifiP2pConfig.Builder()
                        .setNetworkName(name)
                        .setPassphrase(pass)
                        .enablePersistentMode(false)
                        .build()
                }.getOrNull()
                if (config != null) {
                    custom = try {
                        createGroup { m, c, l -> m.createGroup(c, config, l) }
                        true
                    } catch (e: P2pFailure) {
                        // Some vendors reject custom credentials: fall back to a generated group.
                        if (e.reason == WifiP2pManager.P2P_UNSUPPORTED) throw e
                        Log.w(TAG, "createGroup(config) failed: ${e.reason}, trying the legacy call")
                        false
                    }
                }
            }
            if (!custom) createGroup { m, c, l -> m.createGroup(c, l) }
            ownsGroup = true

            val info = p2p.awaitConnection(GROUP_TIMEOUT_MS) {
                it.groupFormed && it.isGroupOwner && it.groupOwnerAddress != null
            } ?: run {
                p2p.removeGroup()
                ownsGroup = false
                throw ShareNetworkException(app.getString(R.string.share_net_group_failed))
            }

            var group = p2p.groupInfo()
            var tries = 0
            while ((group?.networkName == null || group.passphrase == null) && tries++ < 10) {
                delay(300)
                group = p2p.groupInfo()
            }
            return HostNetwork(
                mode = ShareNetworkMode.WIFI_DIRECT,
                host = info.groupOwnerAddress.hostAddress ?: GROUP_OWNER_IP,
                networkName = group?.networkName ?: name.takeIf { custom },
                passphrase = group?.passphrase ?: pass.takeIf { custom },
            )
        } catch (e: P2pFailure) {
            throw ShareNetworkException(p2p.message(e, app.getString(R.string.share_net_group_failed)), e)
        } catch (e: CancellationException) {
            if (ownsGroup) withContext(NonCancellable) { p2p.removeGroup() }
            ownsGroup = false
            throw e
        }
    }

    /** createGroup, removing a busy group once and retrying. */
    private suspend fun createGroup(
        block: (WifiP2pManager, WifiP2pManager.Channel, WifiP2pManager.ActionListener) -> Unit,
    ) {
        try {
            p2p.call(block)
        } catch (e: P2pFailure) {
            if (e.reason != WifiP2pManager.BUSY) throw e
            p2p.removeGroup()
            delay(500)
            p2p.call(block)
        }
    }

    private suspend fun announceP2p(net: HostNetwork, sessionName: String, port: Int) {
        val txt = buildMap {
            put("v", "1")
            put("port", port.toString())
            put("name", fitBytes(sessionName, 200))
            net.networkName?.let { put("net", it) }
            net.passphrase?.let { put("pass", it) }
        }
        val service = WifiP2pDnsSdServiceInfo.newInstance(instanceName(sessionName), SERVICE_TYPE, txt)
        p2p.attempt("clearLocalServices") { m, c, l -> m.clearLocalServices(c, l) }
        var added = p2p.attempt("addLocalService") { m, c, l -> m.addLocalService(c, service, l) }
        if (!added) {
            delay(1_000)
            added = p2p.attempt("addLocalService") { m, c, l -> m.addLocalService(c, service, l) }
        }
        if (!added) Log.w(TAG, "could not announce the Wi-Fi Direct service")
        // Many phones only answer service discovery while they are themselves discoverable.
        // Each scan takes the radio off the group's channel, so it stops while guests are in
        // ([setQuiet]): their audio would stutter.
        while (currentCoroutineContext().isActive) {
            if (!quiet) p2p.attempt("discoverPeers") { m, c, l -> m.discoverPeers(c, l) }
            delay(DISCOVERABLE_EVERY_MS)
        }
    }

    @Volatile
    private var quiet = false

    /**
     * Quiet: stop scanning over Wi-Fi Direct (guests are in and streaming). The service stays
     * registered and Bluetooth, NSD and the gateway keep answering newcomers.
     */
    fun setQuiet(on: Boolean) {
        if (quiet == on) return
        quiet = on
        scope.launch {
            if (network?.mode != ShareNetworkMode.WIFI_DIRECT) return@launch
            if (on) p2p.attempt("stopPeerDiscovery") { m, c, l -> m.stopPeerDiscovery(c, l) }
            else p2p.attempt("discoverPeers") { m, c, l -> m.discoverPeers(c, l) }
        }
    }

    private suspend fun announceBle(net: HostNetwork, sessionName: String, port: Int, alsoLan: Boolean) {
        val lanHost = when (net.mode) {
            ShareNetworkMode.LAN -> net.host
            ShareNetworkMode.WIFI_DIRECT -> if (alsoLan) Lan.wifiAddress(app) else null
        }
        // Guests below API 29 join by the hub's device address, not by name and passphrase.
        val device = if (net.mode == ShareNetworkMode.WIFI_DIRECT) p2p.ownDeviceAddress() else null
        ble.start(
            BleRecord(
                name = sessionName,
                port = port,
                mode = net.mode,
                networkName = net.networkName,
                passphrase = net.passphrase,
                lanHost = lanHost,
                deviceAddress = device,
            ),
        )
    }

    private fun registerNsd(sessionName: String, port: Int) {
        val manager = nsd ?: return
        val info = NsdServiceInfo().apply {
            serviceName = fitBytes(sessionName, 63)
            serviceType = SERVICE_TYPE
            this.port = port
            setAttribute("v", "1")
            setAttribute("name", fitBytes(sessionName, 200))
            setAttribute("port", port.toString())
        }
        val listener = object : NsdManager.RegistrationListener {
            override fun onServiceRegistered(info: NsdServiceInfo) {
                Log.d(TAG, "NSD registered as ${info.serviceName}")
            }

            override fun onRegistrationFailed(info: NsdServiceInfo, errorCode: Int) {
                Log.w(TAG, "NSD registration failed: $errorCode")
            }

            override fun onServiceUnregistered(info: NsdServiceInfo) {}

            override fun onUnregistrationFailed(info: NsdServiceInfo, errorCode: Int) {
                Log.w(TAG, "NSD unregistration failed: $errorCode")
            }
        }
        try {
            manager.registerService(info, NsdManager.PROTOCOL_DNS_SD, listener)
            registration = listener
        } catch (e: RuntimeException) {
            Log.w(TAG, "NSD register", e)
        }
    }

    private suspend fun stopAnnouncing() {
        quiet = false
        announceJob?.cancel()
        announceJob = null
        bleJob?.cancel()
        bleJob = null
        ble.stop()
        registration?.let { listener -> runCatching { nsd?.unregisterService(listener) } }
        registration = null
        if (network?.mode == ShareNetworkMode.WIFI_DIRECT) {
            p2p.attempt("clearLocalServices") { m, c, l -> m.clearLocalServices(c, l) }
        }
    }

    private suspend fun stopNow() {
        val wasP2p = network?.mode == ShareNetworkMode.WIFI_DIRECT || ownsGroup
        stopAnnouncing()
        if (wasP2p) {
            p2p.attempt("stopPeerDiscovery") { m, c, l -> m.stopPeerDiscovery(c, l) }
            if (ownsGroup) p2p.removeGroup()
            p2p.close()
        }
        ownsGroup = false
        network = null
    }

    companion object {
        fun newGroupName() = "DIRECT-hf-" + randomString(4, ALNUM)

        fun newGroupPass() = randomString(10, PASSPHRASE)

        /** Android wants "DIRECT-" and two characters first; at most 32 bytes. */
        fun validGroupName(name: String) =
            name.startsWith("DIRECT-") && name.length >= 9 && name.toByteArray(Charsets.UTF_8).size <= 32

        private const val GROUP_TIMEOUT_MS = 15_000L
        private const val DISCOVERABLE_EVERY_MS = 20_000L
        private const val GROUP_OWNER_IP = "192.168.49.1"
        private const val ALNUM = "abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789"
        private const val PASSPHRASE = "abcdefghijkmnpqrstuvwxyz23456789"
    }
}

/** The guest's side: finds hubs nearby and connects to their network. */
@SuppressLint("MissingPermission")
class ShareFinder(private val context: Context) {
    private val _hubs = MutableStateFlow<List<HubAddress>>(emptyList())

    /** Hubs found nearby (both kinds), newest first; empty while not searching. */
    val hubs: StateFlow<List<HubAddress>> = _hubs.asStateFlow()

    private val app = context.applicationContext
    private val p2p by lazy { P2p(app) }
    private val nsd: NsdManager? by lazy { app.getSystemService(Context.NSD_SERVICE) as NsdManager? }
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

    private class Found(
        var hub: HubAddress,
        val firstSeen: Long,
        var lastSeen: Long,
        var deviceAddress: String?,
        var nsdName: String?,
    )

    /** A Wi-Fi Direct device answering for `_homefi._tcp`: PTR and TXT arrive separately. */
    private class Peer(var instance: String? = null, var txt: Map<String, String>? = null)

    private val found = LinkedHashMap<String, Found>()
    private val peers = HashMap<String, Peer>()

    /** Device addresses learnt by discovery, kept after [stop] for the API < 29 join. */
    private val deviceByName = HashMap<String, String>()
    private val deviceByNetwork = HashMap<String, String>()

    /** Device addresses seen by Wi-Fi Direct discovery (an address only known by Bluetooth is not). */
    private val p2pSeen = HashSet<String>()

    private val ble by lazy { BleScanner(app, scope, ::onBle) }

    /** Explicit Bluetooth choice from [start]; null follows the `share.bluetooth` setting. */
    private var bluetoothChoice: Boolean? = null

    private var searching = false
    private var connecting = false
    private var joined = false
    private var loop: Job? = null
    private var discovery: NsdManager.DiscoveryListener? = null

    private class Pending(val info: NsdServiceInfo, val tries: Int)

    private val resolveQueue = ArrayDeque<Pending>()
    private var resolving = false

    /**
     * Starts looking (needs the share permissions); safe to call twice. [bluetooth] also
     * scans over Bluetooth LE; null follows the "Bluetooth" share setting (checked on every round).
     */
    fun start(bluetooth: Boolean? = null) {
        scope.launch {
            bluetoothChoice = bluetooth
            if (searching) return@launch
            searching = true
            startNsd()
            loop = scope.launch {
                while (isActive) {
                    if (!connecting && bluetoothWanted()) ble.ensure(p2p.locationEnabled()) else ble.stop()
                    if (!connecting && p2p.ready()) {
                        listen()
                        discoverP2p()
                    }
                    if (!connecting) askGateway()
                    prune()
                    publish()
                    delay(DISCOVERY_EVERY_MS)
                }
            }
        }
    }

    fun stop() {
        scope.launch {
            if (!searching) return@launch
            searching = false
            loop?.cancel()
            loop = null
            stopNsd()
            ble.stop()
            found.clear()
            peers.clear()
            _hubs.value = emptyList()
            if (!connecting && p2p.supported) {
                p2p.attempt("clearServiceRequests") { m, c, l -> m.clearServiceRequests(c, l) }
                p2p.attempt("stopPeerDiscovery") { m, c, l -> m.stopPeerDiscovery(c, l) }
            }
        }
    }

    /** Joins the hub's network if needed and returns the host to connect to. Throws [ShareNetworkException]. */
    suspend fun connect(hub: HubAddress): String = withContext(Dispatchers.Main.immediate) {
        when (hub.mode) {
            ShareNetworkMode.LAN -> hub.host
                ?: throw ShareNetworkException(app.getString(R.string.share_net_join_failed, hub.sessionName))
            ShareNetworkMode.WIFI_DIRECT -> joinGroup(hub)
        }
    }

    /** Leaves a Wi-Fi Direct group joined by [connect]. */
    suspend fun leave() {
        withContext(NonCancellable + Dispatchers.Main.immediate) {
            if (!p2p.supported) return@withContext
            val info = p2p.connectionInfo()
            if (joined || (info?.groupFormed == true && !info.isGroupOwner)) p2p.removeGroup()
            joined = false
        }
    }

    // --- Wi-Fi Direct ---------------------------------------------------------------------

    private suspend fun joinGroup(hub: HubAddress): String {
        val failed = app.getString(R.string.share_net_join_failed, hub.sessionName)

        // Already in that group (a second connect, or the app came back)?
        p2p.connectionInfo()?.let { info ->
            val owner = info.groupOwnerAddress?.hostAddress
            if (info.groupFormed && !info.isGroupOwner && owner != null) {
                val group = p2p.groupInfo()
                if (hub.networkName == null || group?.networkName == hub.networkName) {
                    joined = true
                    return owner
                }
            }
        }

        p2p.ensureReady(scanning = !(Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q && hub.networkName != null && hub.passphrase != null))
        connecting = true
        ble.stop() // the radio is busy enough; the search loop scans again afterwards
        try {
            p2p.attempt("stopPeerDiscovery") { m, c, l -> m.stopPeerDiscovery(c, l) }
            if (p2p.connectionInfo()?.groupFormed == true) {
                p2p.removeGroup()
                p2p.awaitConnection(3_000) { !it.groupFormed }
            }
            val config = configFor(hub)
                ?: throw ShareNetworkException(app.getString(R.string.share_net_not_found, hub.sessionName))
            try {
                p2p.call { m, c, l -> m.connect(c, config, l) }
            } catch (e: P2pFailure) {
                if (e.reason != WifiP2pManager.BUSY) throw e
                p2p.attempt("cancelConnect") { m, c, l -> m.cancelConnect(c, l) }
                p2p.removeGroup()
                delay(500)
                p2p.call { m, c, l -> m.connect(c, config, l) }
            }
            val info = p2p.awaitConnection(CONNECT_TIMEOUT_MS) {
                it.groupFormed && !it.isGroupOwner && it.groupOwnerAddress != null
            }
            val owner = info?.groupOwnerAddress?.hostAddress
            if (owner == null) {
                p2p.attempt("cancelConnect") { m, c, l -> m.cancelConnect(c, l) }
                throw ShareNetworkException(failed)
            }
            joined = true
            return owner
        } catch (e: P2pFailure) {
            throw ShareNetworkException(p2p.message(e, failed), e)
        } catch (e: CancellationException) {
            withContext(NonCancellable) { p2p.attempt("cancelConnect") { m, c, l -> m.cancelConnect(c, l) } }
            throw e
        } finally {
            connecting = false
        }
    }

    /**
     * API 29+ with the group's credentials: join it directly (no invitation on the hub, the
     * guest keeps its mobile data). Otherwise connect to the hub's device found by discovery.
     */
    private suspend fun configFor(hub: HubAddress): WifiP2pConfig? {
        val net = hub.networkName
        val pass = hub.passphrase
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q && net != null && pass != null) {
            runCatching {
                WifiP2pConfig.Builder().setNetworkName(net).setPassphrase(pass).build()
            }.onFailure { Log.w(TAG, "group credentials rejected", it) }.getOrNull()?.let { return it }
        }
        val address = deviceFor(hub) ?: lookFor(hub) ?: return null
        if (address.lowercase() !in p2pSeen) warmUp(address)
        @Suppress("DEPRECATION")
        return WifiP2pConfig().apply {
            deviceAddress = address
            wps.setup = WpsInfo.PBC
            groupOwnerIntent = 0
        }
    }

    private fun deviceFor(hub: HubAddress): String? =
        hub.networkName?.let(deviceByNetwork::get) ?: deviceByName[hub.sessionName]

    /**
     * The address came by Bluetooth: let Wi-Fi Direct see the hub (a group owner answers
     * peer discovery) before connecting, as the supplicant may refuse an unknown peer.
     */
    private suspend fun warmUp(address: String) {
        if (!p2p.attempt("discoverPeers") { m, c, l -> m.discoverPeers(c, l) }) return
        withTimeoutOrNull(PEER_WAIT_MS) {
            while (address.lowercase() !in p2p.peerAddresses()) delay(500)
        } ?: Log.d(TAG, "hub $address not in the peer list, connecting anyway")
    }

    /** Runs discovery for a while until the hub's device shows up (QR code on API < 29). */
    private suspend fun lookFor(hub: HubAddress): String? = withTimeoutOrNull(LOOK_FOR_TIMEOUT_MS) {
        var address: String? = null
        while (address == null) {
            listen()
            discoverP2p()
            var waited = 0L
            while (address == null && waited < 5_000) {
                delay(500)
                waited += 500
                address = deviceFor(hub)
            }
        }
        address
    }

    private fun listen() {
        val manager = p2p.manager ?: return
        val channel = runCatching { p2p.channel() }.getOrNull() ?: return
        try {
            manager.setDnsSdResponseListeners(
                channel,
                { instanceName, registrationType, device ->
                    if (registrationType.contains(SERVICE_TYPE, ignoreCase = true)) {
                        peers.getOrPut(device.deviceAddress) { Peer() }.instance = instanceName
                        onPeer(device.deviceAddress)
                    }
                },
                { fullDomainName, txt, device ->
                    if (fullDomainName.contains(SERVICE_TYPE, ignoreCase = true)) {
                        peers.getOrPut(device.deviceAddress) { Peer() }.txt = txt
                        onPeer(device.deviceAddress)
                    }
                },
            )
        } catch (e: RuntimeException) {
            Log.w(TAG, "setDnsSdResponseListeners", e)
        }
    }

    /** Wi-Fi Direct discovery times out after a while: restarted every [DISCOVERY_EVERY_MS]. */
    private suspend fun discoverP2p() {
        p2p.attempt("clearServiceRequests") { m, c, l -> m.clearServiceRequests(c, l) }
        // All Bonjour services: a request for one type only returns PTR records, without the TXT.
        val request = WifiP2pDnsSdServiceRequest.newInstance()
        if (!p2p.attempt("addServiceRequest") { m, c, l -> m.addServiceRequest(c, request, l) }) return
        p2p.attempt("discoverServices") { m, c, l -> m.discoverServices(c, l) }
    }

    private fun onPeer(address: String) {
        val peer = peers[address] ?: return
        val txt = peer.txt
        val name = txt?.get("name")?.takeIf { it.isNotBlank() } ?: peer.instance ?: return
        val hub = HubAddress(
            sessionName = name,
            host = null,
            port = txt?.get("port")?.toIntOrNull() ?: DEFAULT_PORT,
            mode = ShareNetworkMode.WIFI_DIRECT,
            networkName = txt?.get("net")?.takeIf { it.isNotBlank() },
            passphrase = txt?.get("pass")?.takeIf { it.isNotBlank() },
        )
        deviceByName[name] = address
        hub.networkName?.let { deviceByNetwork[it] = address }
        p2pSeen += address.lowercase()
        // The PTR name may be a shortened version of the TXT one: one entry per device.
        found.entries.removeAll { (key, f) -> f.deviceAddress == address && key != keyOf(hub) }
        put(hub, deviceAddress = address, nsdName = null)
    }

    // --- Bluetooth LE ----------------------------------------------------------------------

    private fun bluetoothWanted(): Boolean = bluetoothChoice
        ?: runCatching { (app as? HomefiApplication)?.settings?.current?.share?.bluetooth }.getOrNull()
        ?: true

    /** A hub's record read over Bluetooth: same entries (and keys) as the other channels. */
    private fun onBle(record: BleRecord) {
        if (!searching) return
        if (record.mode == ShareNetworkMode.WIFI_DIRECT) {
            record.deviceAddress?.let { device ->
                deviceByName[record.name] = device
                record.networkName?.let { deviceByNetwork[it] = device }
            }
            put(
                HubAddress(
                    sessionName = record.name,
                    host = null,
                    port = record.port,
                    mode = ShareNetworkMode.WIFI_DIRECT,
                    networkName = record.networkName,
                    passphrase = record.passphrase,
                ),
                deviceAddress = record.deviceAddress,
                nsdName = null,
            )
        }
        // Reachable on the home Wi-Fi only when this phone is on the same network.
        val lan = record.lanHost?.takeIf { it !in Lan.localAddresses() && Lan.sameNetwork(it) } ?: return
        put(
            HubAddress(record.name, lan, record.port, ShareNetworkMode.LAN, networkName = null, passphrase = null),
            deviceAddress = null,
            nsdName = null,
        )
    }

    // --- LAN (NSD) ------------------------------------------------------------------------

    private fun startNsd() {
        val manager = nsd ?: return
        val listener = object : NsdManager.DiscoveryListener {
            override fun onStartDiscoveryFailed(serviceType: String, errorCode: Int) {
                Log.w(TAG, "NSD discovery failed: $errorCode")
                val self = this
                scope.launch { if (discovery === self) discovery = null }
            }

            override fun onStopDiscoveryFailed(serviceType: String, errorCode: Int) {
                Log.d(TAG, "NSD stop failed: $errorCode")
            }

            override fun onDiscoveryStarted(serviceType: String) {}

            override fun onDiscoveryStopped(serviceType: String) {}

            override fun onServiceFound(info: NsdServiceInfo) {
                scope.launch { enqueueResolve(Pending(info, 0)) }
            }

            override fun onServiceLost(info: NsdServiceInfo) {
                scope.launch {
                    if (found.entries.removeAll { it.value.nsdName == info.serviceName }) publish()
                }
            }
        }
        try {
            manager.discoverServices(SERVICE_TYPE, NsdManager.PROTOCOL_DNS_SD, listener)
            discovery = listener
        } catch (e: RuntimeException) {
            Log.w(TAG, "NSD discover", e)
        }
    }

    private fun stopNsd() {
        discovery?.let { listener -> runCatching { nsd?.stopServiceDiscovery(listener) } }
        discovery = null
        resolveQueue.clear()
    }

    private fun enqueueResolve(pending: Pending) {
        if (!searching) return
        if (resolveQueue.none { it.info.serviceName == pending.info.serviceName }) resolveQueue.addLast(pending)
        resolveNext()
    }

    /** One resolve at a time: older NsdManager versions fail a second concurrent one. */
    @Suppress("DEPRECATION")
    private fun resolveNext() {
        if (resolving || !searching) return
        val manager = nsd ?: return
        val pending = resolveQueue.removeFirstOrNull() ?: return
        resolving = true
        val listener = object : NsdManager.ResolveListener {
            override fun onResolveFailed(info: NsdServiceInfo, errorCode: Int) {
                scope.launch {
                    resolving = false
                    if (pending.tries < 3) {
                        delay(500L * (pending.tries + 1))
                        enqueueResolve(Pending(pending.info, pending.tries + 1))
                    } else {
                        resolveNext()
                    }
                }
            }

            override fun onServiceResolved(info: NsdServiceInfo) {
                scope.launch {
                    resolving = false
                    onResolved(info)
                    resolveNext()
                }
            }
        }
        try {
            manager.resolveService(pending.info, listener)
        } catch (e: RuntimeException) {
            Log.w(TAG, "NSD resolve", e)
            resolving = false
            scope.launch { resolveNext() }
        }
    }

    @Suppress("DEPRECATION")
    private fun onResolved(info: NsdServiceInfo) {
        if (!searching) return
        val addresses = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            info.hostAddresses
        } else {
            listOfNotNull(info.host)
        }
        val address = addresses.firstOrNull { it is Inet4Address } ?: addresses.firstOrNull() ?: return
        val host = address.hostAddress?.substringBefore('%') ?: return
        if (host in Lan.localAddresses()) return // our own hub
        val attributes = info.attributes.orEmpty()
        val name = attributes["name"]?.let { String(it, Charsets.UTF_8) }?.takeIf { it.isNotBlank() } ?: info.serviceName
        val port = info.port.takeIf { it > 0 }
            ?: attributes["port"]?.let { String(it, Charsets.UTF_8).toIntOrNull() }
            ?: DEFAULT_PORT
        put(
            HubAddress(name, host, port, ShareNetworkMode.LAN, networkName = null, passphrase = null),
            deviceAddress = null,
            nsdName = info.serviceName,
        )
    }

    // --- list ---------------------------------------------------------------------------

    /**
     * Joined to the hub's group or hotspot from the system Wi-Fi settings (no camera, no
     * location needed): the hub is our gateway, so ask it directly.
     */
    private suspend fun askGateway() {
        val gateway = Gateway.address(app) ?: return
        if (gateway in Lan.localAddresses()) return
        val hub = Gateway.hello(gateway, DEFAULT_PORT) ?: return
        put(hub, deviceAddress = null, nsdName = null)
    }

    private fun keyOf(hub: HubAddress) = "${hub.mode}:${hub.sessionName}"

    private fun put(hub: HubAddress, deviceAddress: String?, nsdName: String?) {
        if (!searching) return
        val now = SystemClock.elapsedRealtime()
        val key = keyOf(hub)
        val existing = found[key]
        found[key] = if (existing != null) {
            existing.hub = richer(existing.hub, hub)
            existing.lastSeen = now
            if (existing.deviceAddress == null) existing.deviceAddress = deviceAddress
            if (existing.nsdName == null) existing.nsdName = nsdName
            existing
        } else {
            Found(hub, now, now, deviceAddress, nsdName)
        }
        publish()
    }

    /** The same hub heard on two channels: the newest values, keeping what only the older one knew. */
    private fun richer(old: HubAddress, new: HubAddress) = new.copy(
        host = new.host ?: old.host,
        networkName = new.networkName ?: old.networkName,
        passphrase = new.passphrase ?: old.passphrase,
        code = new.code ?: old.code,
    )

    /**
     * Entries not heard for [STALE_MS] are gone: Wi-Fi Direct and Bluetooth ones (NSD
     * reports its own losses, so entries NSD knows stay until it does).
     */
    private fun prune() {
        val cutoff = SystemClock.elapsedRealtime() - STALE_MS
        found.entries.removeAll {
            (it.value.hub.mode == ShareNetworkMode.WIFI_DIRECT || it.value.nsdName == null) && it.value.lastSeen < cutoff
        }
    }

    private fun publish() {
        _hubs.value = if (searching) found.values.sortedByDescending { it.firstSeen }.map { it.hub } else emptyList()
    }

    private companion object {
        const val DISCOVERY_EVERY_MS = 15_000L
        const val STALE_MS = 60_000L
        const val CONNECT_TIMEOUT_MS = 25_000L
        const val LOOK_FOR_TIMEOUT_MS = 15_000L
        const val PEER_WAIT_MS = 8_000L
    }
}

// --- helpers ------------------------------------------------------------------------------

private val random = SecureRandom()

private fun randomString(length: Int, alphabet: String): String =
    buildString { repeat(length) { append(alphabet[random.nextInt(alphabet.length)]) } }

/** [text] cut (on code point boundaries) to at most [maxBytes] bytes of UTF-8. */
internal fun fitBytes(text: String, maxBytes: Int): String {
    if (text.toByteArray(Charsets.UTF_8).size <= maxBytes) return text
    val out = StringBuilder()
    var bytes = 0
    var i = 0
    while (i < text.length) {
        val cp = text.codePointAt(i)
        val chunk = String(Character.toChars(cp))
        val size = chunk.toByteArray(Charsets.UTF_8).size
        if (bytes + size > maxBytes) break
        out.append(chunk)
        bytes += size
        i += Character.charCount(cp)
    }
    return out.toString().trimEnd()
}

/** A DNS-SD instance name: one label (no dots), at most 63 bytes. */
private fun instanceName(sessionName: String): String =
    fitBytes(sessionName.replace('.', ' ').trim().ifEmpty { "HomeFi" }, 63)
