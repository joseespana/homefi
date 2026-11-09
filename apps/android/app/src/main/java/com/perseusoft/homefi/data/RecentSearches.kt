package com.perseusoft.homefi.data

import android.content.Context
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/** The last things searched for (newest first), kept on the device; the TV shows them as chips. */
class RecentSearches(context: Context) {
    private val prefs = context.getSharedPreferences(FILE, Context.MODE_PRIVATE)
    private val _items = MutableStateFlow(read())
    val items: StateFlow<List<String>> = _items.asStateFlow()

    fun add(query: String) {
        val next = remember(_items.value, query)
        if (next == _items.value) return
        _items.value = next
        prefs.edit().putString(KEY, next.joinToString(SEPARATOR)).apply()
    }

    fun clear() {
        _items.value = emptyList()
        prefs.edit().remove(KEY).apply()
    }

    private fun read(): List<String> =
        prefs.getString(KEY, null)?.split(SEPARATOR)?.filter(String::isNotBlank).orEmpty().take(MAX)

    companion object {
        private const val FILE = "recent_searches"
        private const val KEY = "items"
        private const val SEPARATOR = "\n"
        const val MAX = 8

        /** [query] first, an older copy of it gone (case-insensitive), at most [MAX] kept. */
        fun remember(list: List<String>, query: String): List<String> {
            val q = query.trim().replace(SEPARATOR, " ")
            if (q.length < 2) return list
            return (listOf(q) + list.filterNot { it.equals(q, ignoreCase = true) }).take(MAX)
        }
    }
}
