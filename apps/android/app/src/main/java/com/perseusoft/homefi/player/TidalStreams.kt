package com.perseusoft.homefi.player

import android.content.Context
import android.net.Uri
import androidx.annotation.OptIn
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DataSpec
import androidx.media3.datasource.ResolvingDataSource
import com.perseusoft.homefi.core.trimStreamCache
import com.perseusoft.homefi.data.userMessage
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.Busy
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import java.io.File
import java.io.IOException

/**
 * Tidal playback: `tidal://track/<id>` is fetched into the stream cache (the
 * whole file, like the desktop app) and played from there. The next tracks of
 * the queue are fetched ahead so they start without a wait.
 */
@OptIn(UnstableApi::class)
class TidalStreams(private val context: Context, private val scope: CoroutineScope) : ResolvingDataSource.Resolver {
    private val dir = File(context.cacheDir, "stream").also { it.mkdirs() }

    private suspend fun fetch(id: String): String = context.homefi.engine.fetchStream(id, dir.absolutePath)

    /** Runs on ExoPlayer's loading thread, so blocking until the file is there is fine. */
    override fun resolveDataSpec(dataSpec: DataSpec): DataSpec {
        val uri = dataSpec.uri
        if (uri.scheme != "tidal") return dataSpec
        val id = uri.lastPathSegment ?: return dataSpec
        val path = try {
            Busy.trackBlocking { runBlocking(Dispatchers.IO) { fetch(id) } }
        } catch (e: Exception) {
            throw IOException(e.userMessage(), e) // shows in the player as a playback error
        }
        return dataSpec.withUri(Uri.fromFile(File(path)))
    }

    /** Fetches the next [AHEAD] Tidal tracks and keeps the cache under [MAX_BYTES]. */
    fun prefetch(player: Player) {
        val ids = (1..AHEAD).mapNotNull { offset ->
            val i = player.currentMediaItemIndex + offset
            if (i < player.mediaItemCount) tidalId(player.getMediaItemAt(i).mediaId) else null
        }
        val current = player.currentMediaItem?.mediaId?.let(::tidalId)
        scope.launch(Dispatchers.IO) {
            val kept = ids.mapNotNull { id -> runCatching { fetch(id) }.getOrNull() }
            val playing = dir.listFiles()?.filter { current != null && it.nameWithoutExtension == current }?.map { it.absolutePath }.orEmpty()
            runCatching { trimStreamCache(dir.absolutePath, MAX_BYTES, kept + playing) }
        }
    }

    companion object {
        private const val AHEAD = 2
        private const val MAX_BYTES: ULong = 2_147_483_648UL // 2 GB
    }
}
