package com.perseusoft.homefi.data

import android.content.Context
import com.perseusoft.homefi.core.AlbumEditionPref
import com.perseusoft.homefi.core.AlbumExplicitPref
import com.perseusoft.homefi.core.AlbumSpatialPref
import com.perseusoft.homefi.core.AlbumVersionPrefs
import com.perseusoft.homefi.core.AudioQuality
import com.perseusoft.homefi.core.EqProfile
import com.perseusoft.homefi.core.EqSettings
import com.perseusoft.homefi.core.eqDefaultSettings
import com.perseusoft.homefi.core.eqSettingsFromJson
import com.perseusoft.homefi.core.eqSettingsToJson
import com.perseusoft.homefi.core.LoudnessSettings
import com.perseusoft.homefi.core.CrossfadeSettings
import com.perseusoft.homefi.core.crossfadeDefaultSettings
import com.perseusoft.homefi.core.crossfadeSettingsFromJson
import com.perseusoft.homefi.core.crossfadeSettingsToJson
import com.perseusoft.homefi.core.TasteSettings
import com.perseusoft.homefi.core.loudnessDefaultSettings
import com.perseusoft.homefi.core.loudnessSettingsFromJson
import com.perseusoft.homefi.core.loudnessSettingsToJson
import com.perseusoft.homefi.core.tasteDefaultSettings
import com.perseusoft.homefi.core.tasteSettingsFromJson
import com.perseusoft.homefi.core.tasteSettingsToJson
import org.json.JSONObject
import com.perseusoft.homefi.core.ShareAddMode
import com.perseusoft.homefi.core.ShareEditMode
import com.perseusoft.homefi.core.SharePermissions
import com.perseusoft.homefi.core.ShareQueueOrder
import com.perseusoft.homefi.core.ShareSkipMode
import com.perseusoft.homefi.share.net.ShareNetworkMode
import com.perseusoft.homefi.ui.player.DanceId
import com.perseusoft.homefi.ui.player.Mood
import com.perseusoft.homefi.ui.player.Specials
import com.perseusoft.homefi.ui.player.Temper
import com.perseusoft.homefi.ui.player.Turns
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

enum class ThemeMode { SYSTEM, LIGHT, DARK }

/**
 * How much the app moves (same values as the desktop). AUTO is FULL unless the system asks
 * for less ("Remove animations"), then SUBTLE; SUBTLE is short fades only; OFF none.
 */
enum class Motion { AUTO, FULL, SUBTLE, OFF }

/** How the full-screen player is laid out. */
/** Look of the player's seek bar. */
enum class ProgressStyle { CLASSIC, WAVY, WAVEFORM, NEON, LED, MINIMAL }

enum class ProgressThickness { THIN, MEDIUM, THICK }

enum class PlayerLayout { CLASSIC, IMMERSIVE, VINYL, STUDIO, RETRO_AMP, LUNA, ALUMINUM, WEB2000 }

/** Shade under the title in the immersive layout; AUTO follows how light the cover is there. */
enum class ImmersiveShade { AUTO, STRONG, SOFT, OFF }

/** How the lyrics view shows a song: the whole text, or one big line filling as it's sung. */
enum class LyricsMode { NORMAL, KARAOKE }

enum class LyricsSize { SMALL, MEDIUM, LARGE }

/** How the player's signal chain reads: one line of nodes, or nodes with a caption above. */
enum class SignalStyle(val key: String) {
    COMPACT("compact"),
    DETAILED("detailed");

    companion object {
        fun of(key: String?): SignalStyle = entries.firstOrNull { it.key == key } ?: COMPACT
    }
}

/** Colour of the mascot's arms and legs; OFF draws none. */
enum class MascotLimbs { WHITE, NAVY, ACCENT, OFF }

