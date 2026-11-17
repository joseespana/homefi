package com.perseusoft.homefi.player.exercise

import com.perseusoft.homefi.core.RideTvEndpoint

private const val MAX_PORT = 65_535
private const val ENDPOINT_VERSION: UByte = 1u
private val HOST_CHARS = Regex("^[A-Za-z0-9.-]+$")
/** The ride key's shape is the core's to check; here only "letters and digits", so a typo is caught early. */
private val KEY_CHARS = Regex("^[A-Za-z0-9]{1,32}$")

/**
 * A ride address: the whole "http://192.168.1.20:47801/K7M2QXPA4B/" the phone sends a paired TV, or typed by
 * hand on the TV (when the router blocks mDNS) as the phone shows it, "192.168.1.20:47801/K7M2QXPA4B". The last
 * part is the ride key: only with it does the ride's music play on this TV (the core's private door); without it
 * ("192.168.1.20:47801", or "192.168.1.20" on the default port) the ride shows, the music stays on the phone.
 * Null when it is not an address.
 */
fun parseRideAddress(text: String, defaultPort: Int): RideTvEndpoint? {
    val rest = text.trim().removePrefix("http://").removePrefix("https://")
    val bare = rest.substringBefore('/').trim()
    if (bare.isEmpty()) return null
    val host = bare.substringBefore(':')
    val portText = bare.substringAfter(':', "")
    val port = if (portText.isEmpty()) defaultPort else portText.toIntOrNull() ?: return null
    if (!HOST_CHARS.matches(host) || port !in 1..MAX_PORT) return null
    val key = rest.substringAfter('/', "").substringBefore('/').trim()
    if (key.isNotEmpty() && !KEY_CHARS.matches(key)) return null
    val path = if (key.isEmpty()) "/" else "/${key.uppercase()}/"
    return RideTvEndpoint(name = host, host = host, port = port.toUShort(), version = ENDPOINT_VERSION, rideUrl = "http://$host:$port${path}ride.json")
}

/** The phone's ride address as the user types it on the TV: no scheme, no trailing slash. */
fun String.asTypedAddress(): String = removePrefix("http://").removePrefix("https://").trimEnd('/')
