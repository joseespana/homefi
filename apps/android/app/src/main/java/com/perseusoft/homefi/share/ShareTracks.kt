package com.perseusoft.homefi.share

import android.util.Base64
import androidx.media3.common.MediaItem
import com.perseusoft.homefi.core.ItemKind
import com.perseusoft.homefi.core.LibraryTrack
import com.perseusoft.homefi.core.SearchHit
import com.perseusoft.homefi.core.ShareBrowseKind
import com.perseusoft.homefi.core.ShareCollection
import com.perseusoft.homefi.core.ShareQueueItem
import com.perseusoft.homefi.core.ShareTrack
import com.perseusoft.homefi.player.fileMediaItem
import com.perseusoft.homefi.player.isExplicit
import com.perseusoft.homefi.player.isFilePath
import com.perseusoft.homefi.player.knownDurationMs
import com.perseusoft.homefi.player.queueId
import com.perseusoft.homefi.player.tidalId
import com.perseusoft.homefi.player.tidalMediaItem
import com.perseusoft.homefi.player.toMediaItem
import com.perseusoft.homefi.player.withShareInfo
import java.io.File

// Sources in the shared queue: `tidal:<track id>` (played with the hub's
// account) and `hub:<absolute path>` (a song of the hub's library).
internal const val TIDAL_SOURCE = "tidal:"
internal const val HUB_SOURCE = "hub:"

/** Library albums / artists in search results: `local:<base64 key>`. */
internal const val LOCAL_COLLECTION = "local:"

private const val B64 = Base64.URL_SAFE or Base64.NO_PADDING or Base64.NO_WRAP

/** URL-safe ids for paths and keys (cover URLs, local collections). */
internal fun encodeId(text: String): String = Base64.encodeToString(text.toByteArray(), B64)

internal fun decodeId(id: String): String? = runCatching { String(Base64.decode(id, B64)) }.getOrNull()

/** Hub path guests turn into a URL with `resolveUrl`; served by the hub's `cover` callback. */
internal fun localCoverUrl(path: String): String = "/cover/${encodeId(path)}"

fun LibraryTrack.toShareTrack(): ShareTrack = ShareTrack(
    source = HUB_SOURCE + path,
    title = title.ifBlank { File(path).nameWithoutExtension },
    artist = artist,
    album = album,
    durationMs = durationMs.takeIf { it > 0u },
    coverUrl = localCoverUrl(path),
    explicit = false,
)

fun SearchHit.toShareTrack(): ShareTrack = ShareTrack(
    source = TIDAL_SOURCE + id,
    title = title,
    artist = artist,
    album = album,
    durationMs = durationMs,
    coverUrl = coverUrl,
    explicit = explicit,
)

internal fun SearchHit.toCollection(): ShareCollection? {
    val kind = when (kind) {
        ItemKind.ALBUM -> ShareBrowseKind.ALBUM
        ItemKind.ARTIST -> ShareBrowseKind.ARTIST
        ItemKind.PLAYLIST -> ShareBrowseKind.PLAYLIST
        else -> return null
    }
    return ShareCollection(kind = kind, id = id, title = title, subtitle = subtitle, coverUrl = coverUrl)
}

/** The queue source of a player item. */
fun MediaItem.shareSource(): String = tidalId(mediaId)?.let { TIDAL_SOURCE + it } ?: (HUB_SOURCE + mediaId)

/** What guests see of a player item. */
fun MediaItem.toShareTrack(lookup: (String) -> LibraryTrack?, durationMs: Long? = null): ShareTrack {
    val meta = mediaMetadata
    val duration = (knownDurationMs ?: durationMs)?.takeIf { it > 0 }?.toULong()
    tidalId(mediaId)?.let { id ->
        return ShareTrack(
            source = TIDAL_SOURCE + id,
            title = meta.title?.toString().orEmpty(),
            artist = meta.artist?.toString().orEmpty(),
            album = meta.albumTitle?.toString().orEmpty(),
            durationMs = duration,
            coverUrl = meta.artworkUri?.toString(),
            explicit = isExplicit,
        )
    }
    val local = isFilePath(mediaId)
    if (local) lookup(mediaId)?.let { return it.toShareTrack() }
    return ShareTrack(
        source = HUB_SOURCE + mediaId,
        title = meta.title?.toString() ?: mediaId.substringAfterLast('/'),
        artist = meta.artist?.toString().orEmpty(),
        album = meta.albumTitle?.toString().orEmpty(),
        durationMs = duration,
        coverUrl = if (local) localCoverUrl(mediaId) else meta.artworkUri?.toString(),
        explicit = false,
    )
}

/** A shared-queue song as a player item (null when the hub can't play it). */
fun ShareQueueItem.toMediaItem(lookup: (String) -> LibraryTrack?): MediaItem? {
    val t = track
    val base = when {
        t.source.startsWith(TIDAL_SOURCE) -> tidalMediaItem(
            t.source.removePrefix(TIDAL_SOURCE),
            t.title,
            t.artist,
            t.album,
            t.coverUrl?.takeIf { it.startsWith("http") },
        )
        t.source.startsWith(HUB_SOURCE) -> {
            val path = t.source.removePrefix(HUB_SOURCE)
            lookup(path)?.toMediaItem() ?: path.takeIf { File(it).isFile }?.let(::fileMediaItem)
        }
        else -> null
    } ?: return null
    return base.withShareInfo(queueId.toLong(), t.durationMs?.toLong(), t.explicit)
}

/** Same shared-queue entry (keeps the buffered next song when the queue changes). */
internal fun sameEntry(a: MediaItem, b: MediaItem): Boolean = a.queueId == b.queueId && a.mediaId == b.mediaId
