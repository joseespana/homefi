package com.perseusoft.homefi.data

import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.asSharedFlow

/** The button of a note ("Open"): what it says and what it does (on the main thread). */
data class NoteAction(val label: String, val run: () -> Unit)

/** One message for the root snackbar, with an optional action. */
data class Note(val text: String, val action: NoteAction? = null)

/** One-line messages for the root snackbar (saved, imported, a refused file in plain words). Thread-safe. */
class Notes {
    private val _flow = MutableSharedFlow<Note>(extraBufferCapacity = BUFFER)

    val flow: SharedFlow<Note> = _flow.asSharedFlow()

    fun post(text: String, action: NoteAction? = null) {
        _flow.tryEmit(Note(text, action))
    }

    private companion object {
        const val BUFFER = 8
    }
}
