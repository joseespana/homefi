package com.perseusoft.homefi.share.net

import android.content.Context
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.net.wifi.WifiManager
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.Inet4Address
import java.net.URL

/**
 * Discovery that needs no permission, camera or location (players like the SR35): join the
 * hub's Wi-Fi Direct group or a phone's hotspot from the system Wi-Fi settings, and the hub
 * is this device's gateway. We just ask it whether it is a HomeFi hub (`/hello`).
 */
internal object Gateway {
    /** IPv4 gateway of the Wi-Fi this device is on, or null. */
    fun address(context: Context): String? {
        val cm = context.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager?
        if (cm != null) {
            @Suppress("DEPRECATION")
            val networks = runCatching { cm.allNetworks.toList() }.getOrDefault(emptyList())
            for (network in networks) {
                val caps = cm.getNetworkCapabilities(network) ?: continue
                if (!caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI)) continue
                val gateway = cm.getLinkProperties(network)?.routes
                    ?.firstOrNull { it.isDefaultRoute && it.gateway is Inet4Address }
                    ?.gateway?.hostAddress
                if (gateway != null && gateway != "0.0.0.0") return gateway
            }
        }
        @Suppress("DEPRECATION")
        val dhcp = (context.applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager?)?.dhcpInfo?.gateway ?: 0
        if (dhcp == 0) return null
        return listOf(dhcp and 0xff, dhcp shr 8 and 0xff, dhcp shr 16 and 0xff, dhcp shr 24 and 0xff).joinToString(".")
    }

    /** The hub at [host]:[port], or null when nothing (or not HomeFi) answers quickly. */
    suspend fun hello(host: String, port: Int): HubAddress? = withContext(Dispatchers.IO) {
        runCatching {
            val connection = URL("http://$host:$port/hello").openConnection() as HttpURLConnection
            try {
                connection.connectTimeout = TIMEOUT_MS
                connection.readTimeout = TIMEOUT_MS
                if (connection.responseCode != 200) return@runCatching null
                val json = JSONObject(connection.inputStream.bufferedReader().use { it.readText() })
                if (json.optString("app") != "homefi") return@runCatching null
                val name = json.optString("name").ifBlank { host }
                HubAddress(name, host, port, ShareNetworkMode.LAN, networkName = null, passphrase = null)
            } finally {
                connection.disconnect()
            }
        }.getOrNull()
    }

    private const val TIMEOUT_MS = 1_500
}