/** Shared control (car mode) and party mode: the hub's rules and this phone as a guest. */
data class ShareSettings(
    /** Blank uses "<name>'s music". */
    val sessionName: String = "",
    /** How others see you; blank uses the phone's model. */
    val name: String = "",
    /** `#RRGGBB` */
    val color: String = "#0891B2",
    val network: ShareNetworkMode = ShareNetworkMode.WIFI_DIRECT,
    /** Keep [code] instead of a random one each session. */
    val fixedCode: Boolean = false,
    val code: String = "",
    val port: Int = 47800,
    /** The share sheet starts on party mode (stream to speakers). */
    val partyDefault: Boolean = false,
    val add: ShareAddMode = ShareAddMode.FREE,
    val maxPerGuest: Int = 3,
    val order: ShareQueueOrder = ShareQueueOrder.TURNS,
    val skip: ShareSkipMode = ShareSkipMode.VOTE,
    val playback: Boolean = false,
    val edit: ShareEditMode = ShareEditMode.ALL,
    val allowExplicit: Boolean = true,
    val dropSongsOnLeave: Boolean = false,
    val noticeJoined: Boolean = true,
    val noticeLeft: Boolean = true,
    val noticeAdded: Boolean = true,
    val noticeApproval: Boolean = true,
    /** Seconds a notice stays on screen. */
    val noticeSeconds: Int = 4,
    /** Look for sessions nearby (Wi-Fi Direct and the home Wi-Fi) while the app is open. */
    val discover: Boolean = true,
    /** Offer a session found nearby on any screen, not only on Home. */
    val discoverEverywhere: Boolean = true,
    /** A Wi-Fi Direct hub also announces itself on the home Wi-Fi when this phone is on one. */
    val announceLan: Boolean = true,
    /** Announce and look for sessions over Bluetooth LE too (works without location / GPS on the hub). */
    val bluetooth: Boolean = true,
    /**
     * A Wi-Fi Direct hub keeps scanning (so new guests find it over Wi-Fi Direct) once guests
     * are in. Off by default: each scan takes the radio off the group's channel and the
     * guests' audio stutters; Bluetooth, the home Wi-Fi and the gateway still find it.
     */
    val visibleWithGuests: Boolean = false,
    /**
     * Wi-Fi Direct group name and password, the same every time so devices without a
     * camera (the SR35) join it once from the system Wi-Fi settings and remember it.
     * Blank: made up on the first party and kept. Android 10+ hubs only (older ones pick their own).
     */
    val groupName: String = "",
    val groupPass: String = "",
    /** As a guest, play the hub's stream on this phone when it streams. */
    val playAlong: Boolean = true,
    /** This phone's speaker latency (Bluetooth...), as hub and as guest; may be negative. */
    val speakerDelayMs: Int = 0,
    /** Playing along: close small gaps by playing a touch faster or slower; off, only jump. */
    val syncBySpeed: Boolean = true,
    /** Playing along: download the next and previous songs ahead and play them from this phone. */
    val preloadSongs: Boolean = true,
    /** As a guest, open on the full player with the hub's song; off, on the room (queue and controls). */
    val guestOpensPlayer: Boolean = true,
    /**
     * Party mode, as hub: at most this long (seconds) a new song waits for every guest to
     * have it loaded, so all start together. 0 starts at once.
     */
    val partyStartWaitSec: Int = 3,
    /** As hub: send the music's levels so guests' visualizers and VU meters move. */
    val sendLevels: Boolean = true,
) {
    fun permissions() = SharePermissions(
        add = add,
        maxPerGuest = maxPerGuest.coerceAtLeast(1).toUInt(),
        order = order,
        skip = skip,
        playback = playback,
        edit = edit,
        allowExplicit = allowExplicit,
        dropSongsOnLeave = dropSongsOnLeave,
    )
}

/**
 * Bring-your-own-key AI: the provider picked (its `aiProviders()` id, blank = none, the
 * guided wizard) and, per provider, the model and server address. Never the key: that
 * lives in [KeyVault].
 */
data class AiSettings(
    val provider: String = "",
    /** Provider id → model id; missing or blank = the provider's default. */
    val models: Map<String, String> = emptyMap(),
    /** Provider id → server address (Ollama, custom); missing = the provider's. */
    val baseUrls: Map<String, String> = emptyMap(),
)

