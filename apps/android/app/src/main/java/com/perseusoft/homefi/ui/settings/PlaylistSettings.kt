package com.perseusoft.homefi.ui.settings

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.ConcertThresholds
import com.perseusoft.homefi.ui.playlists.MAX_PARTY_MINUTES
import com.perseusoft.homefi.ui.playlists.MIN_PARTY_MINUTES
import com.perseusoft.homefi.ui.playlists.PARTY_MINUTES_STEP
import com.perseusoft.homefi.ui.playlists.CurveChips
import com.perseusoft.homefi.ui.playlists.partyLength
import com.perseusoft.homefi.core.PlaylistSettings
import com.perseusoft.homefi.homefi
import kotlin.math.roundToInt

// Ranges mirror the core's constants (concert / playlist settings); the core clamps again at use.
internal const val MIN_CONCERT_SHOWS = 3
internal const val MAX_CONCERT_SHOWS = 15
private const val MIN_THRESHOLD_PCT = 5
private const val MAX_THRESHOLD_PCT = 100
private const val THRESHOLD_STEP_PCT = 5
private const val MIN_SEARCH_BUDGET = 1
private const val MAX_SEARCH_BUDGET = 10
private const val MIN_PER_ARTIST = 1
private const val MAX_PER_ARTIST = 10

/** Settings → Playlists summary: "8 shows · always from 80%". */
@Composable
internal fun playlistsSummary(): String {
    val s by LocalContext.current.homefi.settings.playlists.collectAsStateWithLifecycle()
    return stringResource(R.string.pl_settings_summary, s.concertShows.toInt(), s.concertThresholds.alwaysPct.toInt())
}

