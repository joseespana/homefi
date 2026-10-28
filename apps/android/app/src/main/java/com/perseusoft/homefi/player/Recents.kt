package com.perseusoft.homefi.player

import android.content.Context
import android.net.Uri
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import org.json.JSONArray
import org.json.JSONObject

/** A track that was played: a library file or a Tidal stream. */
data class Recent(
    val mediaId: String,
    val title: String,
    val artist: String,
    val album: String,
    val artworkUri: String?,
    val playedAt: Long,
)

/** The last tracks played, newest first, kept across restarts for the Home screen. */
class Recents(context: Context) {
    private val prefs = context.getSharedPreferences("recents", Context.MODE_PRIVATE)
    private val _items = MutableStateFlow(load())
    val items: StateFlow<List<Recent>> = _items.asStateFlow()

    fun add(item: MediaItem) {
        val id = item.mediaId.takeIf { it.isNotBlank() } ?: return
        val m = item.mediaMetadata
        val recent = Recent(
            mediaId = id,
            title = m.title?.toString() ?: id.substringAfterLast('/'),
            artist = m.artist?.toString().orEmpty(),
            album = m.albumTitle?.toString().orEmpty(),
            artworkUri = m.artworkUri?.toString(),
            playedAt = System.currentTimeMillis(),
        )
        val next = (listOf(recent) + _items.value.filter { it.mediaId != id }).take(MAX)
        _items.value = next
        save(next)
    }

    private fun load(): List<Recent> = runCatching {
        val array = JSONArray(prefs.getString("items", "[]"))
        (0 until array.length()).map { i ->
            val o = array.getJSONObject(i)
            Recent(
                o.getString("id"),
                o.optString("title"),
                o.optString("artist"),
                o.optString("album"),
                o.optString("art").takeIf { it.isNotEmpty() },
                o.optLong("at"),
            )
        }
    }.getOrDefault(emptyList())

    private fun save(items: List<Recent>) {
        val array = JSONArray()
        items.forEach { r ->
            array.put(
                JSONObject()
                    .put("id", r.mediaId)
                    .put("title", r.title)
                    .put("artist", r.artist)
                    .put("album", r.album)
                    .put("art", r.artworkUri ?: "")
                    .put("at", r.playedAt),
            )
        }
        prefs.edit().putString("items", array.toString()).apply()
    }

    companion object {
        private const val MAX = 60
    }
}

/** A Tidal stream item from its id and the metadata we know. */
fun tidalMediaItem(id: String, title: String, artist: String, album: String, artworkUri: String?): MediaItem {
    val uri = Uri.parse("tidal://track/$id")
    return MediaItem.Builder()
        .setMediaId("tidal:$id")
        .setUri(uri)
        .setRequestMetadata(MediaItem.RequestMetadata.Builder().setMediaUri(uri).build())
        .setMediaMetadata(
            MediaMetadata.Builder()
                .setTitle(title)
                .setArtist(artist.ifBlank { null })
                .setAlbumTitle(album.ifBlank { null })
                .setArtworkUri(artworkUri?.let(Uri::parse))
                .setIsPlayable(true)
                .setIsBrowsable(false)
                .build(),
        )
        .build()
}