data class AppSettings(
    val quality: AudioQuality = AudioQuality.HIGH,
    val embedCover: Boolean = true,
    val embedLyrics: Boolean = false,
    val includeVideos: Boolean = false,
    val skipExisting: Boolean = true,
    val threads: Int = 3,
    val template: String = "",
    /** Folder picked by the user (an SD card, say); blank means the default. */
    val downloadDir: String = "",
    val theme: ThemeMode = ThemeMode.SYSTEM,
    /** Transitions between screens, the player opening, lists arriving... */
    val motion: Motion = Motion.AUTO,
    /** A little buzz on the buttons worth feeling (play, skip, seek, options, downloads). */
    val haptics: Boolean = true,
    /** Player stage: album cover, spectrum, VU meters or mascot. */
    val stage: Int = 0,
    val playerLayout: PlayerLayout = PlayerLayout.CLASSIC,
    /** The player layout skins the whole app (bars, lists, settings...), not only the player. */
    val layoutEverywhere: Boolean = true,
    val immersiveShade: ImmersiveShade = ImmersiveShade.AUTO,
    val progressStyle: ProgressStyle = ProgressStyle.WAVY,
    val progressThickness: ProgressThickness = ProgressThickness.MEDIUM,
    val progressAnimated: Boolean = true,
    /** Mascot look and groove; null follows the genre. */
    val mascotMood: Mood? = null,
    /** A famous dance kept going all the time; null mixes them in now and then. */
    val mascotDance: DanceId? = null,
    val mascotSpecials: Specials = Specials.SOMETIMES,
    val mascotLimbs: MascotLimbs = MascotLimbs.WHITE,
    val mascotTurns: Turns = Turns.ALL,
    /** Long-press on the mascot switches to the next style. */
    val mascotLongPress: Boolean = true,
    /** How many quick taps make it angry. */
    val mascotTemper: Temper = Temper.NORMAL,
    /** Where the user dragged the mascot, as a fraction of its stage (0 = centred). */
    val mascotX: Float = 0f,
    val mascotY: Float = 0f,
    val mascotScale: Float = 1f,
    /** Tidal lists show their first page at once and load the rest while scrolling; off loads them whole. */
    val lazyLists: Boolean = true,
    /** One card per album on Tidal lists (explicit / clean, Atmos, remasters... behind a "versions" chip). */
    val albumVersionsGroup: Boolean = true,
    /** Which release the card shows, when both exist. */
    val albumExplicit: AlbumExplicitPref = AlbumExplicitPref.EXPLICIT,
    val albumSpatial: AlbumSpatialPref = AlbumSpatialPref.STEREO,
    val albumEdition: AlbumEditionPref = AlbumEditionPref.ORIGINAL,
    val lyricsMode: LyricsMode = LyricsMode.NORMAL,
    val lyricsSize: LyricsSize = LyricsSize.MEDIUM,
    /** Open the lyrics by themselves when the song has some. */
    val lyricsAutoOpen: Boolean = false,
    /** HomeFito sings along in karaoke. */
    val lyricsMascot: Boolean = true,
    /** Lyrics timing, −2000..2000 ms; positive shows them later (Bluetooth delay). */
    val lyricsOffsetMs: Int = 0,
    val share: ShareSettings = ShareSettings(),
    val ai: AiSettings = AiSettings(),
    /** "How does it sound?" button in the player (it stays in the player's menu either way). */
    val soundReactButton: Boolean = true,
    /** The signal badge names the active layers ("EQ · HD 650 + Electronic") instead of "not bit-perfect". */
    val soundLayerChip: Boolean = true,
    /** The player shows the signal chain (source → processing → output) instead of the badges. */
    val signalChain: Boolean = true,
    val signalStyle: SignalStyle = SignalStyle.COMPACT,
    /** The chain's links flow while playing (motion settings still win). */
    val signalAnimate: Boolean = true,
    /** A taste suggestion card in the equalizer's taste rules. */
    val soundSuggestions: Boolean = true,
    /** Version of the personal-use notice accepted before connecting Tidal; 0 = never. */
    val tidalTermsAccepted: Int = 0,
    /** When it was accepted (epoch ms); 0 = never. */
    val tidalTermsAcceptedAt: Long = 0L,
) {
    /** How the core groups album lists (the same preferences as the desktop's). */
    val albumVersions: AlbumVersionPrefs
        get() = AlbumVersionPrefs(group = albumVersionsGroup, explicit = albumExplicit, spatial = albumSpatial, edition = albumEdition)

    /** The current personal-use notice ([TIDAL_TERMS_VERSION]) was accepted. */
    val termsAccepted: Boolean get() = tidalTermsAccepted >= TIDAL_TERMS_VERSION
}

/** Version of the personal-use notice (R.string.tidal_terms). Bump it when the text
 *  changes in substance: everyone is asked again. Same number as the desktop's. */
const val TIDAL_TERMS_VERSION = 1

/** Lyrics timing limits; moved in [LYRICS_OFFSET_STEP] steps. */
val LYRICS_OFFSET_RANGE = -2000..2000
const val LYRICS_OFFSET_STEP = 100

