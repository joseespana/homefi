package com.perseusoft.homefi.tv.train

import java.net.Inet4Address
import java.net.NetworkInterface

/** This device's LAN address (site-local IPv4, Wi-Fi or Ethernet), or null when it is on no network. */
fun lanAddress(): String? = runCatching {
    NetworkInterface.getNetworkInterfaces().toList()
        .filter { it.isUp && !it.isLoopback }
        .flatMap { it.inetAddresses.toList() }
        .firstOrNull { it is Inet4Address && it.isSiteLocalAddress }
        ?.hostAddress
}.getOrNull()
