package com.perseusoft.homefi.data

import org.json.JSONArray
import org.json.JSONObject

/** Which UI HomeFi opens: follow the device, or force the touch app or the TV app. */
enum class UiForm { AUTO, PHONE, TV }

/** What the TV's ambient mode (screensaver while music plays) shows. */
enum class AmbientStyle { COVER, KARAOKE, MASCOT, CLOCK }

/** What the TV player's side panel shows first. */
enum class PlayerPanel { LYRICS, MASCOT, VISUALIZER, QUEUE }

/** DJ mode on the TV: AUTO shows it only while a mouse is connected. */
enum class TvDjEntry { AUTO, ON, OFF }

/** Where a TV's downloads go: the internal storage, a USB drive, or ask every time. */
enum class DownloadTarget { INTERNAL, USB, ASK }

/** Defaults, ranges and steps of [TvSettings] in one place (the settings screen and `clamped()` read them). */
object TvDefaults {
    val UI_FORM = UiForm.AUTO

    const val AMBIENT_SECONDS = 60
    val AMBIENT_SECONDS_RANGE = 0..600
    const val AMBIENT_SECONDS_STEP = 30
    val AMBIENT_STYLE = AmbientStyle.COVER

    const val KEEP_AWAKE_WHILE_PLAYING = true
    const val AWAKE_AFTER_PAUSE_MINUTES = 5
    val AWAKE_AFTER_PAUSE_MINUTES_RANGE = 1..30

    /** 0 = the controls never hide (the default: they stay on screen). */
    const val CONTROLS_HIDE_SECONDS = 0
    val CONTROLS_HIDE_SECONDS_RANGE = 0..15

    const val DPAD_SEEK_SECONDS = 10
    val DPAD_SEEK_SECONDS_RANGE = 5..60

    val PLAYER_PANEL = PlayerPanel.LYRICS
    const val PLAYER_LAYOUT_COLOURS = true
    const val SAFE_MARGINS = true

    const val TEXT_SCALE = 1.0f
    val TEXT_SCALE_RANGE = 0.9f..1.3f

    val DJ_ENTRY = TvDjEntry.AUTO
    const val VOICE_INPUT = true

    val DOWNLOAD_TARGET = DownloadTarget.INTERNAL
    /** Free space a download target keeps (the core's `download::space` rule: default 500, 100..=5000). */
    const val MIN_FREE_MB = 500
    val MIN_FREE_MB_RANGE = 100..5000

    const val STREAM_CACHE_MB = 1024
    val STREAM_CACHE_MB_RANGE = 256..4096

    /** How far one UP / DOWN press moves an equalizer band on the TV, in dB. */
    const val EQ_STEP_DB = 1.0f
    val EQ_STEP_DB_RANGE = 0.5f..3.0f

    /** Like the phone: a guest's songs wait for the host's yes. */
    const val PARTY_AUTO_APPROVE = false
    /** The Android TV home-screen "Continue listening" row (phase 6). */
    const val CHANNELS = false
    /** A new version downloads by itself; installing then asks the one system OK. */
    const val AUTO_DOWNLOAD_UPDATES = true

    /** A ride sent from a phone while the user is elsewhere: the banner waits this long, then opens it (0 = only the banner). */
    const val RIDE_AUTO_OPEN_SECONDS = 5
    val RIDE_AUTO_OPEN_SECONDS_RANGE = 0..30
}

/**
 * HomeFi on a TV (Android TV / Google TV), kept as one JSON value in the settings. Both forms
 * show it; on a phone it only matters when HomeFi runs on a TV. Read it with [fromJson], which
 * never fails on old, partial or corrupt data, and save it after [clamped].
 */
