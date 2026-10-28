package com.perseusoft.homefi.player

import android.content.ComponentName
import android.content.Context
import android.net.Uri
import androidx.core.content.ContextCompat
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.LibraryTrack
import com.perseusoft.homefi.core.ItemKind
import android.widget.Toast
import com.perseusoft.homefi.core.SearchHit
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.withContext
import com.perseusoft.homefi.core.readTrackInfo
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

data class QueueEntry(
    val mediaId: String,
    val title: String,
    val artist: String,
    /** Cover URL of a Tidal track; library files use their embedded art. */
    val artworkUri: String? = null,
    /** Id in the shared queue while sharing (see [queueId]). */
    val queueId: Long? = null,
)

data class NowPlaying(
    val mediaId: String,
    val title: String,
    val artist: String,
    val album: String,
    val artwork: ByteArray?,
    /** Cover URL of a Tidal stream. */
    val artworkUri: String? = null,
    /** `FLAC · 24-bit / 96 kHz`, `MP3 · 44.1 kHz`, ... */
    val quality: String?,
    /** Genre tag, if the file or stream has one (the mascot dances to it). */
    val genre: String? = null,
)

data class PlayerUiState(
    val current: NowPlaying? = null,
    val isPlaying: Boolean = false,
    val isBuffering: Boolean = false,
    val positionMs: Long = 0,
    val durationMs: Long = 0,
    val shuffle: Boolean = false,
    val repeatMode: Int = Player.REPEAT_MODE_OFF,
    val queue: List<QueueEntry> = emptyList(),
    val index: Int = -1,
    val error: String? = null,
)

/**
 * App-wide handle on the [PlaybackService] session. All calls are safe from
 * the main thread; commands issued before the controller connects are queued.
 */
class PlayerController(private val context: Context, private val trackLookup: (String) -> LibraryTrack?) {
    /** Recently played tracks, for the Home screen. */
    val recents = Recents(context)

    private val scope = MainScope()
    private val _state = MutableStateFlow(PlayerUiState())
    val state: StateFlow<PlayerUiState> = _state.asStateFlow()

    private var controller: MediaController? = null
    private var connecting = false
    private val pending = mutableListOf<(MediaController) -> Unit>()
    private var ticker: Job? = null
    private val extraListeners = mutableListOf<Player.Listener>()

    /**
     * While sharing, "play" / "play next" / "add to queue" go through the shared
     * queue instead: return `true` when handled. `startIndex` is null for enqueues.
     */
    var playOverride: ((items: List<MediaItem>, startIndex: Int?, next: Boolean) -> Boolean)? = null

    /** Called after the user jumped around the queue by hand. */
    var onQueueEdited: (() -> Unit)? = null

    /** The connected controller (null until [connect] finished). Main thread only. */
    val player: Player? get() = controller

    /** Extra listeners on the session's player (the share hub mirrors it). */
    fun addListener(listener: Player.Listener) {
        extraListeners += listener
        controller?.addListener(listener) ?: connect()
    }

    fun removeListener(listener: Player.Listener) {
        extraListeners -= listener
        controller?.removeListener(listener)
    }

    /** Binds to the service (restores the state of a running session). */
    fun connect() {
        if (controller != null || connecting) return
        connecting = true
        val token = SessionToken(context, ComponentName(context, PlaybackService::class.java))
        val future = MediaController.Builder(context, token).buildAsync()
        future.addListener({
            connecting = false
            val c = runCatching { future.get() }.getOrNull() ?: return@addListener
            controller = c
            c.addListener(listener)
            extraListeners.forEach(c::addListener)
            rebuildQueue(c)
            sync(c)
            pending.forEach { it(c) }
            pending.clear()
        }, ContextCompat.getMainExecutor(context))
    }

    private fun withController(block: (MediaController) -> Unit) {
        val c = controller
        if (c != null) block(c) else {
            pending += block
            connect()
        }
    }

    fun play(tracks: List<LibraryTrack>, startIndex: Int = 0, shuffle: Boolean = false) {
        if (tracks.isEmpty()) return
        playItems(tracks.map { it.toMediaItem() }, startIndex, shuffle)
    }

