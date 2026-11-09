package com.perseusoft.homefi.tv.train

import com.perseusoft.homefi.pair.PairReply
import com.perseusoft.homefi.pair.PairRequest
import com.perseusoft.homefi.pair.TvPairProtocol
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import java.io.BufferedInputStream
import java.io.IOException
import java.net.ServerSocket
import java.net.Socket

/** How the TV answers a [PairRequest]: a reply, or null to refuse it (403). */
fun interface PairHandler {
    fun handle(request: PairRequest): PairReply?
}

/**
 * The tiny LAN endpoint a waiting TV runs: `POST /pair` with a JSON body, nothing else. One short
 * connection at a time per client, read timeouts, a body cap; the handler decides who may pair.
 * Plain sockets on purpose: a whole HTTP library for one route would be dead weight.
 */
class PairServer(private val handler: PairHandler) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private var socket: ServerSocket? = null
    private var accepting: Job? = null

    /** The port it listens on, or 0 when stopped. */
    val port: Int get() = socket?.localPort ?: 0

    /** Starts on [preferredPort], or any free port when it is busy. Returns the port in use. */
    @Synchronized
    fun start(preferredPort: Int): Int {
        if (socket != null) return port
        val server = runCatching { ServerSocket(preferredPort) }.getOrElse { ServerSocket(0) }
        socket = server
        accepting = scope.launch {
            while (isActive) {
                val client = try {
                    server.accept()
                } catch (_: IOException) {
                    return@launch // closed
                }
                launch { serve(client) }
            }
        }
        return server.localPort
    }

    @Synchronized
    fun stop() {
        accepting?.cancel()
        accepting = null
        runCatching { socket?.close() }
        socket = null
    }

    private fun serve(client: Socket) {
        client.use {
            runCatching {
                it.soTimeout = IO_TIMEOUT_MS
                val input = BufferedInputStream(it.getInputStream())
                val request = readRequest(input)
                val (status, body) = answer(request)
                val bytes = body.toByteArray()
                val head = "HTTP/1.1 $status\r\nContent-Type: application/json\r\nContent-Length: ${bytes.size}\r\nConnection: close\r\n\r\n"
                it.getOutputStream().apply { write(head.toByteArray()); write(bytes); flush() }
            }
        }
    }

    private class Raw(val line: String, val body: String)

    private fun readRequest(input: BufferedInputStream): Raw {
        val line = readLine(input)
        var length = 0
        while (true) {
            val header = readLine(input)
            if (header.isEmpty()) break
            if (header.startsWith("content-length:", ignoreCase = true)) length = header.substringAfter(':').trim().toIntOrNull() ?: 0
        }
        val size = length.coerceIn(0, TvPairProtocol.MAX_BODY_BYTES)
        val buffer = ByteArray(size)
        var read = 0
        while (read < size) {
            val n = input.read(buffer, read, size - read)
            if (n < 0) break
            read += n
        }
        return Raw(line, String(buffer, 0, read))
    }

    private fun readLine(input: BufferedInputStream): String {
        val line = StringBuilder()
        while (line.length < MAX_LINE) {
            val c = input.read()
            if (c < 0 || c == '\n'.code) break
            if (c != '\r'.code) line.append(c.toChar())
        }
        return line.toString()
    }

    private fun answer(raw: Raw): Pair<String, String> {
        val parts = raw.line.split(' ')
        if (parts.getOrNull(0) != "POST" || parts.getOrNull(1) != TvPairProtocol.PATH) return "404 Not Found" to """{"ok":false}"""
        val request = PairRequest.parse(raw.body) ?: return "400 Bad Request" to """{"ok":false}"""
        val reply = handler.handle(request) ?: return "403 Forbidden" to """{"ok":false}"""
        return "200 OK" to reply.toJson()
    }

    private companion object {
        const val IO_TIMEOUT_MS = 3_000
        const val MAX_LINE = 2_048
    }
}
