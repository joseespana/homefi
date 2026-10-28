package com.perseusoft.homefi.data

import android.content.Context
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.HomefiException
import java.io.EOFException
import java.net.ConnectException
import java.net.NoRouteToHostException
import java.net.SocketException
import java.net.SocketTimeoutException
import java.net.UnknownHostException
import javax.net.ssl.SSLException

/**
 * Errors as people read them: plain words in the phone's language, never a
 * class name, URL or code. Errors from the Rust core already come worded (see
 * `homefi_core::humane`); the phone's own network errors are worded here.
 */
object Humane {
    private lateinit var context: Context

    fun init(context: Context) {
        this.context = context.applicationContext
    }

    fun of(error: Throwable): String {
        if (!::context.isInitialized) return error.message ?: error.toString()
        val chain = generateSequence(error) { it.cause }.take(8).toList()
        for (e in chain) {
            when (e) {
                is HomefiException -> e.message?.takeIf { it.isNotBlank() }?.let { return it }
                is UnknownHostException, is ConnectException, is NoRouteToHostException ->
                    return context.getString(R.string.error_offline)
                is SocketTimeoutException -> return context.getString(R.string.error_timeout)
                is SSLException -> return context.getString(R.string.error_secure)
                is SocketException, is EOFException -> return context.getString(R.string.error_connection_lost)
                else -> Unit
            }
        }
        // our own messages are already worded; anything technical gets a plain one
        val text = error.message?.trim().orEmpty()
        val technical = text.isEmpty() || "://" in text || "Exception" in text || text.startsWith("java.") ||
            text.startsWith("kotlin.") || text.startsWith("android.")
        return if (technical) context.getString(R.string.error_generic) else text
    }
}

/** This error in plain words, for the screen. */
fun Throwable.userMessage(): String = Humane.of(this)
