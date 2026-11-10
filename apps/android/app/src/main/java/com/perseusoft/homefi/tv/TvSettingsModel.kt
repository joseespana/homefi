package com.perseusoft.homefi.tv

import androidx.annotation.StringRes
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material.icons.rounded.Cloud
import androidx.compose.material.icons.rounded.Download
import androidx.compose.material.icons.rounded.Groups
import androidx.compose.material.icons.rounded.Info
import androidx.compose.material.icons.rounded.Lyrics
import androidx.compose.material.icons.rounded.Palette
import androidx.compose.material.icons.rounded.PlayCircle
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.material.icons.rounded.Tv
import androidx.compose.ui.graphics.vector.ImageVector
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.AlbumEditionPref
import com.perseusoft.homefi.core.AlbumExplicitPref
import com.perseusoft.homefi.core.AlbumSpatialPref
import com.perseusoft.homefi.core.AudioQuality
import com.perseusoft.homefi.core.ShareAddMode
import com.perseusoft.homefi.core.ShareEditMode
import com.perseusoft.homefi.core.ShareQueueOrder
import com.perseusoft.homefi.core.ShareSkipMode
import com.perseusoft.homefi.data.AmbientStyle
import com.perseusoft.homefi.data.AppSettings
import com.perseusoft.homefi.data.DownloadTarget
import com.perseusoft.homefi.data.LyricsMode
import com.perseusoft.homefi.data.LyricsSize
import com.perseusoft.homefi.data.Motion
import com.perseusoft.homefi.data.PlayerLayout
import com.perseusoft.homefi.data.ProgressStyle
import com.perseusoft.homefi.data.ProgressThickness
import com.perseusoft.homefi.data.PlayerPanel
import com.perseusoft.homefi.data.SignalStyle
import com.perseusoft.homefi.data.ThemeMode
import com.perseusoft.homefi.data.TvDjEntry
import com.perseusoft.homefi.data.UiForm
import com.perseusoft.homefi.ui.settings.label
import com.perseusoft.homefi.ui.settings.labelRes
import com.perseusoft.homefi.ui.player.label
import com.perseusoft.homefi.ui.player.lyricsOffsetText

// The TV's settings as data (docs/TV_UI.md 6.9): every row says how to read its value from `AppSettings`
// and how to write it back, so one screen draws all of them and a test can walk them without a device.
// The values themselves live in the phone's `SettingsStore`: nothing here keeps a second copy.

/** The words of one option: a string resource (with arguments) or text that is already final. */
sealed interface TvLabel {
    data class Res(@StringRes val id: Int, val args: List<Any> = emptyList()) : TvLabel
    data class Plain(val text: String) : TvLabel
}

/** What an action row does when OK is pressed; the screen decides how (a dialog, an intent). */
enum class TvSettingAction { SIGN_OUT, USB_ACCESS, FORGET_DRIVES, CHECK_UPDATES, WHATS_NEW }

/** One line of the right pane. [enabled] greys a row out while another setting makes it pointless. */
sealed interface TvSettingRow {
    val id: String
    @get:StringRes val title: Int
    @get:StringRes val hint: Int?
    val enabled: (AppSettings) -> Boolean

    class Switch(
        override val id: String,
        override val title: Int,
        override val hint: Int? = null,
        val get: (AppSettings) -> Boolean,
        val set: (AppSettings, Boolean) -> AppSettings,
        override val enabled: (AppSettings) -> Boolean = ALWAYS,
    ) : TvSettingRow

    /** A value out of a short list: LEFT / RIGHT cycles, OK opens the list when it is long. */
    class Choice(
        override val id: String,
        override val title: Int,
        override val hint: Int? = null,
        val options: List<TvLabel>,
        val selected: (AppSettings) -> Int,
        val select: (AppSettings, Int) -> AppSettings,
        override val enabled: (AppSettings) -> Boolean = ALWAYS,
    ) : TvSettingRow

    /** Free text typed with the TV keyboard ([secret] shows dots). */
    class Text(
        override val id: String,
        override val title: Int,
        override val hint: Int? = null,
        val get: (AppSettings) -> String,
        val set: (AppSettings, String) -> AppSettings,
        val maxLength: Int = DEFAULT_TEXT_LENGTH,
        @StringRes val emptyLabel: Int = R.string.tv_set_value_default,
        override val enabled: (AppSettings) -> Boolean = ALWAYS,
    ) : TvSettingRow

