package com.perseusoft.homefi.share.net

import android.content.Context
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import java.net.Inet4Address
import java.net.InetAddress
import java.net.NetworkInterface

/** This device's addresses on the local network. */
internal object Lan {
    /**
     * IPv4 address on the Wi-Fi (or Ethernet) network this device is on, or on its own
     * hotspot when it is the access point; null when it is on neither.
     */
    fun wifiAddress(context: Context): String? {
        val cm = context.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager?
        if (cm != null) {
            @Suppress("DEPRECATION")
            val networks = runCatching { cm.allNetworks.toList() }.getOrDefault(emptyList())
            for (network in networks) {
                val caps = cm.getNetworkCapabilities(network) ?: continue
                if (!caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) &&
                    !caps.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET)
                ) continue
                val address = cm.getLinkProperties(network)?.linkAddresses
                    ?.map { it.address }
                    ?.firstOrNull(::usable)
                if (address != null) return address.hostAddress
            }
        }
        // This phone's own hotspot is not a ConnectivityManager network: look at the interfaces.
        return interfaces()
            .filter { iface -> HOTSPOT_PREFIXES.any { iface.name.startsWith(it) } }
            .flatMap { it.inetAddresses.toList() }
            .firstOrNull(::usable)
            ?.hostAddress
    }

    /** Every address of this device (to skip our own hub when browsing the LAN). */
    fun localAddresses(): Set<String> =
        interfaces().flatMap { it.inetAddresses.toList() }.mapNotNull { it.hostAddress?.substringBefore('%') }.toSet()

    /** Whether [host] (IPv4) is on the same subnet as one of this device's interfaces. */
    fun sameNetwork(host: String): Boolean {
        if (!host.matches(IPV4)) return false // a literal only: no DNS lookup on the main thread
        val target = runCatching { InetAddress.getByName(host) }.getOrNull() as? Inet4Address ?: return false
        return interfaces().filter { !it.isLoopback && !it.name.startsWith("p2p") }.any { iface ->
            iface.interfaceAddresses.any { link ->
                val mine = link.address as? Inet4Address ?: return@any false
                val bits = link.networkPrefixLength.toInt().coerceIn(1, 32)
                val mask = (-1L shl (32 - bits)).toInt()
                (toInt(mine) and mask) == (toInt(target) and mask)
            }
        }
    }

    private fun toInt(address: Inet4Address): Int =
        address.address.fold(0) { acc, b -> (acc shl 8) or (b.toInt() and 0xff) }

    private fun usable(address: InetAddress): Boolean =
        address is Inet4Address && !address.isLoopbackAddress && !address.isLinkLocalAddress

    private fun interfaces(): List<NetworkInterface> =
        runCatching { NetworkInterface.getNetworkInterfaces()?.toList().orEmpty().filter { it.isUp } }
            .getOrDefault(emptyList())

    private val IPV4 = Regex("""\d{1,3}(\.\d{1,3}){3}""")

    private val HOTSPOT_PREFIXES = listOf("wlan", "swlan", "ap", "softap")
}