data class TvSettings(
    val uiForm: UiForm = TvDefaults.UI_FORM,
    /** Seconds of silence in the player before ambient mode starts; 0 = never. */
    val ambientSeconds: Int = TvDefaults.AMBIENT_SECONDS,
    val ambientStyle: AmbientStyle = TvDefaults.AMBIENT_STYLE,
    /** The screen stays on while music plays. */
    val keepAwakeWhilePlaying: Boolean = TvDefaults.KEEP_AWAKE_WHILE_PLAYING,
    /** After pausing, minutes before the screen may sleep (the system screensaver takes over). */
    val awakeAfterPauseMinutes: Int = TvDefaults.AWAKE_AFTER_PAUSE_MINUTES,
    /** The player's controls fade after this many seconds without a key; 0 keeps them on screen. */
    val controlsHideSeconds: Int = TvDefaults.CONTROLS_HIDE_SECONDS,
    /** Seek step of the remote's left / right (held) and fast-forward / rewind keys. */
    val dpadSeekSeconds: Int = TvDefaults.DPAD_SEEK_SECONDS,
    val playerPanel: PlayerPanel = TvDefaults.PLAYER_PANEL,
    /** The TV player keeps the skin's colours; off, it uses the plain TV palette. */
    val playerLayoutColours: Boolean = TvDefaults.PLAYER_LAYOUT_COLOURS,
    /** Keep content inside the overscan-safe margins (off for TVs without overscan). */
    val safeMargins: Boolean = TvDefaults.SAFE_MARGINS,
    val textScale: Float = TvDefaults.TEXT_SCALE,
    val djEntry: TvDjEntry = TvDefaults.DJ_ENTRY,
    /** A mic button next to search and the AI chats (hidden when no speech service answers). */
    val voiceInput: Boolean = TvDefaults.VOICE_INPUT,
    val downloadTarget: DownloadTarget = TvDefaults.DOWNLOAD_TARGET,
    /** Free MB a download target must keep (core rule); under it, a USB drive is offered. */
    val minFreeMb: Int = TvDefaults.MIN_FREE_MB,
    /** Roots of removable volumes (USB) the library also reads. */
    val libraryVolumes: List<String> = emptyList(),
    /** Tidal stream cache size on the TV. */
    val streamCacheMb: Int = TvDefaults.STREAM_CACHE_MB,
    /** As the party hub, let guests' songs in without asking. */
    val partyAutoApprove: Boolean = TvDefaults.PARTY_AUTO_APPROVE,
    /** dB one UP / DOWN press moves an equalizer band (Sound, D-pad editing). */
    val eqStepDb: Float = TvDefaults.EQ_STEP_DB,
    val channels: Boolean = TvDefaults.CHANNELS,
    /** Download a found update in the background (off: "Update now" downloads it). */
    val autoDownloadUpdates: Boolean = TvDefaults.AUTO_DOWNLOAD_UPDATES,
    val rideAutoOpenSeconds: Int = TvDefaults.RIDE_AUTO_OPEN_SECONDS,
) {
    /** Every number brought inside its range (a hand-edited or older value never breaks the TV UI). */
    fun clamped() = copy(
        ambientSeconds = ambientSeconds.coerceIn(TvDefaults.AMBIENT_SECONDS_RANGE),
        awakeAfterPauseMinutes = awakeAfterPauseMinutes.coerceIn(TvDefaults.AWAKE_AFTER_PAUSE_MINUTES_RANGE),
        controlsHideSeconds = controlsHideSeconds.coerceIn(TvDefaults.CONTROLS_HIDE_SECONDS_RANGE),
        dpadSeekSeconds = dpadSeekSeconds.coerceIn(TvDefaults.DPAD_SEEK_SECONDS_RANGE),
        textScale = textScale.coerceIn(TvDefaults.TEXT_SCALE_RANGE),
        minFreeMb = minFreeMb.coerceIn(TvDefaults.MIN_FREE_MB_RANGE),
        streamCacheMb = streamCacheMb.coerceIn(TvDefaults.STREAM_CACHE_MB_RANGE),
        rideAutoOpenSeconds = rideAutoOpenSeconds.coerceIn(TvDefaults.RIDE_AUTO_OPEN_SECONDS_RANGE),
        eqStepDb = eqStepDb.coerceIn(TvDefaults.EQ_STEP_DB_RANGE),
        libraryVolumes = libraryVolumes.map(String::trim).filter(String::isNotEmpty).distinct(),
    )

    fun toJson(): String = JSONObject()
        .put("uiForm", uiForm.name)
        .put("ambientSeconds", ambientSeconds)
        .put("ambientStyle", ambientStyle.name)
        .put("keepAwakeWhilePlaying", keepAwakeWhilePlaying)
        .put("awakeAfterPauseMinutes", awakeAfterPauseMinutes)
        .put("controlsHideSeconds", controlsHideSeconds)
        .put("dpadSeekSeconds", dpadSeekSeconds)
        .put("playerPanel", playerPanel.name)
        .put("playerLayoutColours", playerLayoutColours)
        .put("safeMargins", safeMargins)
        .put("textScale", textScale.toDouble())
        .put("djEntry", djEntry.name)
        .put("voiceInput", voiceInput)
        .put("downloadTarget", downloadTarget.name)
        .put("minFreeMb", minFreeMb)
        .put("libraryVolumes", JSONArray(libraryVolumes))
        .put("streamCacheMb", streamCacheMb)
        .put("partyAutoApprove", partyAutoApprove)
        .put("eqStepDb", eqStepDb.toDouble())
        .put("channels", channels)
        .put("autoDownloadUpdates", autoDownloadUpdates)
        .put("rideAutoOpenSeconds", rideAutoOpenSeconds)
        .toString()

    companion object {
        /**
         * Lenient: null, empty, corrupt or partial JSON gives the defaults for whatever is
         * missing or of the wrong type, unknown keys and enum names are ignored. Always clamped.
         */
        fun fromJson(raw: String?): TvSettings {
            val d = TvSettings()
            val o = raw?.takeIf { it.isNotBlank() }?.let { runCatching { JSONObject(it) }.getOrNull() } ?: return d
            return TvSettings(
                uiForm = o.enum("uiForm", d.uiForm),
                ambientSeconds = o.int("ambientSeconds", d.ambientSeconds),
                ambientStyle = o.enum("ambientStyle", d.ambientStyle),
                keepAwakeWhilePlaying = o.bool("keepAwakeWhilePlaying", d.keepAwakeWhilePlaying),
                awakeAfterPauseMinutes = o.int("awakeAfterPauseMinutes", d.awakeAfterPauseMinutes),
                controlsHideSeconds = o.int("controlsHideSeconds", d.controlsHideSeconds),
                dpadSeekSeconds = o.int("dpadSeekSeconds", d.dpadSeekSeconds),
                playerPanel = o.enum("playerPanel", d.playerPanel),
                playerLayoutColours = o.bool("playerLayoutColours", d.playerLayoutColours),
                safeMargins = o.bool("safeMargins", d.safeMargins),
                textScale = o.float("textScale", d.textScale),
                djEntry = o.enum("djEntry", d.djEntry),
                voiceInput = o.bool("voiceInput", d.voiceInput),
                downloadTarget = o.enum("downloadTarget", d.downloadTarget),
                minFreeMb = o.int("minFreeMb", d.minFreeMb),
                libraryVolumes = o.strings("libraryVolumes"),
                streamCacheMb = o.int("streamCacheMb", d.streamCacheMb),
                partyAutoApprove = o.bool("partyAutoApprove", d.partyAutoApprove),
                eqStepDb = o.float("eqStepDb", d.eqStepDb),
                channels = o.bool("channels", d.channels),
                autoDownloadUpdates = o.bool("autoDownloadUpdates", d.autoDownloadUpdates),
                rideAutoOpenSeconds = o.int("rideAutoOpenSeconds", d.rideAutoOpenSeconds),
            ).clamped()
        }
    }
}

// Typed reads that fall back to the default when the key is missing, null or of another type.

private fun JSONObject.bool(key: String, default: Boolean): Boolean = (opt(key) as? Boolean) ?: default

private fun JSONObject.int(key: String, default: Int): Int = (opt(key) as? Number)?.toInt() ?: default

private fun JSONObject.float(key: String, default: Float): Float =
    (opt(key) as? Number)?.toFloat()?.takeIf { it.isFinite() } ?: default

private inline fun <reified E : Enum<E>> JSONObject.enum(key: String, default: E): E =
    (opt(key) as? String)?.let { name -> enumValues<E>().firstOrNull { it.name.equals(name, ignoreCase = true) } } ?: default

private fun JSONObject.strings(key: String): List<String> {
    val array = optJSONArray(key) ?: return emptyList()
    return List(array.length()) { array.opt(it) as? String }.filterNotNull()
}
