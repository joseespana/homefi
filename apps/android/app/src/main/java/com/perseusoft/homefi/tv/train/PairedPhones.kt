package com.perseusoft.homefi.tv.train

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject
import java.security.SecureRandom

/** A phone this TV trusts: its [token] replaces the one-time code, [rideUrl] is where its last ride was. */
data class PairedPhone(val name: String, val token: String, val rideUrl: String) {
    /** The phone's host ("192.168.1.20"), where its rides are found again. */
    val host: String get() = rideUrl.removePrefix("http://").substringBefore('/').substringBefore(':')
}

/** The phones paired with this TV (name, token, last address), kept in the TV app's own preferences. */
class PairedPhones(context: Context) {
    private val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
    private val random = SecureRandom()
    private var phones: List<PairedPhone> = read()

    @Synchronized
    fun all(): List<PairedPhone> = phones

    @Synchronized
    fun byToken(token: String): PairedPhone? = phones.firstOrNull { it.token == token }

    /** Pairs a phone by name (the same name replaces its old entry). */
    @Synchronized
    fun add(name: String, rideUrl: String): PairedPhone {
        val phone = PairedPhone(name, newToken(), rideUrl)
        save(phones.filter { it.name != name } + phone)
        return phone
    }

    /** The phone's ride moved (new IP or port): remember where it is now. */
    @Synchronized
    fun update(token: String, rideUrl: String) {
        if (rideUrl.isEmpty()) return
        save(phones.map { if (it.token == token) it.copy(rideUrl = rideUrl) else it })
    }

    @Synchronized
    fun clear() = save(emptyList())

    private fun newToken(): String = ByteArray(TOKEN_BYTES).also(random::nextBytes).joinToString("") { "%02x".format(it) }

    private fun save(list: List<PairedPhone>) {
        phones = list
        val json = JSONArray()
        list.forEach { json.put(JSONObject().put("name", it.name).put("token", it.token).put("rideUrl", it.rideUrl)) }
        prefs.edit().putString(KEY, json.toString()).apply()
    }

    private fun read(): List<PairedPhone> = runCatching {
        val json = JSONArray(prefs.getString(KEY, "[]"))
        (0 until json.length()).map { json.getJSONObject(it) }.map { PairedPhone(it.getString("name"), it.getString("token"), it.optString("rideUrl")) }
    }.getOrDefault(emptyList())

    private companion object {
        const val PREFS = "tv_paired_phones"
        const val KEY = "phones"
        const val TOKEN_BYTES = 16
    }
}