    class Action(
        override val id: String,
        override val title: Int,
        override val hint: Int? = null,
        val action: TvSettingAction,
        /** What the row shows on its right (a count, a state), or null. */
        val value: ((AppSettings) -> TvLabel?)? = null,
        override val enabled: (AppSettings) -> Boolean = ALWAYS,
    ) : TvSettingRow
}

private val ALWAYS: (AppSettings) -> Boolean = { true }

/** A text field of a name or a template never needs more than this many characters. */
const val DEFAULT_TEXT_LENGTH = 64

/** A category of the left pane. [rows] is empty for the AI page, which has its own screen. */
enum class TvSettingsCategory(@StringRes val title: Int, @StringRes val about: Int, val icon: ImageVector) {
    APPEARANCE(R.string.tv_set_cat_look, R.string.tv_set_cat_look_about, Icons.Rounded.Palette),
    PLAYER(R.string.tv_set_cat_player, R.string.tv_set_cat_player_about, Icons.Rounded.PlayCircle),
    SOUND(R.string.tv_set_cat_sound, R.string.tv_set_cat_sound_about, Icons.Rounded.Tune),
    DOWNLOADS(R.string.tv_set_cat_downloads, R.string.tv_set_cat_downloads_about, Icons.Rounded.Download),
    TIDAL(R.string.tv_set_cat_tidal, R.string.tv_set_cat_tidal_about, Icons.Rounded.Cloud),
    PARTY(R.string.tv_set_cat_party, R.string.tv_set_cat_party_about, Icons.Rounded.Groups),
    TV(R.string.tv_set_cat_tv, R.string.tv_set_cat_tv_about, Icons.Rounded.Tv),
    LYRICS(R.string.tv_set_cat_lyrics, R.string.tv_set_cat_lyrics_about, Icons.Rounded.Lyrics),
    AI(R.string.settings_ai, R.string.tv_set_cat_ai_about, Icons.Rounded.AutoAwesome),
    ABOUT(R.string.tv_set_cat_about, R.string.tv_set_cat_about_about, Icons.Rounded.Info),
}

/** The rows of [category]; empty for the AI page. */
fun settingsRows(category: TvSettingsCategory): List<TvSettingRow> = when (category) {
    TvSettingsCategory.APPEARANCE -> appearanceRows
    TvSettingsCategory.PLAYER -> playerRows
    TvSettingsCategory.SOUND -> soundRows
    TvSettingsCategory.DOWNLOADS -> downloadRows
    TvSettingsCategory.TIDAL -> tidalRows
    TvSettingsCategory.PARTY -> partyRows
    TvSettingsCategory.TV -> tvRows
    TvSettingsCategory.LYRICS -> lyricsRows
    TvSettingsCategory.AI -> emptyList()
    TvSettingsCategory.ABOUT -> aboutRows
}

// ------------------------------------------------------------------ builders

private fun res(@StringRes id: Int, vararg args: Any) = TvLabel.Res(id, args.toList())

/** An enum as a [TvSettingRow.Choice], its options in [entries] order. */
private fun <E : Enum<E>> enumRow(
    id: String,
    @StringRes title: Int,
    entries: List<E>,
    label: (E) -> Int,
    get: (AppSettings) -> E,
    set: (AppSettings, E) -> AppSettings,
    @StringRes hint: Int? = null,
    enabled: (AppSettings) -> Boolean = ALWAYS,
) = TvSettingRow.Choice(
    id, title, hint,
    options = entries.map { res(label(it)) },
    selected = { s -> entries.indexOf(get(s)).coerceAtLeast(0) },
    select = { s, i -> entries.getOrNull(i)?.let { set(s, it) } ?: s },
    enabled = enabled,
)

/** The index of the entry of [values] closest to [value]: a stored value between two steps still shows one. */
internal fun <N : Number> nearestIndex(values: List<N>, value: N): Int =
    values.indices.minByOrNull { kotlin.math.abs(values[it].toDouble() - value.toDouble()) } ?: 0

