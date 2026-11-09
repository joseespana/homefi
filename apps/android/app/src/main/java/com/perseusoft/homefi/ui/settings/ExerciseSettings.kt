package com.perseusoft.homefi.ui.settings

import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.CourseLook
import com.perseusoft.homefi.core.ExerciseSettings
import com.perseusoft.homefi.core.PresetId
import com.perseusoft.homefi.core.PulseStyle
import com.perseusoft.homefi.core.SourcesMode
import com.perseusoft.homefi.core.TvAudio
import com.perseusoft.homefi.core.exerciseDefaultSettings
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.exercise.labelRes
import kotlin.math.roundToInt

// Ranges mirror the core's `exercise::settings` constants; the core clamps again when it stores them.
private const val MIN_MINUTES = 10
private const val MAX_MINUTES = 90
private const val MINUTES_STEP = 5
private const val MIN_HISTORY = 5
private const val MAX_HISTORY = 200
private const val HISTORY_STEP = 5
private const val MIN_TOLERANCE_RPM = 2
private const val MAX_TOLERANCE_RPM = 12
private const val MAX_ANALYSIS_BUDGET = 50
private const val ANALYSIS_STEP = 5
private const val MAX_COUNTDOWN_S = 10
private const val MIN_NOTICE_S = 5
private const val MAX_NOTICE_S = 30
private const val PERCENT = 100
private const val VOLUME_STEP_PCT = 5
private const val MAX_OFFSET_MS = 500
private const val OFFSET_STEP_MS = 50
private const val MIN_PORT = 1024
private const val MAX_PORT = 65535
private const val PORT_STEP = 1

/** Settings → Train summary: "30 min · First ride". */
@Composable
internal fun exerciseSummary(): String {
    val app = LocalContext.current.homefi
    val s by app.settings.exercise.collectAsStateWithLifecycle()
    return stringResource(R.string.ex_set_summary, s.rideMinutes.toInt(), stringResource(s.defaultPreset.labelRes()))
}

