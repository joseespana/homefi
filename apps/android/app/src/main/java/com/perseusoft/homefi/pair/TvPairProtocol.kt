package com.perseusoft.homefi.pair

import org.json.JSONObject
import java.net.URI

/** The pairing contract between the phone app and the TV app, in one place (both sides use it). */
object TvPairProtocol {
    /** The TV's pairing endpoint; the next free port is used when it is busy, and the QR carries the real one. */
    const val DEFAULT_PORT = 47_810
    const val PATH = "/pair"
    const val SCHEME = "homefi"
    const val HOST = "tv"
    const val MAX_BODY_BYTES = 4_096
    const val CODE_DIGITS = 6
}

/** `homefi://tv?host=<tv-ip>&port=<port>&code=<one-time code>`: what the TV's QR holds. */
data class TvPairLink(val host: String, val port: Int, val code: String) {
    fun toLink(): String = "${TvPairProtocol.SCHEME}://${TvPairProtocol.HOST}?host=$host&port=$port&code=$code"

    companion object {
        private const val MAX_PORT = 65_535
        private val HOST_CHARS = Regex("^[A-Za-z0-9.-]+$")
        private val CODE_CHARS = Regex("^[A-Za-z0-9]{4,16}$")

        /** The link in [text], or null when it is not a well-formed TV pairing link. */
        fun parse(text: String?): TvPairLink? {
            val uri = runCatching { URI(text?.trim().orEmpty()) }.getOrNull() ?: return null
            if (uri.scheme != TvPairProtocol.SCHEME || uri.authority != TvPairProtocol.HOST) return null
            val query = uri.rawQuery.orEmpty().split('&').mapNotNull {
                val key = it.substringBefore('=', "")
                if (key.isEmpty()) null else key to it.substringAfter('=', "")
            }.toMap()
            val host = query["host"]?.takeIf { HOST_CHARS.matches(it) } ?: return null
            val port = query["port"]?.toIntOrNull()?.takeIf { it in 1..MAX_PORT } ?: return null
            val code = query["code"]?.takeIf { CODE_CHARS.matches(it) } ?: return null
            return TvPairLink(host, port, code)
        }
    }
}

/**
 * What the phone tells the TV: a one-time [code] (first pairing) or the [token] the TV gave it
 * (later rides), where its ride is ([rideUrl], empty when no ride runs yet) and its [phoneName].
 */
data class PairRequest(val code: String?, val token: String?, val rideUrl: String, val phoneName: String) {
    fun toJson(): String = JSONObject()
        .put("code", code ?: "")
        .put("token", token ?: "")
        .put("rideUrl", rideUrl)
        .put("phoneName", phoneName)
        .toString()

    companion object {
        /** Lenient: null for anything that is not a JSON object with a code or a token. */
        fun parse(body: String): PairRequest? = runCatching {
            val json = JSONObject(body)
            val code = json.optString("code").takeIf { it.isNotBlank() }
            val token = json.optString("token").takeIf { it.isNotBlank() }
            if (code == null && token == null) return null
            PairRequest(code, token, json.optString("rideUrl").trim(), json.optString("phoneName").trim().ifEmpty { "Phone" })
        }.getOrNull()
    }
}

/** The TV's answer: its name and the [token] this phone sends next time instead of a code. */
data class PairReply(val tvName: String, val token: String) {
    fun toJson(): String = JSONObject().put("ok", true).put("tvName", tvName).put("token", token).toString()

    companion object {
        fun parse(body: String): PairReply? = runCatching {
            val json = JSONObject(body)
            if (!json.optBoolean("ok")) return null
            PairReply(json.optString("tvName"), json.optString("token"))
        }.getOrNull()
    }
}