/** A number out of a list of steps; the stored value shows as the nearest step. */
private fun <N : Number> numberRow(
    id: String,
    @StringRes title: Int,
    values: List<N>,
    label: (N) -> TvLabel,
    get: (AppSettings) -> N,
    set: (AppSettings, N) -> AppSettings,
    @StringRes hint: Int? = null,
    enabled: (AppSettings) -> Boolean = ALWAYS,
) = TvSettingRow.Choice(
    id, title, hint,
    options = values.map(label),
    selected = { s -> nearestIndex(values, get(s)) },
    select = { s, i -> values.getOrNull(i)?.let { set(s, it) } ?: s },
    enabled = enabled,
)

private fun switchRow(
    id: String,
    @StringRes title: Int,
    get: (AppSettings) -> Boolean,
    set: (AppSettings, Boolean) -> AppSettings,
    @StringRes hint: Int? = null,
    enabled: (AppSettings) -> Boolean = ALWAYS,
) = TvSettingRow.Switch(id, title, hint, get, set, enabled)

private fun seconds(n: Int): TvLabel = res(R.string.tv_set_seconds, n)
private fun minutes(n: Int): TvLabel = res(R.string.tv_set_minutes, n)
private fun megabytes(n: Int): TvLabel = if (n >= MB_PER_GB) res(R.string.tv_set_gb, n / MB_PER_GB) else res(R.string.tv_set_mb, n)
private fun percent(f: Float): TvLabel = res(R.string.tv_set_percent, Math.round(f * PERCENT))
private fun decibels(f: Float): TvLabel = TvLabel.Plain(String.format(java.util.Locale.ROOT, "%.1f dB", f))

private const val MB_PER_GB = 1024
private const val PERCENT = 100

/** Seconds of the screensaver wait, 0 = off, in the range of [com.perseusoft.homefi.data.TvDefaults.AMBIENT_SECONDS_RANGE]. */
private val AMBIENT_STEPS = listOf(0, 30, 60, 90, 120, 180, 300, 600)
private val AWAKE_STEPS = listOf(1, 2, 3, 5, 10, 15, 20, 30)

/** 0 = the controls never hide. */
private val CONTROLS_STEPS = listOf(0, 2, 3, 5, 8, 10, 15)
private val SEEK_STEPS = listOf(5, 10, 15, 20, 30, 45, 60)
private val TEXT_SCALES = listOf(0.9f, 1.0f, 1.1f, 1.2f, 1.3f)
private val FREE_MB_STEPS = listOf(100, 250, 500, 1000, 2000, 5000)
private val STREAM_CACHE_STEPS = listOf(256, 512, 1024, 2048, 4096)
private val EQ_STEPS = listOf(0.5f, 1.0f, 1.5f, 2.0f, 3.0f)
private val RIDE_OPEN_STEPS = listOf(0, 3, 5, 10, 15, 30)
private val LYRICS_OFFSET_STEPS = (-2000..2000 step 250).toList()
private val THREAD_STEPS = (1..6).toList()
private val MAX_PER_GUEST_STEPS = (1..10).toList()
private val NOTICE_STEPS = listOf(2, 3, 4, 6, 8, 10)
private val SPEAKER_DELAY_STEPS = (-500..500 step 50).toList()
private val START_WAIT_STEPS = listOf(0, 1, 2, 3, 5, 8, 10)

// ------------------------------------------------------------------ the pages

private val appearanceRows = listOf(
    enumRow("theme", R.string.settings_appearance, ThemeMode.entries, { it.label() }, { it.theme }, { s, v -> s.copy(theme = v) }),
    enumRow("motion", R.string.settings_motion, Motion.entries, { it.labelRes() }, { it.motion }, { s, v -> s.copy(motion = v) }, R.string.settings_motion_hint),
    enumRow("skin", R.string.layout_title, PlayerLayout.entries, { it.labelRes() }, { it.playerLayout }, { s, v -> s.copy(playerLayout = v) }),
    switchRow("skinEverywhere", R.string.layout_everywhere, { it.layoutEverywhere }, { s, v -> s.copy(layoutEverywhere = v) }, R.string.layout_everywhere_hint),
    enumRow("progressStyle", R.string.tv_set_progress_style, ProgressStyle.entries, { it.labelRes() }, { it.progressStyle }, { s, v -> s.copy(progressStyle = v) }),
    enumRow("progressThickness", R.string.progress_thickness, ProgressThickness.entries, { it.labelRes() }, { it.progressThickness }, { s, v -> s.copy(progressThickness = v) }),
    switchRow("progressAnimated", R.string.progress_animated, { it.progressAnimated }, { s, v -> s.copy(progressAnimated = v) }),
    numberRow("tv.textScale", R.string.tv_set_text_size, TEXT_SCALES, ::percent, { it.tv.textScale }, { s, v -> s.copy(tv = s.tv.copy(textScale = v)) }, R.string.tv_set_text_size_hint),
    switchRow("tv.safeMargins", R.string.tv_set_safe_margins, { it.tv.safeMargins }, { s, v -> s.copy(tv = s.tv.copy(safeMargins = v)) }, R.string.tv_set_safe_margins_hint),
)