/** Settings → Playlists: the same options (and JSON) as the desktop's. */
@Composable
internal fun PlaylistsSettings() {
    val store = LocalContext.current.homefi.settings
    val s by store.playlists.collectAsStateWithLifecycle()
    val set: (PlaylistSettings) -> Unit = store::setPlaylists
    val setThresholds: (ConcertThresholds) -> Unit = { set(s.copy(concertThresholds = it)) }
    val t = s.concertThresholds

    SettingsGroup(title = stringResource(R.string.pl_set_concert)) {
        CountSlider(
            stringResource(R.string.pl_shows),
            s.concertShows.toInt(), MIN_CONCERT_SHOWS..MAX_CONCERT_SHOWS, step = 1,
            readout = { stringResource(R.string.pl_shows_value, it) },
            hint = stringResource(R.string.pl_set_shows_hint),
        ) { set(s.copy(concertShows = it.toUByte())) }
        RowDivider()
        PctSlider(R.string.pl_set_always, R.string.pl_set_pct, t.alwaysPct.toInt()) { setThresholds(t.copy(alwaysPct = it.toUByte())) }
        RowDivider()
        PctSlider(R.string.pl_set_usually, R.string.pl_set_pct, t.usuallyPct.toInt()) { setThresholds(t.copy(usuallyPct = it.toUByte())) }
        RowDivider()
        PctSlider(
            R.string.pl_set_sometimes, R.string.pl_set_pct, t.sometimesPct.toInt(),
            hint = R.string.pl_set_sometimes_hint,
        ) { setThresholds(t.copy(sometimesPct = it.toUByte())) }
        RowDivider()
        PctSlider(
            R.string.pl_set_encore, R.string.pl_set_pct_plays, t.encorePct.toInt(),
            hint = R.string.pl_set_encore_hint,
        ) { setThresholds(t.copy(encorePct = it.toUByte())) }
        RowDivider()
        CountSlider(
            stringResource(R.string.pl_set_budget), s.searchBudget.toInt(), MIN_SEARCH_BUDGET..MAX_SEARCH_BUDGET, step = 1,
            readout = { it.toString() },
            hint = stringResource(R.string.pl_set_budget_hint),
        ) { set(s.copy(searchBudget = it.toUByte())) }
        RowDivider()
        SwitchRow(stringResource(R.string.pl_set_skip_tape), s.skipTape) { set(s.copy(skipTape = it)) }
        RowDivider()
        SwitchRow(stringResource(R.string.pl_set_covers), s.coversOriginalArtist) { set(s.copy(coversOriginalArtist = it)) }
        RowDivider()
        SwitchRow(stringResource(R.string.pl_set_show_sources), s.showSources) { set(s.copy(showSources = it)) }
    }
    SettingsGroup(title = stringResource(R.string.pl_set_party)) {
        CountSlider(
            stringResource(R.string.pl_set_party_minutes),
            s.partyMinutes.toInt(), MIN_PARTY_MINUTES..MAX_PARTY_MINUTES, step = PARTY_MINUTES_STEP,
            readout = { partyLength(it) },
            hint = stringResource(R.string.pl_set_party_defaults_hint),
        ) { set(s.copy(partyMinutes = it.toUShort())) }
        RowDivider()
        StackedSetting(stringResource(R.string.pl_set_party_curve)) {
            CurveChips(s.partyCurve, enabled = true) { set(s.copy(partyCurve = it)) }
        }
        RowDivider()
        SwitchRow(stringResource(R.string.pl_set_party_explicit), s.partyAllowExplicit) { set(s.copy(partyAllowExplicit = it)) }
        RowDivider()
        CountSlider(
            stringResource(R.string.pl_set_party_per_artist), s.partyMaxPerArtist.toInt(), MIN_PER_ARTIST..MAX_PER_ARTIST, step = 1,
            readout = { it.toString() },
            hint = stringResource(R.string.pl_set_party_per_artist_hint),
        ) { set(s.copy(partyMaxPerArtist = it.toUByte())) }
    }
    SettingsGroup(title = stringResource(R.string.pl_set_chat)) {
        CountSlider(
            stringResource(R.string.pl_set_party_minutes),
            s.chatMinutes.toInt(), MIN_PARTY_MINUTES..MAX_PARTY_MINUTES, step = PARTY_MINUTES_STEP,
            readout = { partyLength(it) },
            hint = stringResource(R.string.pl_set_chat_defaults_hint),
        ) { set(s.copy(chatMinutes = it.toUShort())) }
        RowDivider()
        StackedSetting(stringResource(R.string.pl_set_party_curve)) {
            CurveChips(s.chatCurve, enabled = true) { set(s.copy(chatCurve = it)) }
        }
        RowDivider()
        SwitchRow(stringResource(R.string.pl_set_chat_explicit), s.chatAllowExplicit) { set(s.copy(chatAllowExplicit = it)) }
        RowDivider()
        SwitchRow(
            stringResource(R.string.pl_set_chat_from_song), s.chatFromSong,
            hint = stringResource(R.string.pl_set_chat_from_song_hint),
        ) { set(s.copy(chatFromSong = it)) }
    }
    SettingsGroup(title = stringResource(R.string.pl_settings_title)) {
        SwitchRow(stringResource(R.string.pl_set_home_section), s.homeSection) { set(s.copy(homeSection = it)) }
        RowDivider()
        SwitchRow(
            stringResource(R.string.pl_set_prefer_library), s.preferLibrary,
            hint = stringResource(R.string.pl_set_prefer_library_hint),
        ) { set(s.copy(preferLibrary = it)) }
    }
    SettingsGroup(title = stringResource(R.string.pl_set_tidal)) {
        SwitchRow(
            stringResource(R.string.pl_set_tidal_auto), s.tidalAutoSave,
            hint = stringResource(R.string.pl_set_tidal_auto_hint),
        ) { set(s.copy(tidalAutoSave = it)) }
        RowDivider()
        SwitchRow(
            stringResource(R.string.pl_set_tidal_delete), s.tidalDeleteWith,
            hint = stringResource(R.string.pl_set_tidal_delete_hint),
        ) { set(s.copy(tidalDeleteWith = it)) }
    }
}

/** A percentage slider in 5% steps: moves a local value while dragging, saves when let go. */
@Composable
private fun PctSlider(label: Int, readout: Int, value: Int, hint: Int? = null, onSet: (Int) -> Unit) {
    CountSlider(
        stringResource(label), value, MIN_THRESHOLD_PCT..MAX_THRESHOLD_PCT, step = THRESHOLD_STEP_PCT,
        readout = { stringResource(readout, it) },
        hint = hint?.let { stringResource(it) },
        onSet = onSet,
    )
}

/**
 * An integer slider labelled "[label]: readout". The local value follows the thumb; [onSet]
 * runs when it's let go, and the value the store holds (clamped by the core) comes back in [value].
 */
@Composable
internal fun CountSlider(
    label: String,
    value: Int,
    range: IntRange,
    step: Int,
    readout: @Composable (Int) -> String,
    hint: String? = null,
    onSet: (Int) -> Unit,
) {
    var dragging by remember(value) { mutableFloatStateOf(value.toFloat()) }
    val shown = ((dragging / step).roundToInt() * step).coerceIn(range.first, range.last)
    val steps = ((range.last - range.first) / step - 1).coerceAtLeast(0)
    SliderRow(
        "$label: ${readout(shown)}", dragging, range.first.toFloat()..range.last.toFloat(),
        onChange = { dragging = it },
        onFinished = { onSet(shown) },
        steps = steps,
        hint = hint,
    )
}
