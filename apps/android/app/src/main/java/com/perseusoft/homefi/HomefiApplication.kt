package com.perseusoft.homefi

import android.app.Application
import coil3.ImageLoader
import coil3.PlatformContext
import coil3.SingletonImageLoader
import com.perseusoft.homefi.core.HomefiEngine
import com.perseusoft.homefi.core.setLanguage
import com.perseusoft.homefi.data.AccountStore
import com.perseusoft.homefi.data.AiSetup
import com.perseusoft.homefi.data.AppUpdater
import com.perseusoft.homefi.data.DeviceForm
import com.perseusoft.homefi.data.DjSetupsStore
import com.perseusoft.homefi.data.DownloadRepository
import com.perseusoft.homefi.data.IncomingFiles
import com.perseusoft.homefi.data.Notes
import com.perseusoft.homefi.data.SavedRides
import com.perseusoft.homefi.data.refs
import com.perseusoft.homefi.data.HomePlaylists
import com.perseusoft.homefi.data.KeyVault
import com.perseusoft.homefi.data.SettingsStore
import com.perseusoft.homefi.data.WhatsNew
import com.perseusoft.homefi.library.EmbeddedCover
import com.perseusoft.homefi.library.EmbeddedCoverFetcher
import com.perseusoft.homefi.library.EmbeddedCoverKeyer
import com.perseusoft.homefi.library.LibraryRepository
import com.perseusoft.homefi.lyrics.LyricsRepository
import com.perseusoft.homefi.player.AudioOutputs
import com.perseusoft.homefi.player.Equalizer
import com.perseusoft.homefi.player.PlayerController
import com.perseusoft.homefi.player.dj.DjSession
import com.perseusoft.homefi.player.exercise.RideSession
import com.perseusoft.homefi.player.SoundLayers
import com.perseusoft.homefi.share.ShareManager
import com.perseusoft.homefi.ui.playlists.SongSeeds
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
    /** Mounted storage volumes (internal, USB, SD) and their free space. */
    lateinit var volumes: com.perseusoft.homefi.data.VolumeMonitor
        private set
    /** Songs hearted in the TV player. */
    val liked: com.perseusoft.homefi.data.LikedSongs by lazy { com.perseusoft.homefi.data.LikedSongs(this) }
    /** What the TV's search was asked for lately. */
    val recentSearches: com.perseusoft.homefi.data.RecentSearches by lazy { com.perseusoft.homefi.data.RecentSearches(this) }
    lateinit var library: LibraryRepository
        private set
    lateinit var player: PlayerController
        private set
    /** Playlists HomeFi built (concert setlists), saved as JSON in its own folder. */
    lateinit var playlists: HomePlaylists
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
    /** The simple DJ (two decks); idle, with no players, until its screen opens. */
    lateinit var dj: DjSession
        private set
    /** Exercise mode (indoor cycling): the ride's plan, clock, cues and TV. */
    lateinit var ride: RideSession
        private set
    /**
     * Phone or TV, read once at startup from the device and the `tv.uiForm` setting: a change of
     * that setting applies the next time the app starts (the UI never swaps under the user).
     */
    lateinit var deviceForm: DeviceForm
        private set
    /** One-line messages for the root snackbar. */
    val notes = Notes()
    /** The TV this phone is paired with (QR, no typing): rides open there by themselves. */
    val tvPairings: com.perseusoft.homefi.pair.TvPairings by lazy { com.perseusoft.homefi.pair.TvPairings(this) }
    /** Rides saved by themselves after each build, kept, favorited, exported. */
    lateinit var rides: SavedRides
        private set
    /** Named DJ setups (options + screen layout). */
    lateinit var djSetups: DjSetupsStore
        private set
    /** A ride or DJ setup file opened from another app, waiting for a yes. */
    lateinit var incoming: IncomingFiles
        private set
    /** Shared control / party mode, as hub or guest. */
    lateinit var share: ShareManager
        private set
    /** "Playlist like this": a song waiting to open the AI chat. */
    val songSeeds = SongSeeds()

    /** "HomeFi X is available" (official build only; nothing about the user is sent). */
    lateinit var updates: AppUpdater
        private set

    /** "What's new" after an update, from the core's embedded CHANGELOG. */
    lateinit var whatsNew: WhatsNew
        private set

    override fun onCreate() {
        super.onCreate()
        // Loading the Rust library (JNA, UniFFI checksums) takes ~180 ms: do it on another thread while
        // the main thread builds what does not need the core. The same thread sets the core's language
        // (error messages in the phone's language), and the main thread waits for it before any core use.
        val languageTag = java.util.Locale.getDefault().toLanguageTag()
        var coreReady = false
        val warmUp = Thread({
            coreReady = runCatching { setLanguage(languageTag) }.isSuccess
        }, "homefi-core-warmup").apply { start() }
        CrashLog.install(this)
        outputs = AudioOutputs(this)
        warmUp.join()
        // it failed there: try again here so the failure shows up as it always did
        if (!coreReady) setLanguage(languageTag)
        com.perseusoft.homefi.data.Humane.init(this)
        engine = HomefiEngine(filesDir.resolve("homefi").absolutePath)
        account = AccountStore(engine, appScope)
        settings = SettingsStore(this)
        deviceForm = DeviceForm.detect(this, settings.current.tv.uiForm)
        // album lists come grouped from the core, as the settings say (and in the phone's language)
        engine.setAlbumVersions(settings.current.albumVersions, java.util.Locale.getDefault().toLanguageTag())
        appScope.launch {
            settings.settings.map { it.albumVersions }.distinctUntilChanged().collect { prefs ->
                engine.setAlbumVersions(prefs, java.util.Locale.getDefault().toLanguageTag())
            }
        }
        volumes = com.perseusoft.homefi.data.VolumeMonitor(this, appScope)
        downloads = DownloadRepository(this, engine, settings, appScope)
        library = LibraryRepository(this, engine, downloads, appScope)
        playlists = HomePlaylists(engine, appScope, settings.playlists) { account.isLoggedIn }
        equalizer = Equalizer(settings, outputs, appScope)
        ai = AiSetup(settings, KeyVault(this))
        sound = SoundLayers(this, equalizer, outputs, appScope)
        player = PlayerController(this) { mediaId -> library.find(mediaId) }
        dj = DjSession(this)
        val appDir = filesDir.resolve("homefi").absolutePath
        rides = SavedRides(this, appDir, appScope, notes) { library.refs() }
        djSetups = DjSetupsStore(this, appDir, settings, appScope, notes)
        incoming = IncomingFiles(this, appScope, notes, rides, djSetups)
        share = ShareManager(this)
        ride = RideSession(this)
        lyrics = LyricsRepository(engine, appScope)
        updates = AppUpdater(this, settings, appScope)
        whatsNew = WhatsNew(this, settings, appScope)
        whatsNew.check()
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