private val playerRows = listOf(
    enumRow("tv.playerPanel", R.string.tv_set_player_panel, PlayerPanel.entries, ::panelLabel, { it.tv.playerPanel }, { s, v -> s.copy(tv = s.tv.copy(playerPanel = v)) }, R.string.tv_set_player_panel_hint),
    switchRow("tv.playerLayoutColours", R.string.tv_set_player_colours, { it.tv.playerLayoutColours }, { s, v -> s.copy(tv = s.tv.copy(playerLayoutColours = v)) }, R.string.tv_set_player_colours_hint),
    numberRow(
        "tv.controlsHideSeconds", R.string.tv_set_controls_hide, CONTROLS_STEPS,
        { if (it == 0) res(R.string.tv_set_never) else seconds(it) },
        { it.tv.controlsHideSeconds }, { s, v -> s.copy(tv = s.tv.copy(controlsHideSeconds = v)) }, R.string.tv_set_controls_hide_hint,
    ),
    numberRow("tv.dpadSeekSeconds", R.string.tv_set_seek, SEEK_STEPS, ::seconds, { it.tv.dpadSeekSeconds }, { s, v -> s.copy(tv = s.tv.copy(dpadSeekSeconds = v)) }, R.string.tv_set_seek_hint),
    switchRow("signalAnimate", R.string.signal_animate, { it.signalAnimate }, { s, v -> s.copy(signalAnimate = v) }, enabled = { it.signalChain }),
    TvSettingRow.Choice(
        "signalChain", R.string.signal_chain, R.string.signal_chain_hint,
        options = listOf(res(R.string.signal_style_compact), res(R.string.signal_style_detailed), res(R.string.signal_style_off)),
        // three states over two settings: a style, or off
        selected = { s -> if (!s.signalChain) 2 else SignalStyle.entries.indexOf(s.signalStyle).coerceAtLeast(0) },
        select = { s, i ->
            when {
                i == SignalStyle.entries.size -> s.copy(signalChain = false)
                i in SignalStyle.entries.indices -> s.copy(signalChain = true, signalStyle = SignalStyle.entries[i])
                else -> s
            }
        },
    ),
    switchRow("soundReactButton", R.string.sound_settings_react_button, { it.soundReactButton }, { s, v -> s.copy(soundReactButton = v) }, R.string.sound_settings_react_button_hint),
    switchRow("soundLayerChip", R.string.sound_settings_layer_chip, { it.soundLayerChip }, { s, v -> s.copy(soundLayerChip = v) }, R.string.sound_settings_layer_chip_hint),
)

