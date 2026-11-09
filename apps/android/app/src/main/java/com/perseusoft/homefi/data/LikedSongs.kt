package com.perseusoft.homefi.data

import android.content.Context
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * The songs the user hearted in the TV player, by media id (a file path or a TIDAL id), kept on the
 * device. The engine has no call that writes a TIDAL favourite yet, so this stays local.
 */
class LikedSongs(context: Context) {
    private val prefs = context.getSharedPreferences("liked_songs", Context.MODE_PRIVATE)
    private val _ids = MutableStateFlow<Set<String>>(prefs.getStringSet(KEY, emptySet()).orEmpty().toSet())
    val ids: StateFlow<Set<String>> = _ids.asStateFlow()

    fun toggle(mediaId: String) {
        val next = toggled(_ids.value, mediaId)
        _ids.value = next
        prefs.edit().putStringSet(KEY, next).apply()
    }

    companion object {
        private const val KEY = "ids"

        /** [ids] with [mediaId] added, or removed when it was there. */
        fun toggled(ids: Set<String>, mediaId: String): Set<String> = if (mediaId in ids) ids - mediaId else ids + mediaId
    }
}