/** Settings → Train: every option of Exercise mode (the core's JSON, same rows as the desktop's page). */
@Composable
internal fun ExerciseSettingsPage() {
    val app = LocalContext.current.homefi
    val store = app.settings
    val s by store.exercise.collectAsStateWithLifecycle()
    val set: (ExerciseSettings) -> Unit = store::setExercise

    SettingsGroup(title = stringResource(R.string.ex_set_group_ride)) {
        SwitchRow(stringResource(R.string.ex_set_entry), s.showEntry, hint = stringResource(R.string.ex_set_entry_hint)) { set(s.copy(showEntry = it)) }
        RowDivider()
        SwitchRow(stringResource(R.string.ex_set_beginner), s.beginnerMode, hint = stringResource(R.string.ex_set_beginner_hint)) { set(s.copy(beginnerMode = it)) }
        RowDivider()
        SegmentedRow(
            stringResource(R.string.ex_set_style), PresetId.entries, s.defaultPreset,
            label = { stringResource(it.labelRes()) }, onSelect = { set(s.copy(defaultPreset = it)) },
        )
        RowDivider()
        CountSlider(
            stringResource(R.string.ex_set_minutes), s.rideMinutes.toInt(), MIN_MINUTES..MAX_MINUTES, step = MINUTES_STEP,
            readout = { stringResource(R.string.ex_minutes_value, it) },
        ) { set(s.copy(rideMinutes = it.toUShort())) }
        RowDivider()
        CountSlider(
            stringResource(R.string.rides_set_history), s.rideHistoryMax.toInt(), MIN_HISTORY..MAX_HISTORY, step = HISTORY_STEP,
            readout = { stringResource(R.string.rides_set_history_value, it) }, hint = stringResource(R.string.rides_set_history_hint),
        ) { set(s.copy(rideHistoryMax = it.toUShort())) }
    }

    SettingsGroup(title = stringResource(R.string.ex_music)) {
        SegmentedRow(
            stringResource(R.string.ex_set_sources), SourcesMode.entries, s.sources,
            label = { stringResource(it.labelRes()) }, onSelect = { set(s.copy(sources = it)) },
        )
        RowDivider()
        CountSlider(
            stringResource(R.string.ex_set_tolerance), s.cadenceToleranceRpm.toInt(), MIN_TOLERANCE_RPM..MAX_TOLERANCE_RPM, step = 1,
            readout = { stringResource(R.string.ex_set_rpm_value, it) }, hint = stringResource(R.string.ex_set_tolerance_hint),
        ) { set(s.copy(cadenceToleranceRpm = it.toUByte())) }
        RowDivider()
        SwitchRow(stringResource(R.string.ex_set_half_time), s.allowHalfTime, hint = stringResource(R.string.ex_set_half_time_hint)) { set(s.copy(allowHalfTime = it)) }
        RowDivider()
        SwitchRow(stringResource(R.string.ex_set_double_time), s.allowDoubleTime, hint = stringResource(R.string.ex_set_double_time_hint)) { set(s.copy(allowDoubleTime = it)) }
        RowDivider()
        SwitchRow(stringResource(R.string.ex_set_explicit), s.allowExplicit) { set(s.copy(allowExplicit = it)) }
        RowDivider()
        CountSlider(
            stringResource(R.string.ex_set_budget), s.analysisBudget.toInt(), 0..MAX_ANALYSIS_BUDGET, step = ANALYSIS_STEP,
            readout = { stringResource(R.string.ex_set_songs_value, it) }, hint = stringResource(R.string.ex_set_budget_hint),
        ) { set(s.copy(analysisBudget = it.toUByte())) }
    }

    SettingsGroup(title = stringResource(R.string.ex_set_group_cues)) {
        SwitchRow(stringResource(R.string.ex_set_cues), s.audibleCues, hint = stringResource(R.string.ex_set_cues_hint)) { set(s.copy(audibleCues = it)) }
        RowDivider()
        CountSlider(
            stringResource(R.string.ex_set_cue_volume), (s.cueVolume * PERCENT).roundToInt(), 0..PERCENT, step = VOLUME_STEP_PCT,
            readout = { "$it%" },
        ) { set(s.copy(cueVolume = it / PERCENT.toFloat())) }
        RowDivider()
        CountSlider(
            stringResource(R.string.ex_set_countdown), s.countdownSeconds.toInt(), 0..MAX_COUNTDOWN_S, step = 1,
            readout = { stringResource(R.string.ex_set_seconds_value, it) },
        ) { set(s.copy(countdownSeconds = it.toUByte())) }
        RowDivider()
        CountSlider(
            stringResource(R.string.ex_set_notice), s.segmentNoticeSeconds.toInt(), MIN_NOTICE_S..MAX_NOTICE_S, step = 1,
            readout = { stringResource(R.string.ex_set_seconds_value, it) },
        ) { set(s.copy(segmentNoticeSeconds = it.toUByte())) }
        RowDivider()
        SwitchRow(stringResource(R.string.ex_set_click), s.metronomeClick, hint = stringResource(R.string.ex_set_click_hint)) { set(s.copy(metronomeClick = it)) }
        RowDivider()
        SegmentedRow(
            stringResource(R.string.ex_set_pulse), PulseStyle.entries, s.pulseStyle,
            label = { stringResource(it.labelRes()) }, onSelect = { set(s.copy(pulseStyle = it)) },
        )
        RowDivider()
        CountSlider(
            stringResource(R.string.ex_set_offset), s.pulseOffsetMs.toInt(), -MAX_OFFSET_MS..MAX_OFFSET_MS, step = OFFSET_STEP_MS,
            readout = { stringResource(R.string.ex_set_ms_value, it) }, hint = stringResource(R.string.ex_set_offset_hint),
        ) { set(s.copy(pulseOffsetMs = it.toShort())) }
    }

    SettingsGroup(title = stringResource(R.string.ex_set_group_screen)) {
        SwitchRow(stringResource(R.string.ex_set_always_dark), s.rideAlwaysDark) { set(s.copy(rideAlwaysDark = it)) }
        RowDivider()
        SwitchRow(stringResource(R.string.ex_set_haptics), s.hapticCues) { set(s.copy(hapticCues = it)) }
        RowDivider()
        SwitchRow(stringResource(R.string.ex_set_keep_on), s.keepScreenOn) { set(s.copy(keepScreenOn = it)) }
        RowDivider()
        SegmentedRow(
            stringResource(R.string.ex_set_course_look), CourseLook.entries, s.courseLook,
            label = { stringResource(it.labelRes()) }, onSelect = { set(s.copy(courseLook = it)) },
            hint = stringResource(R.string.ex_set_course_look_hint),
        )
        RowDivider()
        SwitchRow(stringResource(R.string.ex_set_course_summits), s.courseSummits) { set(s.copy(courseSummits = it)) }
    }

    SettingsGroup(title = stringResource(R.string.ex_tv), description = stringResource(R.string.ex_set_tv_hint)) {
        SwitchRow(stringResource(R.string.ex_set_tv), s.tvEnabled) { set(s.copy(tvEnabled = it)) }
        RowDivider()
        SwitchRow(stringResource(R.string.ex_set_tv_lyrics), s.tvLyrics, hint = stringResource(R.string.ex_set_tv_lyrics_hint)) { set(s.copy(tvLyrics = it)) }
        RowDivider()
        SegmentedRow(
            stringResource(R.string.ex_tv_audio), TvAudio.entries, s.tvAudio,
            label = { stringResource(if (it == TvAudio.TV) R.string.ex_tv_audio_tv else R.string.ex_tv_audio_here) },
            onSelect = { set(s.copy(tvAudio = it)) },
        )
        RowDivider()
        CountSlider(
            stringResource(R.string.ex_set_port), s.tvPort.toInt(), MIN_PORT..MAX_PORT, step = PORT_STEP,
            readout = { "$it" }, hint = stringResource(R.string.ex_set_port_hint),
        ) { set(s.copy(tvPort = it.toUShort())) }
    }

    SettingsGroup(title = stringResource(R.string.ex_set_group_health)) {
        ActionRow(
            stringResource(R.string.ex_set_health), hint = stringResource(R.string.ex_set_health_hint),
            trailing = {
                TextButton(onClick = { set(s.copy(healthNoticeSeen = false)) }, enabled = s.healthNoticeSeen) {
                    Text(stringResource(R.string.ex_set_health_again))
                }
            },
        )
        RowDivider()
        ActionRow(
            stringResource(R.string.ex_set_reset),
            trailing = { TextButton(onClick = { set(exerciseDefaultSettings()) }) { Text(stringResource(R.string.ex_set_reset_do)) } },
        )
    }
}

private fun SourcesMode.labelRes(): Int = when (this) {
    SourcesMode.LIBRARY_FIRST -> R.string.ex_sources_library_first
    SourcesMode.TIDAL_FIRST -> R.string.ex_sources_tidal_first
    SourcesMode.LIBRARY_ONLY -> R.string.ex_sources_library_only
}

private fun PulseStyle.labelRes(): Int = when (this) {
    PulseStyle.RING -> R.string.ex_pulse_ring
    PulseStyle.BAR -> R.string.ex_pulse_bar
    PulseStyle.FLASH -> R.string.ex_pulse_flash
}

private fun CourseLook.labelRes(): Int = when (this) {
    CourseLook.RIBBON -> R.string.ex_look_ribbon
    CourseLook.ALPINE -> R.string.ex_look_alpine
    CourseLook.STAGE -> R.string.ex_look_stage
}