private val soundRows = listOf(
    numberRow("tv.eqStepDb", R.string.tv_set_eq_step, EQ_STEPS, ::decibels, { it.tv.eqStepDb }, { s, v -> s.copy(tv = s.tv.copy(eqStepDb = v)) }, R.string.tv_set_eq_step_hint),
    switchRow("soundButton", R.string.tv_set_sound_button, { it.soundButton }, { s, v -> s.copy(soundButton = v) }),
    switchRow("soundTiltSliders", R.string.tv_set_tilt, { it.soundTiltSliders }, { s, v -> s.copy(soundTiltSliders = v) }),
    switchRow("soundSuggestions", R.string.sound_settings_suggestions, { it.soundSuggestions }, { s, v -> s.copy(soundSuggestions = v) }, R.string.sound_settings_suggestions_hint),
    switchRow("dj.simple", R.string.dj_set_simple, { it.djSimpleView }, { s, v -> s.copy(djSimpleView = v) }, R.string.dj_set_simple_hint),
    switchRow("dj.tips", R.string.dj_set_tips, { it.djShowTips }, { s, v -> s.copy(djShowTips = v) }),
    switchRow("dj.bassSwap", R.string.dj_set_bass_swap, { it.djBassSwap }, { s, v -> s.copy(djBassSwap = v) }, R.string.dj_set_bass_swap_hint),
    switchRow("dj.keyLock", R.string.dj_key_lock, { it.djKeyLock }, { s, v -> s.copy(djKeyLock = v) }, R.string.dj_set_key_lock_hint),
    enumRow(
        "tv.djEntry", R.string.tv_set_dj_entry, TvDjEntry.entries,
        { when (it) { TvDjEntry.AUTO -> R.string.tv_set_dj_auto; TvDjEntry.ON -> R.string.tv_set_on; TvDjEntry.OFF -> R.string.tv_set_off } },
        { it.tv.djEntry }, { s, v -> s.copy(tv = s.tv.copy(djEntry = v)) }, R.string.tv_set_dj_entry_hint,
    ),
)

/** Tags a download carries: listed under Downloads and under Lyrics, one row. */
private val embedLyricsRow = switchRow("embedLyrics", R.string.settings_embed_lyrics, { it.embedLyrics }, { s, v -> s.copy(embedLyrics = v) })

private fun panelLabel(panel: PlayerPanel): Int = when (panel) {
    PlayerPanel.LYRICS -> R.string.tv_set_panel_lyrics
    PlayerPanel.MASCOT -> R.string.tv_set_panel_mascot
    PlayerPanel.VISUALIZER -> R.string.tv_set_panel_visualizer
    PlayerPanel.QUEUE -> R.string.tv_set_panel_queue
}

private val downloadRows = listOf(
    enumRow("quality", R.string.settings_quality, AudioQuality.entries, { it.label() }, { it.quality }, { s, v -> s.copy(quality = v) }, R.string.settings_quality_hint),
    enumRow(
        "tv.downloadTarget", R.string.tv_set_download_target, DownloadTarget.entries,
        { when (it) { DownloadTarget.INTERNAL -> R.string.tv_dl_internal; DownloadTarget.USB -> R.string.tv_dl_usb; DownloadTarget.ASK -> R.string.tv_dl_ask } },
        { it.tv.downloadTarget }, { s, v -> s.copy(tv = s.tv.copy(downloadTarget = v)) }, R.string.tv_set_download_target_hint,
    ),
    numberRow("tv.minFreeMb", R.string.tv_set_min_free, FREE_MB_STEPS, ::megabytes, { it.tv.minFreeMb }, { s, v -> s.copy(tv = s.tv.copy(minFreeMb = v)) }, R.string.tv_set_min_free_hint),
    TvSettingRow.Action("usbAccess", R.string.tv_set_usb_access, R.string.tv_set_usb_access_hint, TvSettingAction.USB_ACCESS),
    TvSettingRow.Action(
        "tv.libraryVolumes", R.string.tv_set_forget_drives, R.string.tv_set_forget_drives_hint, TvSettingAction.FORGET_DRIVES,
        value = { s -> res(R.string.tv_set_count, s.tv.libraryVolumes.size) },
        enabled = { it.tv.libraryVolumes.isNotEmpty() },
    ),
    numberRow("tv.streamCacheMb", R.string.tv_set_stream_cache, STREAM_CACHE_STEPS, ::megabytes, { it.tv.streamCacheMb }, { s, v -> s.copy(tv = s.tv.copy(streamCacheMb = v)) }, R.string.tv_set_stream_cache_hint),
    switchRow("embedCover", R.string.settings_embed_cover, { it.embedCover }, { s, v -> s.copy(embedCover = v) }),
    embedLyricsRow,
    switchRow("includeVideos", R.string.settings_videos, { it.includeVideos }, { s, v -> s.copy(includeVideos = v) }),
    switchRow("skipExisting", R.string.settings_skip_existing, { it.skipExisting }, { s, v -> s.copy(skipExisting = v) }),
    numberRow("threads", R.string.tv_set_threads, THREAD_STEPS, { res(R.string.tv_set_count, it) }, { it.threads }, { s, v -> s.copy(threads = v) }),
    TvSettingRow.Text(
        "template", R.string.settings_template, null, { it.template }, { s, v -> s.copy(template = v) },
        maxLength = TEMPLATE_LENGTH,
    ),
)