class SettingsStore(context: Context) {
    private val prefs = context.getSharedPreferences("settings", Context.MODE_PRIVATE)
    private val _settings = MutableStateFlow(load())
    val settings: StateFlow<AppSettings> = _settings.asStateFlow()

    val current: AppSettings get() = _settings.value

    private fun load() = AppSettings(
        quality = prefs.getString("quality", null)
            ?.let { runCatching { AudioQuality.valueOf(it) }.getOrNull() }
            ?: AudioQuality.HIGH,
        embedCover = prefs.getBoolean("embed_cover", true),
        embedLyrics = prefs.getBoolean("embed_lyrics", false),
        includeVideos = prefs.getBoolean("include_videos", false),
        skipExisting = prefs.getBoolean("skip_existing", true),
        threads = prefs.getInt("threads", 3),
        lazyLists = prefs.getBoolean("lazy_lists", true),
        albumVersionsGroup = prefs.getBoolean("album_versions_group", true),
        albumExplicit = enumPref("album_explicit", AlbumExplicitPref.EXPLICIT),
        albumSpatial = enumPref("album_spatial", AlbumSpatialPref.STEREO),
        albumEdition = enumPref("album_edition", AlbumEditionPref.ORIGINAL),
        template = prefs.getString("template", "") ?: "",
        downloadDir = prefs.getString("download_dir", "") ?: "",
        theme = prefs.getString("theme", null)
            ?.let { runCatching { ThemeMode.valueOf(it) }.getOrNull() }
            ?: ThemeMode.SYSTEM,
        motion = enumPref("motion", Motion.AUTO),
        haptics = prefs.getBoolean("haptics", true),
        stage = prefs.getInt("stage", 0),
        playerLayout = prefs.getString("player_layout", null)
            ?.let { runCatching { PlayerLayout.valueOf(it) }.getOrNull() }
            ?: PlayerLayout.CLASSIC,
        layoutEverywhere = prefs.getBoolean("layout_everywhere", true),
        immersiveShade = enumPref("immersive_shade", ImmersiveShade.AUTO),
        progressStyle = prefs.getString("progress_style", null)
            ?.let { runCatching { ProgressStyle.valueOf(it) }.getOrNull() }
            ?: ProgressStyle.WAVY,
        progressThickness = prefs.getString("progress_thickness", null)
            ?.let { runCatching { ProgressThickness.valueOf(it) }.getOrNull() }
            ?: ProgressThickness.MEDIUM,
        progressAnimated = prefs.getBoolean("progress_animated", true),
        mascotMood = prefs.getString("mascot_mood", null)
            ?.let { runCatching { Mood.valueOf(it) }.getOrNull() },
        mascotDance = prefs.getString("mascot_dance", null)
            ?.let { runCatching { DanceId.valueOf(it) }.getOrNull() },
        mascotSpecials = prefs.getString("mascot_specials", null)
            ?.let { runCatching { Specials.valueOf(it) }.getOrNull() }
            ?: Specials.SOMETIMES,
        mascotTurns = prefs.getString("mascot_turns", null)
            ?.let { runCatching { Turns.valueOf(it) }.getOrNull() }
            ?: Turns.ALL,
        mascotLongPress = prefs.getBoolean("mascot_long_press", true),
        mascotTemper = prefs.getString("mascot_temper", null)
            ?.let { runCatching { Temper.valueOf(it) }.getOrNull() }
            ?: Temper.NORMAL,
        mascotLimbs = prefs.getString("mascot_limbs", null)
            ?.let { runCatching { MascotLimbs.valueOf(it) }.getOrNull() }
            ?: MascotLimbs.WHITE,
        mascotX = prefs.getFloat("mascot_x", 0f),
        mascotY = prefs.getFloat("mascot_y", 0f),
        mascotScale = prefs.getFloat("mascot_scale", 1f),
        lyricsMode = enumPref("lyrics_mode", LyricsMode.NORMAL),
        lyricsSize = enumPref("lyrics_size", LyricsSize.MEDIUM),
        lyricsAutoOpen = prefs.getBoolean("lyrics_auto_open", false),
        lyricsMascot = prefs.getBoolean("lyrics_mascot", true),
        lyricsOffsetMs = prefs.getInt("lyrics_offset_ms", 0).coerceIn(LYRICS_OFFSET_RANGE),
        share = loadShare(),
        ai = AiSettings(
            provider = prefs.getString("ai_provider", "").orEmpty(),
            models = stringMap("ai_models"),
            baseUrls = stringMap("ai_base_urls"),
        ),
        soundReactButton = prefs.getBoolean("sound_react_button", true),
        soundLayerChip = prefs.getBoolean("sound_layer_chip", true),
        signalChain = prefs.getBoolean("signal_chain", true),
        signalStyle = SignalStyle.of(prefs.getString("signal_style", null)),
        signalAnimate = prefs.getBoolean("signal_animate", true),
        soundSuggestions = prefs.getBoolean("sound_suggestions", true),
        tidalTermsAccepted = prefs.getInt("tidal_terms_accepted", 0),
        tidalTermsAcceptedAt = prefs.getLong("tidal_terms_accepted_at", 0L),
    )

