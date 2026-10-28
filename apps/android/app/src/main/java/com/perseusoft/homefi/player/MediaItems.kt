package com.perseusoft.homefi.player

import android.net.Uri
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import com.perseusoft.homefi.core.LibraryTrack
import com.perseusoft.homefi.core.SearchHit
import java.io.File

/** Library files use their absolute path as media id. */
fun LibraryTrack.toMediaItem(): MediaItem {
    val uri = Uri.fromFile(File(path))
    return MediaItem.Builder()
        .setMediaId(path)
        .setUri(uri)
        .setRequestMetadata(MediaItem.RequestMetadata.Builder().setMediaUri(uri).build())
        .setMediaMetadata(
            MediaMetadata.Builder()
                .setTitle(title)
                .setArtist(artist.ifBlank { null })
                .setAlbumTitle(album.ifBlank { null })
                .setAlbumArtist(albumArtist.ifBlank { null })
                .setTrackNumber(trackNumber.toInt().takeIf { it > 0 })
                .setDiscNumber(discNumber.toInt().takeIf { it > 0 })
                .setGenre(genre.ifBlank { null })
                .setReleaseYear(year?.toInt())
                .setIsPlayable(true)
                .setIsBrowsable(false)
                .build()
        )
        .build()
}

/** Any URI, e.g. a `content://` file opened from a file manager. */
fun uriMediaItem(uri: Uri, title: String?): MediaItem =
    MediaItem.Builder()
        .setMediaId(uri.toString())
        .setUri(uri)
        .setRequestMetadata(MediaItem.RequestMetadata.Builder().setMediaUri(uri).build())
        .setMediaMetadata(MediaMetadata.Builder().setTitle(title).setIsPlayable(true).setIsBrowsable(false).build())
        .build()

/** A local file without library metadata; the media id stays the path so covers still load. */
fun fileMediaItem(path: String): MediaItem {
    val uri = Uri.fromFile(File(path))
    return MediaItem.Builder()
        .setMediaId(path)
        .setUri(uri)
        .setRequestMetadata(MediaItem.RequestMetadata.Builder().setMediaUri(uri).build())
        .setMediaMetadata(MediaMetadata.Builder().setTitle(File(path).nameWithoutExtension).setIsPlayable(true).setIsBrowsable(false).build())
        .build()
}

/**
 * A Tidal track: `tidal://track/<id>`. The service resolves it to a file in the
 * stream cache just before it plays (see [TidalStreams]).
 */
fun SearchHit.toTidalMediaItem(): MediaItem = tidalMediaItem(id, title, artist, album, coverUrl)

/** The Tidal track id of a `tidal:<id>` media id. */
fun tidalId(mediaId: String): String? = mediaId.removePrefix("tidal:").takeIf { mediaId.startsWith("tidal:") }

/** `true` when a media id is a local file path (library item). */
fun isFilePath(mediaId: String): Boolean = mediaId.startsWith("/")

private const val EXTRA_QUEUE_ID = "homefi.share.queue_id"
private const val EXTRA_DURATION = "homefi.share.duration_ms"
private const val EXTRA_EXPLICIT = "homefi.share.explicit"

/** The shared-queue id this item was queued with (shared control), if any. */
val MediaItem.queueId: Long?
    get() = mediaMetadata.extras?.takeIf { it.containsKey(EXTRA_QUEUE_ID) }?.getLong(EXTRA_QUEUE_ID)

/** Duration and explicit flag known before playing (items from the shared queue). */
val MediaItem.knownDurationMs: Long?
    get() = mediaMetadata.extras?.takeIf { it.containsKey(EXTRA_DURATION) }?.getLong(EXTRA_DURATION)

val MediaItem.isExplicit: Boolean
    get() = mediaMetadata.extras?.getBoolean(EXTRA_EXPLICIT) == true

/** A copy carrying the shared-queue id (kept in the metadata extras, which survive the session). */
fun MediaItem.withShareInfo(queueId: Long?, durationMs: Long? = knownDurationMs, explicit: Boolean = isExplicit): MediaItem {
    val extras = android.os.Bundle(mediaMetadata.extras ?: android.os.Bundle.EMPTY)
    if (queueId != null) extras.putLong(EXTRA_QUEUE_ID, queueId) else extras.remove(EXTRA_QUEUE_ID)
    if (durationMs != null) extras.putLong(EXTRA_DURATION, durationMs)
    extras.putBoolean(EXTRA_EXPLICIT, explicit)
    return buildUpon().setMediaMetadata(mediaMetadata.buildUpon().setExtras(extras).build()).build()
}