/** A file name template is longer than a name: `{album.artist}/{album.title}/{item.number:02d}. {item.title}` is 70. */
private const val TEMPLATE_LENGTH = 160

private val tidalRows = listOf(
    TvSettingRow.Action("account", R.string.tv_set_account, null, TvSettingAction.SIGN_OUT),
    switchRow("lazyLists", R.string.settings_lazy_lists, { it.lazyLists }, { s, v -> s.copy(lazyLists = v) }, R.string.settings_lazy_lists_hint),
    switchRow("albumVersionsGroup", R.string.settings_album_group, { it.albumVersionsGroup }, { s, v -> s.copy(albumVersionsGroup = v) }, R.string.settings_album_group_hint),
    enumRow(
        "albumExplicit", R.string.settings_album_explicit, AlbumExplicitPref.entries,
        { if (it == AlbumExplicitPref.EXPLICIT) R.string.album_pref_explicit else R.string.album_pref_clean },
        { it.albumExplicit }, { s, v -> s.copy(albumExplicit = v) }, R.string.settings_album_explicit_hint, enabled = { it.albumVersionsGroup },
    ),
    enumRow(
        "albumSpatial", R.string.settings_album_spatial, AlbumSpatialPref.entries,
        { if (it == AlbumSpatialPref.STEREO) R.string.album_pref_stereo else R.string.album_pref_atmos },
        { it.albumSpatial }, { s, v -> s.copy(albumSpatial = v) }, R.string.settings_album_spatial_hint, enabled = { it.albumVersionsGroup },
    ),
    enumRow(
        "albumEdition", R.string.settings_album_edition, AlbumEditionPref.entries,
        { if (it == AlbumEditionPref.ORIGINAL) R.string.album_pref_original else R.string.album_pref_remaster },
        { it.albumEdition }, { s, v -> s.copy(albumEdition = v) }, R.string.settings_album_edition_hint, enabled = { it.albumVersionsGroup },
    ),
)