    private fun stringMap(key: String): Map<String, String> = runCatching {
        val o = JSONObject(prefs.getString(key, null) ?: return emptyMap())
        o.keys().asSequence().associateWith { o.getString(it) }
    }.getOrDefault(emptyMap())

    private inline fun <reified E : Enum<E>> enumPref(key: String, default: E): E =
        prefs.getString(key, null)?.let { runCatching { enumValueOf<E>(it) }.getOrNull() } ?: default

    private fun loadShare(): ShareSettings {
        val d = ShareSettings()
        return ShareSettings(
            sessionName = prefs.getString("share_session", d.sessionName) ?: d.sessionName,
            name = prefs.getString("share_name", d.name) ?: d.name,
            color = prefs.getString("share_color", d.color) ?: d.color,
            network = enumPref("share_network", d.network),
            fixedCode = prefs.getBoolean("share_fixed_code", d.fixedCode),
            code = prefs.getString("share_code", d.code) ?: d.code,
            port = prefs.getInt("share_port", d.port),
            partyDefault = prefs.getBoolean("share_party_default", d.partyDefault),
            add = enumPref("share_add", d.add),
            maxPerGuest = prefs.getInt("share_max_per_guest", d.maxPerGuest),
            order = enumPref("share_order", d.order),
            skip = enumPref("share_skip", d.skip),
            playback = prefs.getBoolean("share_playback", d.playback),
            edit = enumPref("share_edit", d.edit),
            allowExplicit = prefs.getBoolean("share_explicit", d.allowExplicit),
            dropSongsOnLeave = prefs.getBoolean("share_drop_on_leave", d.dropSongsOnLeave),
            noticeJoined = prefs.getBoolean("share_notice_joined", d.noticeJoined),
            noticeLeft = prefs.getBoolean("share_notice_left", d.noticeLeft),
            noticeAdded = prefs.getBoolean("share_notice_added", d.noticeAdded),
            noticeApproval = prefs.getBoolean("share_notice_approval", d.noticeApproval),
            noticeSeconds = prefs.getInt("share_notice_seconds", d.noticeSeconds),
            discover = prefs.getBoolean("share_discover", d.discover),
            discoverEverywhere = prefs.getBoolean("share_discover_everywhere", d.discoverEverywhere),
            announceLan = prefs.getBoolean("share_announce_lan", d.announceLan),
            bluetooth = prefs.getBoolean("share_bluetooth", d.bluetooth),
            visibleWithGuests = prefs.getBoolean("share_visible_with_guests", d.visibleWithGuests),
            groupName = prefs.getString("share_group_name", d.groupName).orEmpty(),
            groupPass = prefs.getString("share_group_pass", d.groupPass).orEmpty(),
            playAlong = prefs.getBoolean("share_play_along", d.playAlong),
            speakerDelayMs = prefs.getInt("share_speaker_delay", d.speakerDelayMs),
            syncBySpeed = prefs.getBoolean("share_sync_by_speed", d.syncBySpeed),
            preloadSongs = prefs.getBoolean("share_preload_songs", d.preloadSongs),
            guestOpensPlayer = prefs.getBoolean("share_guest_opens_player", d.guestOpensPlayer),
            partyStartWaitSec = prefs.getInt("share_party_start_wait", d.partyStartWaitSec),
            sendLevels = prefs.getBoolean("share_send_levels", d.sendLevels),
        )
    }

