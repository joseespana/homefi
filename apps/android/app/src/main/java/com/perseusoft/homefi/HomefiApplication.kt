package com.perseusoft.homefi

import android.app.Application
import coil3.ImageLoader
import coil3.PlatformContext
import coil3.SingletonImageLoader
import com.perseusoft.homefi.core.HomefiEngine
import com.perseusoft.homefi.core.setLanguage
import com.perseusoft.homefi.data.AccountStore
import com.perseusoft.homefi.data.AiSetup
import com.perseusoft.homefi.data.DownloadRepository
import com.perseusoft.homefi.data.KeyVault
import com.perseusoft.homefi.data.SettingsStore
import com.perseusoft.homefi.library.EmbeddedCover
import com.perseusoft.homefi.library.EmbeddedCoverFetcher
import com.perseusoft.homefi.library.EmbeddedCoverKeyer
import com.perseusoft.homefi.library.LibraryRepository
import com.perseusoft.homefi.lyrics.LyricsRepository
import com.perseusoft.homefi.player.AudioOutputs
import com.perseusoft.homefi.player.Equalizer
import com.perseusoft.homefi.player.PlayerController
import com.perseusoft.homefi.player.SoundLayers
import com.perseusoft.homefi.share.ShareManager
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch

class HomefiApplication : Application(), SingletonImageLoader.Factory {
    /** Lives as long as the process: downloads keep running across screens. */
    val appScope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    lateinit var engine: HomefiEngine
        private set
    /** The signed-in Tidal account, read off the main thread. */
    lateinit var account: AccountStore
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
    /** Equalizer settings, presets and the output whose profile plays. */
    lateinit var equalizer: Equalizer
        private set
    /** Bring-your-own-key AI (keys in the Android Keystore). */
    lateinit var ai: AiSetup
        private set
    /** Taste layers, loudness and "How does it sound?" on top of the equalizer. */
    lateinit var sound: SoundLayers
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
        account = AccountStore(engine, appScope)
        settings = SettingsStore(this)
        // album lists come grouped from the core, as the settings say (and in the phone's language)
        engine.setAlbumVersions(settings.current.albumVersions, java.util.Locale.getDefault().toLanguageTag())
        appScope.launch {
            settings.settings.map { it.albumVersions }.distinctUntilChanged().collect { prefs ->
                engine.setAlbumVersions(prefs, java.util.Locale.getDefault().toLanguageTag())
            }
        }
        downloads = DownloadRepository(this, engine, settings, appScope)
        library = LibraryRepository(this, engine, downloads, appScope)
        outputs = AudioOutputs(this)
        equalizer = Equalizer(settings, outputs, appScope)
        ai = AiSetup(settings, KeyVault(this))
        sound = SoundLayers(this, equalizer, outputs, appScope)
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

    /** AutoEq's headphone list and profiles, shared by the engine and the assistant. */
    val autoeqCacheDir: String get() = filesDir.resolve("homefi").resolve("autoeq-cache").absolutePath
}

val android.content.Context.homefi: HomefiApplication
    get() = applicationContext as HomefiApplication