private val partyRows = listOf(
    switchRow("tv.partyAutoApprove", R.string.tv_set_party_auto, { it.tv.partyAutoApprove }, { s, v -> s.copy(tv = s.tv.copy(partyAutoApprove = v)) }, R.string.tv_set_party_auto_hint),
    TvSettingRow.Text("share.name", R.string.sharing_your_name, null, { it.share.name }, { s, v -> s.copy(share = s.share.copy(name = v)) }, maxLength = SHARE_NAME_LENGTH),
    TvSettingRow.Text("share.sessionName", R.string.sharing_session_name, null, { it.share.sessionName }, { s, v -> s.copy(share = s.share.copy(sessionName = v)) }, maxLength = SHARE_NAME_LENGTH),
    enumRow("share.add", R.string.sharing_perm_add, ShareAddMode.entries, { it.labelRes() }, { it.share.add }, { s, v -> s.copy(share = s.share.copy(add = v)) }),
    numberRow("share.maxPerGuest", R.string.tv_set_max_per_guest, MAX_PER_GUEST_STEPS, { res(R.string.tv_set_count, it) }, { it.share.maxPerGuest }, { s, v -> s.copy(share = s.share.copy(maxPerGuest = v)) }),
    enumRow("share.order", R.string.sharing_perm_order, ShareQueueOrder.entries, { it.labelRes() }, { it.share.order }, { s, v -> s.copy(share = s.share.copy(order = v)) }),
    enumRow("share.skip", R.string.sharing_perm_skip, ShareSkipMode.entries, { it.labelRes() }, { it.share.skip }, { s, v -> s.copy(share = s.share.copy(skip = v)) }),
    enumRow("share.edit", R.string.sharing_perm_edit, ShareEditMode.entries, { it.labelRes() }, { it.share.edit }, { s, v -> s.copy(share = s.share.copy(edit = v)) }),
    switchRow("share.playback", R.string.sharing_perm_playback, { it.share.playback }, { s, v -> s.copy(share = s.share.copy(playback = v)) }),
    switchRow("share.allowExplicit", R.string.sharing_perm_explicit, { it.share.allowExplicit }, { s, v -> s.copy(share = s.share.copy(allowExplicit = v)) }),
    switchRow("share.dropSongsOnLeave", R.string.sharing_perm_drop, { it.share.dropSongsOnLeave }, { s, v -> s.copy(share = s.share.copy(dropSongsOnLeave = v)) }),
    switchRow("share.noticeJoined", R.string.sharing_notice_joined_setting, { it.share.noticeJoined }, { s, v -> s.copy(share = s.share.copy(noticeJoined = v)) }),
    switchRow("share.noticeLeft", R.string.sharing_notice_left_setting, { it.share.noticeLeft }, { s, v -> s.copy(share = s.share.copy(noticeLeft = v)) }),
    switchRow("share.noticeAdded", R.string.sharing_notice_added_setting, { it.share.noticeAdded }, { s, v -> s.copy(share = s.share.copy(noticeAdded = v)) }),
    switchRow("share.noticeApproval", R.string.sharing_notice_approval_setting, { it.share.noticeApproval }, { s, v -> s.copy(share = s.share.copy(noticeApproval = v)) }),
    numberRow("share.noticeSeconds", R.string.tv_set_notice_seconds, NOTICE_STEPS, ::seconds, { it.share.noticeSeconds }, { s, v -> s.copy(share = s.share.copy(noticeSeconds = v)) }),
    numberRow(
        "share.partyStartWaitSec", R.string.tv_set_start_wait, START_WAIT_STEPS,
        { if (it == 0) res(R.string.tv_set_off) else seconds(it) },
        { it.share.partyStartWaitSec }, { s, v -> s.copy(share = s.share.copy(partyStartWaitSec = v)) }, R.string.sharing_start_wait_hint,
    ),
    numberRow(
        "share.speakerDelayMs", R.string.tv_set_speaker_delay, SPEAKER_DELAY_STEPS, { TvLabel.Plain("$it ms") },
        { it.share.speakerDelayMs }, { s, v -> s.copy(share = s.share.copy(speakerDelayMs = v)) }, R.string.sharing_speaker_delay_setting_hint,
    ),
    switchRow("share.sendLevels", R.string.sharing_send_levels, { it.share.sendLevels }, { s, v -> s.copy(share = s.share.copy(sendLevels = v)) }, R.string.sharing_send_levels_hint),
)

/** The phone keeps a name to 32 characters. */
private const val SHARE_NAME_LENGTH = 32

private val tvRows = listOf(
    enumRow(
        "tv.uiForm", R.string.tv_set_ui_form, UiForm.entries,
        { when (it) { UiForm.AUTO -> R.string.tv_set_ui_auto; UiForm.PHONE -> R.string.tv_set_ui_phone; UiForm.TV -> R.string.tv_set_ui_tv } },
        { it.tv.uiForm }, { s, v -> s.copy(tv = s.tv.copy(uiForm = v)) }, R.string.tv_set_ui_form_hint,
    ),
    numberRow(
        "tv.ambientSeconds", R.string.tv_set_ambient, AMBIENT_STEPS,
        { if (it == 0) res(R.string.tv_set_off) else seconds(it) },
        { it.tv.ambientSeconds }, { s, v -> s.copy(tv = s.tv.copy(ambientSeconds = v)) }, R.string.tv_set_ambient_hint,
    ),
    enumRow(
        "tv.ambientStyle", R.string.tv_set_ambient_style, AmbientStyle.entries,
        { when (it) { AmbientStyle.COVER -> R.string.tv_set_ambient_cover; AmbientStyle.KARAOKE -> R.string.tv_set_ambient_karaoke; AmbientStyle.MASCOT -> R.string.tv_set_ambient_mascot; AmbientStyle.CLOCK -> R.string.tv_set_ambient_clock } },
        { it.tv.ambientStyle }, { s, v -> s.copy(tv = s.tv.copy(ambientStyle = v)) }, enabled = { it.tv.ambientSeconds > 0 },
    ),
    switchRow("tv.keepAwakeWhilePlaying", R.string.tv_set_keep_awake, { it.tv.keepAwakeWhilePlaying }, { s, v -> s.copy(tv = s.tv.copy(keepAwakeWhilePlaying = v)) }, R.string.tv_set_keep_awake_hint),
    numberRow(
        "tv.awakeAfterPauseMinutes", R.string.tv_set_awake_after, AWAKE_STEPS, ::minutes,
        { it.tv.awakeAfterPauseMinutes }, { s, v -> s.copy(tv = s.tv.copy(awakeAfterPauseMinutes = v)) }, R.string.tv_set_awake_after_hint,
        enabled = { it.tv.keepAwakeWhilePlaying },
    ),
    switchRow("tv.voiceInput", R.string.tv_set_voice, { it.tv.voiceInput }, { s, v -> s.copy(tv = s.tv.copy(voiceInput = v)) }, R.string.tv_set_voice_hint),
    numberRow(
        "tv.rideAutoOpenSeconds", R.string.tv_set_ride_open, RIDE_OPEN_STEPS,
        { if (it == 0) res(R.string.tv_set_off) else seconds(it) },
        { it.tv.rideAutoOpenSeconds }, { s, v -> s.copy(tv = s.tv.copy(rideAutoOpenSeconds = v)) }, R.string.tv_set_ride_open_hint,
    ),
    switchRow("tv.autoDownloadUpdates", R.string.tv_set_auto_update, { it.tv.autoDownloadUpdates }, { s, v -> s.copy(tv = s.tv.copy(autoDownloadUpdates = v)) }, R.string.tv_set_auto_update_hint),
)