    fun update(transform: (AppSettings) -> AppSettings) {
        val next = transform(_settings.value)
        prefs.edit()
            .putString("quality", next.quality.name)
            .putBoolean("embed_cover", next.embedCover)
            .putBoolean("embed_lyrics", next.embedLyrics)
            .putBoolean("include_videos", next.includeVideos)
            .putBoolean("skip_existing", next.skipExisting)
            .putInt("threads", next.threads)
            .putBoolean("lazy_lists", next.lazyLists)
            .putBoolean("album_versions_group", next.albumVersionsGroup)
            .putString("album_explicit", next.albumExplicit.name)
            .putString("album_spatial", next.albumSpatial.name)
            .putString("album_edition", next.albumEdition.name)
            .putString("template", next.template)
            .putString("download_dir", next.downloadDir)
            .putString("theme", next.theme.name)
            .putString("motion", next.motion.name)
            .putBoolean("haptics", next.haptics)
            .putInt("stage", next.stage)
            .putString("player_layout", next.playerLayout.name)
            .putBoolean("layout_everywhere", next.layoutEverywhere)
            .putString("immersive_shade", next.immersiveShade.name)
            .putString("progress_style", next.progressStyle.name)
            .putString("progress_thickness", next.progressThickness.name)
            .putBoolean("progress_animated", next.progressAnimated)
            .putString("mascot_mood", next.mascotMood?.name)
            .putString("mascot_dance", next.mascotDance?.name)
            .putString("mascot_specials", next.mascotSpecials.name)
            .putString("mascot_limbs", next.mascotLimbs.name)
            .putString("mascot_turns", next.mascotTurns.name)
            .putBoolean("mascot_long_press", next.mascotLongPress)
            .putString("mascot_temper", next.mascotTemper.name)
            .putFloat("mascot_x", next.mascotX)
            .putFloat("mascot_y", next.mascotY)
            .putFloat("mascot_scale", next.mascotScale)
            .putString("lyrics_mode", next.lyricsMode.name)
            .putString("lyrics_size", next.lyricsSize.name)
            .putBoolean("lyrics_auto_open", next.lyricsAutoOpen)
            .putBoolean("lyrics_mascot", next.lyricsMascot)
            .putInt("lyrics_offset_ms", next.lyricsOffsetMs.coerceIn(LYRICS_OFFSET_RANGE))
            .putShare(next.share)
            .putString("ai_provider", next.ai.provider)
            .putString("ai_models", JSONObject(next.ai.models).toString())
            .putString("ai_base_urls", JSONObject(next.ai.baseUrls).toString())
            .putBoolean("sound_react_button", next.soundReactButton)
            .putBoolean("sound_layer_chip", next.soundLayerChip)
            .putBoolean("signal_chain", next.signalChain)
            .putString("signal_style", next.signalStyle.key)
            .putBoolean("signal_animate", next.signalAnimate)
            .putBoolean("sound_suggestions", next.soundSuggestions)
            .putInt("tidal_terms_accepted", next.tidalTermsAccepted)
            .putLong("tidal_terms_accepted_at", next.tidalTermsAcceptedAt)
            .apply()
        _settings.value = next
    }

    private fun android.content.SharedPreferences.Editor.putShare(s: ShareSettings) = this
        .putString("share_session", s.sessionName)
        .putString("share_name", s.name)
        .putString("share_color", s.color)
        .putString("share_network", s.network.name)
        .putBoolean("share_fixed_code", s.fixedCode)
        .putString("share_code", s.code)
        .putInt("share_port", s.port)
        .putBoolean("share_party_default", s.partyDefault)
        .putString("share_add", s.add.name)
        .putInt("share_max_per_guest", s.maxPerGuest)
        .putString("share_order", s.order.name)
        .putString("share_skip", s.skip.name)
        .putBoolean("share_playback", s.playback)
        .putString("share_edit", s.edit.name)
        .putBoolean("share_explicit", s.allowExplicit)
        .putBoolean("share_drop_on_leave", s.dropSongsOnLeave)
        .putBoolean("share_notice_joined", s.noticeJoined)
        .putBoolean("share_notice_left", s.noticeLeft)
        .putBoolean("share_notice_added", s.noticeAdded)
        .putBoolean("share_notice_approval", s.noticeApproval)
        .putInt("share_notice_seconds", s.noticeSeconds)
        .putBoolean("share_discover", s.discover)
        .putBoolean("share_discover_everywhere", s.discoverEverywhere)
        .putBoolean("share_announce_lan", s.announceLan)
        .putBoolean("share_bluetooth", s.bluetooth)
        .putBoolean("share_visible_with_guests", s.visibleWithGuests)
        .putString("share_group_name", s.groupName)
        .putString("share_group_pass", s.groupPass)
        .putBoolean("share_play_along", s.playAlong)
        .putInt("share_speaker_delay", s.speakerDelayMs)
        .putBoolean("share_sync_by_speed", s.syncBySpeed)
        .putBoolean("share_preload_songs", s.preloadSongs)
        .putBoolean("share_guest_opens_player", s.guestOpensPlayer)
        .putInt("share_party_start_wait", s.partyStartWaitSec)
        .putBoolean("share_send_levels", s.sendLevels)

