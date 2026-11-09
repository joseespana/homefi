package com.perseusoft.homefi.data

import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.asSharedFlow

/** One-line messages for the root snackbar (saved, imported, a refused file in plain words). Thread-safe. */
class Notes {
    private val _flow = MutableSharedFlow<String>(extraBufferCapacity = BUFFER)

    val flow: SharedFlow<String> = _flow.asSharedFlow()

    fun post(text: String) {
        _flow.tryEmit(text)
    }

    private companion object {
        const val BUFFER = 8
    }
}
