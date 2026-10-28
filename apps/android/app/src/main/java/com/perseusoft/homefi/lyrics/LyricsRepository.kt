package com.perseusoft.homefi.lyrics

import android.net.Uri
import androidx.compose.runtime.Immutable
import com.perseusoft.homefi.core.HomefiEngine
import com.perseusoft.homefi.core.SongLyrics
import com.perseusoft.homefi.data.userMessage
import com.perseusoft.homefi.player.isFilePath
import com.perseusoft.homefi.player.tidalId
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** One line of a song; [timeMs] is when it starts (null when the lyrics aren't synced). */
@Immutable
data class Line(val timeMs: Long?, val text: String) {
    val blank: Boolean get() = text.isBlank()
}

/** A song's lyrics, ready for the player: line lookups by time are binary searches. */
@Immutable
class Lyrics(val lines: List<Line>, val synced: Boolean, val provider: String?) {
    private val times = LongArray(lines.size) { lines[it].timeMs ?: 0L }

    /** The last line that started at or before [timeMs]; -1 before the first one (or when not synced). */
    fun lineAt(timeMs: Long): Int {
        if (!synced || lines.isEmpty()) return -1
        var lo = 0
        var hi = times.size
        while (lo < hi) {
            val mid = (lo + hi) ushr 1
            if (times[mid] <= timeMs) lo = mid + 1 else hi = mid
        }
        return lo - 1
    }

    fun timeOf(index: Int): Long = times[index]

    /** The first line with words after [index] (-1 searches from the start); -1 when none. */
    fun nextSung(index: Int): Int {
        for (i in (index + 1).coerceAtLeast(0) until lines.size) if (!lines[i].blank) return i
        return -1
    }

    /** The last line with words before [index]; -1 when none. */
    fun previousSung(index: Int): Int {
        for (i in (index - 1).coerceAtMost(lines.lastIndex) downTo 0) if (!lines[i].blank) return i
        return -1
    }

    /**
     * When line [index] is done being sung: when the next line starts, but no later than
     * a guess from its length, so a line before a long instrumental doesn't crawl.
     */
    fun endOf(index: Int): Long {
        val start = times[index]
        val guess = (1_200L + lines[index].text.length * 110L).coerceIn(1_500L, 9_000L)
        val next = if (index + 1 < times.size) times[index + 1] else Long.MAX_VALUE
        return minOf(next, start + guess)
    }

    companion object {
        fun of(l: SongLyrics): Lyrics? {
            val lines = l.lines.map { Line(it.timeMs?.toLong(), it.text.trim()) }
            if (lines.all { it.blank }) return null
            return Lyrics(lines, l.synced && lines.all { it.timeMs != null }, l.provider?.takeIf { it.isNotBlank() })
        }
    }
}

sealed interface LyricsState {
    data object Loading : LyricsState
    /** The song has none (or there is no way to look: a file shared by a hub, a content URI). */
    data object None : LyricsState
    data class Failed(val message: String) : LyricsState
    data class Ready(val lyrics: Lyrics) : LyricsState
}

/**
 * Lyrics of what plays, by media id, cached in memory: library files read their `.lrc` or
 * lyrics tag (Rust `localLyrics`), Tidal streams ask Tidal. A guest gets the hub's song as
 * `tidal:<id>` when it comes from Tidal; anything else has none.
 */
class LyricsRepository(private val engine: HomefiEngine, private val scope: CoroutineScope) {
    private val _states = MutableStateFlow<Map<String, LyricsState>>(emptyMap())
    val states: StateFlow<Map<String, LyricsState>> = _states.asStateFlow()

    /** Looks the song up unless it is cached (or being looked up); failures are tried again. */
    fun request(mediaId: String) {
        var start = false
        _states.update { m ->
            val known = m[mediaId]
            if (known != null && known !is LyricsState.Failed) {
                start = false
                m
            } else {
                start = true
                put(m, mediaId, LyricsState.Loading)
            }
        }
        if (start) scope.launch { _states.update { put(it, mediaId, load(mediaId)) } }
    }

    fun retry(mediaId: String) {
        _states.update { it - mediaId }
        request(mediaId)
    }

    private suspend fun load(mediaId: String): LyricsState = try {
        val tidal = tidalId(mediaId)
        val path = when {
            isFilePath(mediaId) -> mediaId
            mediaId.startsWith("file://") -> Uri.parse(mediaId).path
            else -> null
        }
        val found = when {
            tidal != null -> engine.trackLyrics(tidal)
            // the file's own lyrics, else Tidal's for the same recording (by its ISRC)
            path != null -> engine.fileLyrics(path)
            else -> null
        }
        found?.let(Lyrics::of)?.let { LyricsState.Ready(it) } ?: LyricsState.None
    } catch (e: CancellationException) {
        throw e
    } catch (e: Exception) {
        LyricsState.Failed(e.userMessage())
    }

    private fun put(m: Map<String, LyricsState>, key: String, value: LyricsState): Map<String, LyricsState> {
        val next = LinkedHashMap(m)
        next.remove(key)
        next[key] = value
        while (next.size > MAX_SONGS) next.remove(next.keys.first())
        return next
    }

    private companion object {
        const val MAX_SONGS = 60
    }
}
