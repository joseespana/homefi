package com.perseusoft.homefi.library

import coil3.ImageLoader
import coil3.decode.DataSource
import coil3.decode.ImageSource
import coil3.fetch.FetchResult
import coil3.fetch.Fetcher
import coil3.fetch.SourceFetchResult
import coil3.key.Keyer
import coil3.request.Options
import com.perseusoft.homefi.core.readCover
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okio.Buffer
import java.util.concurrent.ConcurrentHashMap

/** Coil model: the cover embedded in (or next to) a local audio file. */
data class EmbeddedCover(val path: String)

/** Loads covers through the Rust core (lofty), which understands every tag format. */
class EmbeddedCoverFetcher(private val cover: EmbeddedCover, private val options: Options) : Fetcher {
    override suspend fun fetch(): FetchResult? {
        if (cover.path in missing) return null
        val bytes = withContext(Dispatchers.IO) { readCover(cover.path) }
        if (bytes == null) {
            missing += cover.path
            return null
        }
        return SourceFetchResult(
            source = ImageSource(Buffer().write(bytes), options.fileSystem),
            mimeType = null,
            dataSource = DataSource.DISK,
        )
    }

    class Factory : Fetcher.Factory<EmbeddedCover> {
        override fun create(data: EmbeddedCover, options: Options, imageLoader: ImageLoader): Fetcher =
            EmbeddedCoverFetcher(data, options)
    }

    companion object {
        /** Files known to have no cover, so scrolling does not re-read them. */
        private val missing: MutableSet<String> = ConcurrentHashMap.newKeySet()

        fun forget(path: String) = missing.remove(path)
    }
}

class EmbeddedCoverKeyer : Keyer<EmbeddedCover> {
    override fun key(data: EmbeddedCover, options: Options): String = "cover:${data.path}"
}