    fun playItems(items: List<MediaItem>, startIndex: Int, shuffle: Boolean) = withController { c ->
        if (items.isNotEmpty() && playOverride?.invoke(items, startIndex.coerceIn(0, items.lastIndex), false) == true) return@withController
        c.shuffleModeEnabled = shuffle
        val start = if (shuffle) (items.indices).random() else startIndex.coerceIn(0, items.lastIndex)
        c.setMediaItems(items, start, 0L)
        c.prepare()
        c.play()
    }

    /** Tidal tracks, streamed (see TidalStreams). */
    fun playTidal(tracks: List<SearchHit>, startIndex: Int = 0, shuffle: Boolean = false) {
        if (tracks.isEmpty()) return
        playItems(tracks.map { it.toTidalMediaItem() }, startIndex, shuffle)
    }

    /**
     * Adds to the queue without interrupting what plays: right after the
     * current track (`next`) or at the end. An empty queue starts playing.
     */
    fun enqueue(items: List<MediaItem>, next: Boolean) = withController { c ->
        if (items.isEmpty()) return@withController
        if (playOverride?.invoke(items, null, next) == true) {
            // the shared queue decides where they go
        } else if (c.mediaItemCount == 0) {
            c.setMediaItems(items)
            c.prepare()
            c.play()
        } else {
            c.addMediaItems(if (next) c.currentMediaItemIndex + 1 else c.mediaItemCount, items)
        }
        val what = if (items.size == 1) "“${items[0].mediaMetadata.title}”" else context.getString(R.string.queue_songs, items.size)
        Toast.makeText(context, context.getString(if (next) R.string.queue_added_next else R.string.queue_added, what), Toast.LENGTH_SHORT).show()
    }

    fun queueTracks(tracks: List<LibraryTrack>, next: Boolean) = enqueue(tracks.map { it.toMediaItem() }, next)

    fun queueTidal(tracks: List<SearchHit>, next: Boolean) =
        enqueue(tracks.filter { it.kind == ItemKind.TRACK }.map { it.toTidalMediaItem() }, next)

    /** `true` while something else (the shared queue) decides what plays. */
    val playsElsewhere: Boolean get() = playOverride != null

    /**
     * The rest of a Tidal list that started playing before it was fully loaded: added right
     * after its last loaded track ([lastId] at [lastIndex]) while the queue still holds it
     * there. Silently skipped once the user replaced the queue.
     */
    fun appendTidal(lastId: String, lastIndex: Int, rest: List<SearchHit>) = withController { c ->
        val items = rest.filter { it.kind == ItemKind.TRACK }.map { it.toTidalMediaItem() }
        if (items.isEmpty() || lastIndex !in 0 until c.mediaItemCount) return@withController
        if (c.getMediaItemAt(lastIndex).mediaId != "tidal:$lastId") return@withController
        c.addMediaItems(lastIndex + 1, items)
    }

    /** Plays recently played tracks again (library file, plain file or Tidal stream). */
    fun playRecents(items: List<Recent>, startIndex: Int) =
        playItems(items.map(::recentMediaItem), startIndex, false)

    private fun recentMediaItem(r: Recent): MediaItem {
        tidalId(r.mediaId)?.let { return tidalMediaItem(it, r.title, r.artist, r.album, r.artworkUri) }
        if (isFilePath(r.mediaId)) return trackLookup(r.mediaId)?.toMediaItem() ?: fileMediaItem(r.mediaId)
        return uriMediaItem(Uri.parse(r.mediaId), r.title)
    }

    fun playUri(uri: Uri, title: String?) = playItems(listOf(uriMediaItem(uri, title)), 0, false)

    /** Plays one local file, with library metadata when it was scanned already. */
    fun playFile(path: String) {
        scope.launch {
            val track = trackLookup(path) ?: withContext(Dispatchers.IO) { runCatching { readTrackInfo(path) }.getOrNull() }
            if (track != null) play(listOf(track)) else playItems(listOf(fileMediaItem(path)), 0, false)
        }
    }

