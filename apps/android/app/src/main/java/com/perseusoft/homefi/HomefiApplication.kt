package com.perseusoft.homefi

import android.app.Application
import coil3.ImageLoader
import coil3.PlatformContext
import coil3.SingletonImageLoader
import com.perseusoft.homefi.core.HomefiEngine
import com.perseusoft.homefi.core.setLanguage
import com.perseusoft.homefi.data.DownloadRepository
import com.perseusoft.homefi.data.SettingsStore
import com.perseusoft.homefi.library.EmbeddedCover
import com.perseusoft.homefi.library.EmbeddedCoverFetcher
import com.perseusoft.homefi.library.EmbeddedCoverKeyer
import com.perseusoft.homefi.library.LibraryRepository
import com.perseusoft.homefi.lyrics.LyricsRepository
import com.perseusoft.homefi.player.AudioOutputs
import com.perseusoft.homefi.player.PlayerController
import com.perseusoft.homefi.share.ShareManager
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob

class HomefiApplication : Application(), SingletonImageLoader.Factory {
    /** Lives as long as the process: downloads keep running across screens. */
    val appScope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    lateinit var engine: HomefiEngine
        private set
    lateinit var settings: SettingsStore
        private set
    lateinit var downloads: DownloadRepository
        private set
    lateinit var library: LibraryRepository
        private set
    lateinit var player: PlayerController
        private set
    lateinit var outputs: AudioOutputs
        private set
    /** Song lyrics for the player, cached per song. */
    lateinit var lyrics: LyricsRepository
        private set
    /** Shared control / party mode, as hub or guest. */
    lateinit var share: ShareManager
        private set

    override fun onCreate() {
        super.onCreate()
        CrashLog.install(this)
        // error messages from the Rust core in the phone's language
        setLanguage(java.util.Locale.getDefault().toLanguageTag())
        com.perseusoft.homefi.data.Humane.init(this)
        engine = HomefiEngine(filesDir.resolve("homefi").absolutePath)
        settings = SettingsStore(this)
        downloads = DownloadRepository(this, engine, settings, appScope)
        library = LibraryRepository(this, engine, downloads, appScope)
        outputs = AudioOutputs(this)
        player = PlayerController(this) { mediaId -> library.find(mediaId) }
        share = ShareManager(this)
        lyrics = LyricsRepository(engine, appScope)
    }

    override fun newImageLoader(context: PlatformContext): ImageLoader =
        ImageLoader.Builder(context)
            .components {
                add(EmbeddedCoverKeyer(), EmbeddedCover::class)
                add(EmbeddedCoverFetcher.Factory(), EmbeddedCover::class)
            }
            .build()
}

val android.content.Context.homefi: HomefiApplication
    get() = applicationContext as HomefiApplication
