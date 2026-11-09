package com.perseusoft.homefi.player.exercise

import com.perseusoft.homefi.core.RideTvEndpoint

private const val MAX_PORT = 65_535
private const val ENDPOINT_VERSION: UByte = 1u
private val HOST_CHARS = Regex("^[A-Za-z0-9.-]+$")

/**
 * A ride typed by hand on the TV (when the router blocks mDNS): "192.168.1.20:47801", "192.168.1.20" (the
 * default port) or the whole "http://192.168.1.20:47801/" the phone shows. Null when it is not an address.
 */
fun parseRideAddress(text: String, defaultPort: Int): RideTvEndpoint? {
    val bare = text.trim().removePrefix("http://").removePrefix("https://").substringBefore('/').trim()
    if (bare.isEmpty()) return null
    val host = bare.substringBefore(':')
    val portText = bare.substringAfter(':', "")
    val port = if (portText.isEmpty()) defaultPort else portText.toIntOrNull() ?: return null
    if (!HOST_CHARS.matches(host) || port !in 1..MAX_PORT) return null
    return RideTvEndpoint(name = host, host = host, port = port.toUShort(), version = ENDPOINT_VERSION, rideUrl = "http://$host:$port/ride.json")
}

/** The phone's ride address as the user types it on the TV: no scheme, no trailing slash. */
fun String.asTypedAddress(): String = removePrefix("http://").removePrefix("https://").trimEnd('/')