    fun playNext(track: LibraryTrack) = withController { c ->
        if (playOverride?.invoke(listOf(track.toMediaItem()), null, true) == true) return@withController
        if (c.mediaItemCount == 0) play(listOf(track)) else c.addMediaItem(c.currentMediaItemIndex + 1, track.toMediaItem())
    }

    fun pause() = withController { it.pause() }

    fun setVolume(volume: Float) = withController { it.volume = volume.coerceIn(0f, 1f) }

    /** Songs after the current one, in play order (shuffle aside). */
    fun upcoming(): List<MediaItem> {
        val c = controller ?: return emptyList()
        if (c.mediaItemCount == 0) return emptyList()
        return (c.currentMediaItemIndex + 1 until c.mediaItemCount).map(c::getMediaItemAt)
    }

    /**
     * Makes the songs after the current one exactly [items], keeping the ones
     * already in place ([same]) so the next track stays buffered. An empty or
     * finished player starts playing them.
     */
    fun replaceUpcoming(items: List<MediaItem>, same: (MediaItem, MediaItem) -> Boolean) = withController { c ->
        if (c.mediaItemCount == 0) {
            if (items.isNotEmpty()) {
                c.setMediaItems(items)
                c.prepare()
                c.play()
            }
            return@withController
        }
        val from = c.currentMediaItemIndex + 1
        var keep = 0
        while (keep < items.size && from + keep < c.mediaItemCount && same(c.getMediaItemAt(from + keep), items[keep])) keep++
        if (from + keep < c.mediaItemCount) c.removeMediaItems(from + keep, c.mediaItemCount)
        if (keep < items.size) c.addMediaItems(items.subList(keep, items.size))
        if (c.playbackState == Player.STATE_ENDED && from < c.mediaItemCount) {
            c.seekToDefaultPosition(from)
            c.play()
        }
    }

    /** Swaps upcoming songs for copies with new metadata (no playback change). */
    fun retagUpcoming(transform: (index: Int, MediaItem) -> MediaItem?) = withController { c ->
        val from = c.currentMediaItemIndex + 1
        for (i in from until c.mediaItemCount) {
            transform(i - from, c.getMediaItemAt(i))?.let { c.replaceMediaItem(i, it) }
        }
    }

    fun togglePlayPause() = withController { c ->
        if (c.playbackState == Player.STATE_IDLE) c.prepare()
        if (c.playbackState == Player.STATE_ENDED) c.seekToDefaultPosition()
        if (c.isPlaying) c.pause() else c.play()
    }

    fun next() = withController { it.seekToNext() }
    fun previous() = withController {
        it.seekToPrevious()
        onQueueEdited?.invoke()
    }
    fun seekTo(positionMs: Long) = withController { it.seekTo(positionMs) }
    fun skipTo(index: Int) = withController {
        it.seekToDefaultPosition(index)
        onQueueEdited?.invoke()
    }
    fun toggleShuffle() = withController { it.shuffleModeEnabled = !it.shuffleModeEnabled }

    fun cycleRepeat() = withController { c ->
        c.repeatMode = when (c.repeatMode) {
            Player.REPEAT_MODE_OFF -> Player.REPEAT_MODE_ALL
            Player.REPEAT_MODE_ALL -> Player.REPEAT_MODE_ONE
            else -> Player.REPEAT_MODE_OFF
        }
    }

    private val listener = object : Player.Listener {
        override fun onEvents(player: Player, events: Player.Events) {
            val c = controller ?: return
            if (events.contains(Player.EVENT_TIMELINE_CHANGED)) rebuildQueue(c)
            if (events.contains(Player.EVENT_MEDIA_ITEM_TRANSITION)) c.currentMediaItem?.let(recents::add)
            sync(c)
        }
    }

    private fun rebuildQueue(c: MediaController) {
        val queue = (0 until c.mediaItemCount).map { i ->
            val item = c.getMediaItemAt(i)
            QueueEntry(
                item.mediaId,
                item.mediaMetadata.title?.toString().orEmpty(),
                item.mediaMetadata.artist?.toString().orEmpty(),
                item.mediaMetadata.artworkUri?.toString(),
                item.queueId,
            )
        }
        _state.update { it.copy(queue = queue) }
    }

