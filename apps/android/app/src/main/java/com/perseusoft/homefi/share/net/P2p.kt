package com.perseusoft.homefi.share.net

import android.annotation.SuppressLint
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.location.LocationManager
import android.net.wifi.WifiManager
import android.net.wifi.p2p.WifiP2pGroup
import android.net.wifi.p2p.WifiP2pInfo
import android.net.wifi.p2p.WifiP2pManager
import android.os.Build
import android.os.Looper
import android.provider.Settings
import android.util.Log
import androidx.core.content.ContextCompat
import com.perseusoft.homefi.R
import kotlinx.coroutines.channels.Channel as Signal
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withTimeoutOrNull
import kotlin.coroutines.resume

internal const val TAG = "HomeFiShareNet"
internal const val SERVICE_TYPE = "_homefi._tcp"

/** A Wi-Fi Direct call that failed with one of [WifiP2pManager]'s reason codes. */
internal class P2pFailure(val reason: Int) : Exception("Wi-Fi P2P failure $reason")

/**
 * Thin coroutine layer over [WifiP2pManager]. Every callback arrives on the main
 * looper, so callers keep their state on the main thread.
 */
@SuppressLint("MissingPermission")
internal class P2p(context: Context) {
    val app: Context = context.applicationContext
    val manager: WifiP2pManager? = app.getSystemService(Context.WIFI_P2P_SERVICE) as WifiP2pManager?
    private var channel: WifiP2pManager.Channel? = null

    val supported: Boolean
        get() = manager != null && app.packageManager.hasSystemFeature(PackageManager.FEATURE_WIFI_DIRECT)

    fun channel(): WifiP2pManager.Channel {
        channel?.let { return it }
        val m = manager ?: throw ShareNetworkException(app.getString(R.string.share_net_no_wifi_direct))
        val created = m.initialize(app, Looper.getMainLooper()) { channel = null }
            ?: throw ShareNetworkException(app.getString(R.string.share_net_no_wifi_direct))
        channel = created
        return created
    }