    /** The user accepted the current personal-use notice. */
    fun acceptTerms() = update { it.copy(tidalTermsAccepted = TIDAL_TERMS_VERSION, tidalTermsAcceptedAt = System.currentTimeMillis()) }

    fun updateAi(transform: (AiSettings) -> AiSettings) = update { it.copy(ai = transform(it.ai)) }

    // ------------------------------------------------------------ equalizer

    private val _eq = MutableStateFlow(prefs.getString("eq_settings", null)?.let(::eqSettingsFromJson) ?: eqDefaultSettings())
    /** On/off, automatic preamp and the profiles per output (the core's JSON, as on the desktop). */
    val eq: StateFlow<EqSettings> = _eq.asStateFlow()

    /** In memory at once (the audio follows); [saveEq] writes it. */
    fun setEq(settings: EqSettings) {
        _eq.value = settings
    }

    fun saveEq() = prefs.edit().putString("eq_settings", eqSettingsToJson(_eq.value)).apply()

    private val _eqPresets = MutableStateFlow(loadPresets())
    /** Profiles the user saved by name. */
    val eqPresets: StateFlow<Map<String, EqProfile>> = _eqPresets.asStateFlow()

    // kept in the settings' own JSON (its per-output map, by name), so profiles have one format
    private fun loadPresets(): Map<String, EqProfile> =
        prefs.getString("eq_presets", null)?.let { eqSettingsFromJson(it).perOutput } ?: emptyMap()

    fun setEqPresets(presets: Map<String, EqProfile>) {
        _eqPresets.value = presets
        val holder = eqDefaultSettings().copy(perOutput = presets)
        prefs.edit().putString("eq_presets", eqSettingsToJson(holder)).apply()
    }

    // ------------------------------------------------- taste layers, loudness

    private val _taste = MutableStateFlow(
        prefs.getString("taste_settings", null)?.let { runCatching { tasteSettingsFromJson(it) }.getOrNull() }
            ?: tasteDefaultSettings(),
    )
    /** Taste rules (a tilt per genre, artist, album, playlist or track), the core's JSON as on the desktop. */
    val taste: StateFlow<TasteSettings> = _taste.asStateFlow()

    fun setTaste(taste: TasteSettings) {
        _taste.value = taste
        prefs.edit().putString("taste_settings", tasteSettingsToJson(taste)).apply()
    }

    private val _loudness = MutableStateFlow(
        prefs.getString("loudness_settings", null)?.let { runCatching { loudnessSettingsFromJson(it) }.getOrNull() }
            ?: loudnessDefaultSettings(),
    )
    /** Loudness normalization (off by default: bit-perfect). */
    val loudness: StateFlow<LoudnessSettings> = _loudness.asStateFlow()

    fun setLoudness(loudness: LoudnessSettings) {
        _loudness.value = loudness
        prefs.edit().putString("loudness_settings", loudnessSettingsToJson(loudness)).apply()
    }

    // ------------------------------------------------------------ crossfade

    private val _crossfade = MutableStateFlow(
        prefs.getString("crossfade_settings", null)?.let { runCatching { crossfadeSettingsFromJson(it) }.getOrNull() }
            ?: crossfadeDefaultSettings(),
    )
    /** Crossfade between songs (off by default: gapless, bit-perfect); the desktop's `crossfade` JSON. */
    val crossfade: StateFlow<CrossfadeSettings> = _crossfade.asStateFlow()

    fun setCrossfade(crossfade: CrossfadeSettings) {
        _crossfade.value = crossfade
        prefs.edit().putString("crossfade_settings", crossfadeSettingsToJson(crossfade)).apply()
    }

    /** Shortcut for the share section. */
    fun updateShare(transform: (ShareSettings) -> ShareSettings) = update { it.copy(share = transform(it.share)) }
}