    private fun sync(c: MediaController) {
        val item = c.currentMediaItem
        val metadata = c.mediaMetadata
        val current = item?.let {
            val track = trackLookup(it.mediaId)
            NowPlaying(
                mediaId = it.mediaId,
                title = metadata.title?.toString() ?: track?.title ?: it.mediaId.substringAfterLast('/'),
                artist = metadata.artist?.toString() ?: track?.artist.orEmpty(),
                album = metadata.albumTitle?.toString() ?: track?.album.orEmpty(),
                artwork = metadata.artworkData,
                artworkUri = metadata.artworkUri?.toString(),
                quality = track?.quality ?: streamQuality(c),
                genre = metadata.genre?.toString()?.ifBlank { null } ?: track?.genre?.ifBlank { null },
            )
        }
        _state.update {
            it.copy(
                current = current,
                isPlaying = c.isPlaying,
                isBuffering = c.playbackState == Player.STATE_BUFFERING,
                positionMs = c.currentPosition.coerceAtLeast(0),
                durationMs = c.duration.takeIf { d -> d != C.TIME_UNSET } ?: 0,
                shuffle = c.shuffleModeEnabled,
                repeatMode = c.repeatMode,
                index = c.currentMediaItemIndex,
                error = c.playerError?.let(::errorText),
            )
        }
        updateTicker(c)
    }

    /** Polls the position while playing (Player has no position callback). */
    private fun updateTicker(c: MediaController) {
        if (c.isPlaying && ticker?.isActive != true) {
            ticker = scope.launch {
                while (isActive) {
                    _state.update { it.copy(positionMs = c.currentPosition.coerceAtLeast(0)) }
                    delay(500)
                }
            }
        } else if (!c.isPlaying) {
            ticker?.cancel()
            ticker = null
        }
    }

    /** Format of the decoded audio track, for files that are not in the library. */
    private fun streamQuality(c: MediaController): String? {
        val format = c.currentTracks.groups
            .firstOrNull { it.type == C.TRACK_TYPE_AUDIO && it.isSelected }
            ?.let { group -> (0 until group.length).firstOrNull(group::isTrackSelected)?.let(group::getTrackFormat) }
            ?: return null
        val codec = when (format.sampleMimeType) {
            MimeTypes.AUDIO_FLAC -> "FLAC"
            MimeTypes.AUDIO_MPEG -> "MP3"
            MimeTypes.AUDIO_RAW, MimeTypes.AUDIO_WAV -> "WAV"
            MimeTypes.AUDIO_AAC -> "AAC"
            MimeTypes.AUDIO_ALAC -> "ALAC"
            MimeTypes.AUDIO_VORBIS -> "Vorbis"
            MimeTypes.AUDIO_OPUS -> "Opus"
            else -> format.sampleMimeType?.substringAfter('/')?.uppercase()
        } ?: return null
        val rate = format.sampleRate.takeIf { it > 0 } ?: return codec
        val khz = if (rate % 1000 == 0) "${rate / 1000}" else "%.1f".format(rate / 1000.0)
        return "$codec · $khz kHz"
    }

    private fun errorText(e: PlaybackException): String =
        when (e.errorCode) {
            PlaybackException.ERROR_CODE_DECODING_FORMAT_UNSUPPORTED,
            PlaybackException.ERROR_CODE_DECODER_INIT_FAILED -> context.getString(R.string.player_error_format)
            PlaybackException.ERROR_CODE_IO_FILE_NOT_FOUND -> context.getString(R.string.player_error_missing)
            PlaybackException.ERROR_CODE_IO_NO_PERMISSION -> context.getString(R.string.player_error_permission)
            PlaybackException.ERROR_CODE_IO_NETWORK_CONNECTION_FAILED -> context.getString(R.string.error_offline)
            PlaybackException.ERROR_CODE_IO_NETWORK_CONNECTION_TIMEOUT -> context.getString(R.string.error_timeout)
            // a Tidal song that could not be fetched carries the reason in plain words
            else -> e.cause?.let { com.perseusoft.homefi.data.Humane.of(it) }
                ?.takeIf { it != context.getString(R.string.error_generic) }
                ?: context.getString(R.string.player_error_other)
        }
}