    fun close() {
        val c = channel ?: return
        channel = null
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O_MR1) runCatching { c.close() }
    }

    /**
     * Throws a readable [ShareNetworkException] when Wi-Fi Direct cannot be used right now.
     * Location services only matter for [scanning] (below API 33): creating a group, or
     * joining one by name and passphrase, works without them (players with no GPS, like the SR35).
     */
    fun ensureReady(scanning: Boolean = true) {
        if (!supported) throw ShareNetworkException(app.getString(R.string.share_net_no_wifi_direct))
        if (!SharePermissions.granted(app)) throw ShareNetworkException(app.getString(R.string.share_net_permission))
        if (!wifiEnabled()) throw ShareNetworkException(app.getString(R.string.share_net_wifi_off))
        if (scanning && Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU && !locationEnabled()) {
            throw ShareNetworkException(app.getString(R.string.share_net_location_off))
        }
    }

    /** True when the Wi-Fi Direct preconditions hold (no exception, for background discovery). */
    fun ready(): Boolean = runCatching { ensureReady() }.isSuccess

    fun wifiEnabled(): Boolean =
        (app.getSystemService(Context.WIFI_SERVICE) as WifiManager?)?.isWifiEnabled == true

    @Suppress("DEPRECATION")
    fun locationEnabled(): Boolean {
        val lm = app.getSystemService(Context.LOCATION_SERVICE) as LocationManager? ?: return false
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            lm.isLocationEnabled
        } else {
            Settings.Secure.getInt(app.contentResolver, Settings.Secure.LOCATION_MODE, Settings.Secure.LOCATION_MODE_OFF) !=
                Settings.Secure.LOCATION_MODE_OFF
        }
    }

    /** Runs an [WifiP2pManager.ActionListener] call; throws [P2pFailure] on failure. */
    suspend fun call(block: (WifiP2pManager, WifiP2pManager.Channel, WifiP2pManager.ActionListener) -> Unit) {
        val m = manager ?: throw P2pFailure(WifiP2pManager.P2P_UNSUPPORTED)
        val c = channel()
        val reason = suspendCancellableCoroutine { cont ->
            val listener = object : WifiP2pManager.ActionListener {
                override fun onSuccess() {
                    if (cont.isActive) cont.resume(-1)
                }

                override fun onFailure(reason: Int) {
                    if (cont.isActive) cont.resume(reason)
                }
            }
            try {
                block(m, c, listener)
            } catch (e: SecurityException) {
                Log.w(TAG, "Wi-Fi P2P permission", e)
                if (cont.isActive) cont.resume(PERMISSION)
            }
        }
        if (reason == PERMISSION) throw ShareNetworkException(app.getString(R.string.share_net_permission))
        if (reason >= 0) throw P2pFailure(reason)
    }

    /** Like [call] but only reports whether it worked (for best-effort cleanup). */
    suspend fun attempt(
        what: String,
        block: (WifiP2pManager, WifiP2pManager.Channel, WifiP2pManager.ActionListener) -> Unit,
    ): Boolean = try {
        call(block)
        true
    } catch (e: P2pFailure) {
        Log.d(TAG, "$what failed: ${e.reason}")
        false
    } catch (e: ShareNetworkException) {
        Log.d(TAG, "$what failed: ${e.message}")
        false
    }

    suspend fun connectionInfo(): WifiP2pInfo? {
        val m = manager ?: return null
        val c = runCatching { channel() }.getOrNull() ?: return null
        return suspendCancellableCoroutine { cont ->
            try {
                m.requestConnectionInfo(c) { info -> if (cont.isActive) cont.resume(info) }
            } catch (e: SecurityException) {
                if (cont.isActive) cont.resume(null)
            }
        }
    }

    suspend fun groupInfo(): WifiP2pGroup? {
        val m = manager ?: return null
        val c = runCatching { channel() }.getOrNull() ?: return null
        return suspendCancellableCoroutine { cont ->
            try {
                m.requestGroupInfo(c) { group -> if (cont.isActive) cont.resume(group) }
            } catch (e: SecurityException) {
                if (cont.isActive) cont.resume(null)
            }
        }
    }

    /**
     * This device's Wi-Fi Direct address (for guests below API 29, which connect by device).
     * As group owner it is the group's owner; API 29+ also asks requestDeviceInfo. Null when
     * unknown or anonymized (02:00:00:00:00:00).
     */
    suspend fun ownDeviceAddress(): String? {
        fun usable(address: String?) = address?.lowercase()?.takeIf { it.isNotBlank() && it != "02:00:00:00:00:00" }
        val group = groupInfo()
        if (group?.isGroupOwner == true) usable(group.owner?.deviceAddress)?.let { return it }
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) return null
        val m = manager ?: return null
        val c = runCatching { channel() }.getOrNull() ?: return null
        val device = suspendCancellableCoroutine { cont ->
            try {
                m.requestDeviceInfo(c) { device -> if (cont.isActive) cont.resume(device) }
            } catch (e: SecurityException) {
                if (cont.isActive) cont.resume(null)
            }
        }
        return usable(device?.deviceAddress)
    }

    /** Device addresses in the current peer list. */
    suspend fun peerAddresses(): Set<String> {
        val m = manager ?: return emptySet()
        val c = runCatching { channel() }.getOrNull() ?: return emptySet()
        return suspendCancellableCoroutine { cont ->
            try {
                m.requestPeers(c) { list ->
                    if (cont.isActive) cont.resume(list?.deviceList.orEmpty().map { it.deviceAddress.lowercase() }.toSet())
                }
            } catch (e: SecurityException) {
                if (cont.isActive) cont.resume(emptySet())
            }
        }
    }

    suspend fun removeGroup(): Boolean = attempt("removeGroup") { m, c, l -> m.removeGroup(c, l) }

    /**
     * Waits (up to [timeoutMs]) until the connection info satisfies [done]; re-checks on every
     * WIFI_P2P_CONNECTION_CHANGED broadcast and at least once a second.
     */
    suspend fun awaitConnection(timeoutMs: Long, done: (WifiP2pInfo) -> Boolean): WifiP2pInfo? {
        val signal = Signal<Unit>(Signal.CONFLATED)
        val receiver = object : BroadcastReceiver() {
            override fun onReceive(context: Context, intent: Intent) {
                signal.trySend(Unit)
            }
        }
        ContextCompat.registerReceiver(
            app, receiver, IntentFilter(WifiP2pManager.WIFI_P2P_CONNECTION_CHANGED_ACTION),
            ContextCompat.RECEIVER_NOT_EXPORTED,
        )
        try {
            return withTimeoutOrNull(timeoutMs) {
                var info = connectionInfo()
                while (info == null || !done(info)) {
                    withTimeoutOrNull(1_000) { signal.receive() }
                    info = connectionInfo()
                }
                info
            }
        } finally {
            runCatching { app.unregisterReceiver(receiver) }
            signal.close()
        }
    }

    /** A readable message for a failed call, [fallback] when nothing more specific applies. */
    fun message(e: P2pFailure, fallback: String): String = when {
        e.reason == WifiP2pManager.P2P_UNSUPPORTED -> app.getString(R.string.share_net_no_wifi_direct)
        !wifiEnabled() -> app.getString(R.string.share_net_wifi_off)
        Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU && !locationEnabled() ->
            app.getString(R.string.share_net_location_off)
        e.reason == WifiP2pManager.BUSY -> app.getString(R.string.share_net_busy)
        else -> fallback
    }

    private companion object {
        const val PERMISSION = -2
    }
}