private val lyricsRows = listOf(
    enumRow("lyricsMode", R.string.lyrics_mode, LyricsMode.entries, { it.label() }, { it.lyricsMode }, { s, v -> s.copy(lyricsMode = v) }, R.string.lyrics_mode_hint),
    enumRow("lyricsSize", R.string.lyrics_size, LyricsSize.entries, { it.label() }, { it.lyricsSize }, { s, v -> s.copy(lyricsSize = v) }),
    numberRow(
        "lyricsOffsetMs", R.string.tv_set_lyrics_timing, LYRICS_OFFSET_STEPS,
        { TvLabel.Plain(lyricsOffsetText(it) + " s") },
        { it.lyricsOffsetMs }, { s, v -> s.copy(lyricsOffsetMs = v) }, R.string.lyrics_timing_hint,
    ),
    switchRow("lyricsAutoOpen", R.string.lyrics_auto_open, { it.lyricsAutoOpen }, { s, v -> s.copy(lyricsAutoOpen = v) }, R.string.lyrics_auto_open_hint),
    switchRow("lyricsMascot", R.string.lyrics_mascot, { it.lyricsMascot }, { s, v -> s.copy(lyricsMascot = v) }, R.string.lyrics_mascot_hint),
    embedLyricsRow,
)

private val aboutRows = listOf(
    TvSettingRow.Action("whatsNewOpen", R.string.update_whats_new, R.string.whats_new_open_hint, TvSettingAction.WHATS_NEW),
    switchRow("whatsNew", R.string.whats_new_setting, { it.whatsNew }, { s, v -> s.copy(whatsNew = v) }, R.string.whats_new_setting_hint),
    TvSettingRow.Action("updateCheck", R.string.update_check_now, null, TvSettingAction.CHECK_UPDATES),
    switchRow("updateAutoCheck", R.string.update_auto, { it.updateAutoCheck }, { s, v -> s.copy(updateAutoCheck = v) }, R.string.update_auto_hint),
    switchRow("updatePrereleases", R.string.update_prereleases, { it.updatePrereleases }, { s, v -> s.copy(updatePrereleases = v) }, R.string.update_prereleases_hint),
)

/** Every row of every page: the ids a test walks, and what [rowById] looks in. */
val allSettingRows: List<TvSettingRow> by lazy { TvSettingsCategory.entries.flatMap(::settingsRows).distinctBy { it.id } }

/** The next option when LEFT / RIGHT is pressed ([step] = -1 or 1): wraps around, so a short list never dead-ends. */
fun cycleIndex(current: Int, size: Int, step: Int): Int = if (size <= 0) 0 else ((current + step) % size + size) % size

/** A list this long opens a picker on OK instead of cycling with each press. */
const val PICKER_FROM_OPTIONS = 6
