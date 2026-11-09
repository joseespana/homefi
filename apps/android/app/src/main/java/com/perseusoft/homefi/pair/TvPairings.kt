package com.perseusoft.homefi.pair

import android.content.Context
import com.perseusoft.homefi.HomefiApplication
import com.perseusoft.homefi.R
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.URL

/** The TV this phone is paired with: where its pairing endpoint listens and the token it gave us. */
data class PairedTv(val name: String, val host: String, val port: Int, val token: String)

enum class TvSendState { IDLE, SENDING, SENT, UNREACHABLE }

/**
 * The phone side of "Connect to TV" without typing: scans or opens the TV's `homefi://tv` link, tells the
 * TV where the ride is (`POST /pair`), remembers the TV, and sends every next ride by itself. Pure
 * client: nothing about the account or library leaves the phone. One TV is remembered (the last paired).
 */
class TvPairings(private val app: HomefiApplication) {
    private val prefs = app.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
    private val _paired = MutableStateFlow(read())
    val paired: StateFlow<PairedTv?> = _paired.asStateFlow()
    private val _sendState = MutableStateFlow(TvSendState.IDLE)
    val sendState: StateFlow<TvSendState> = _sendState.asStateFlow()

    /** A scanned or opened link: pairs, sends the running ride if there is one, and says how it went. */
    fun connect(link: TvPairLink) {
        app.appScope.launch {
            val rideUrl = app.ride.tv.value.url.orEmpty()
            when (val reply = post(link.host, link.port, PairRequest(link.code, null, rideUrl, app.share.displayName()))) {
                is Outcome.Replied -> {
                    save(PairedTv(reply.value.tvName, link.host, link.port, reply.value.token))
                    // pairing means "show my rides on that TV"
                    app.settings.exercise.value.let { if (!it.tvEnabled) app.settings.setExercise(it.copy(tvEnabled = true)) }
                    _sendState.value = if (rideUrl.isNotEmpty()) TvSendState.SENT else TvSendState.IDLE
                    val text = if (rideUrl.isNotEmpty()) R.string.tv_pair_done_sent else R.string.tv_pair_done
                    app.notes.post(app.getString(text, reply.value.tvName))
                }
                Outcome.Refused -> app.notes.post(app.getString(R.string.tv_pair_refused))
                Outcome.Unreachable -> app.notes.post(app.getString(R.string.tv_pair_unreachable))
            }
        }
    }

    /** A ride started (its page is at [rideUrl]): the paired TV opens it by itself. */
    fun sendRide(rideUrl: String) {
        val tv = _paired.value ?: return
        _sendState.value = TvSendState.SENDING
        app.appScope.launch {
            _sendState.value = when (val reply = post(tv.host, tv.port, PairRequest(null, tv.token, rideUrl, app.share.displayName()))) {
                is Outcome.Replied -> TvSendState.SENT
                Outcome.Refused -> {
                    forget() // the TV forgot this phone: pair again by scanning
                    TvSendState.IDLE
                }
                Outcome.Unreachable -> TvSendState.UNREACHABLE
            }
        }
    }

    /** "Send again" while a ride runs (the TV was off or on another network). */
    fun resend() {
        app.ride.tv.value.url?.let(::sendRide)
    }

    fun forget() {
        prefs.edit().clear().apply()
        _paired.value = null
        _sendState.value = TvSendState.IDLE
    }

    private sealed interface Outcome {
        data class Replied(val value: PairReply) : Outcome
        data object Refused : Outcome
        data object Unreachable : Outcome
    }

    private suspend fun post(host: String, port: Int, request: PairRequest): Outcome = withContext(Dispatchers.IO) {
        val connection = runCatching { URL("http://$host:$port${TvPairProtocol.PATH}").openConnection() as HttpURLConnection }
            .getOrNull() ?: return@withContext Outcome.Unreachable
        try {
            connection.connectTimeout = HTTP_TIMEOUT_MS
            connection.readTimeout = HTTP_TIMEOUT_MS
            connection.requestMethod = "POST"
            connection.doOutput = true
            connection.setRequestProperty("Content-Type", "application/json")
            connection.outputStream.use { it.write(request.toJson().toByteArray()) }
            when (connection.responseCode) {
                HttpURLConnection.HTTP_OK -> {
                    val reply = PairReply.parse(connection.inputStream.bufferedReader().use { it.readText() })
                    if (reply != null) Outcome.Replied(reply) else Outcome.Unreachable
                }
                HttpURLConnection.HTTP_FORBIDDEN, HttpURLConnection.HTTP_UNAUTHORIZED -> Outcome.Refused
                else -> Outcome.Unreachable
            }
        } catch (_: java.io.IOException) {
            Outcome.Unreachable
        } finally {
            connection.disconnect()
        }
    }

    private fun save(tv: PairedTv) {
        prefs.edit().putString(KEY, JSONObject().put("name", tv.name).put("host", tv.host).put("port", tv.port).put("token", tv.token).toString()).apply()
        _paired.value = tv
    }

    private fun read(): PairedTv? = runCatching {
        val json = JSONObject(prefs.getString(KEY, null) ?: return null)
        PairedTv(json.getString("name"), json.getString("host"), json.getInt("port"), json.getString("token"))
    }.getOrNull()

    private companion object {
        const val PREFS = "tv_pairing"
        const val KEY = "tv"
        const val HTTP_TIMEOUT_MS = 3_000
    }
}
