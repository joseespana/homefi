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
import com.perseusoft.homefi.core.CrossfadeMode
import com.perseusoft.homefi.core.CrossfadeSettings
import com.perseusoft.homefi.core.FadeCurve
import com.perseusoft.homefi.core.crossfadeClamped
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.Crossfading
import java.util.Locale
import kotlin.math.roundToInt

/** Settings → Playback summary: "Crossfade 6 s" or "Gapless". */
@Composable
internal fun playbackSummary(): String {
    val s by LocalContext.current.homefi.settings.crossfade.collectAsStateWithLifecycle()
    return if (s.mode == CrossfadeMode.ON) stringResource(R.string.crossfade_summary_on, seconds(s.seconds)) else stringResource(R.string.crossfade_summary_off)
}

private fun seconds(v: Float): String =
    if (v == v.roundToInt().toFloat()) v.roundToInt().toString() else String.format(Locale.getDefault(), "%.1f", v)

private fun FadeCurve.labelRes(): Int = when (this) {
    FadeCurve.EQUAL_POWER -> R.string.crossfade_curve_equal_power
    FadeCurve.S_CURVE -> R.string.crossfade_curve_s
    FadeCurve.LINEAR -> R.string.crossfade_curve_linear
}

private fun FadeCurve.hintRes(): Int = when (this) {
    FadeCurve.EQUAL_POWER -> R.string.crossfade_curve_equal_power_hint
    FadeCurve.S_CURVE -> R.string.crossfade_curve_s_hint
    FadeCurve.LINEAR -> R.string.crossfade_curve_linear_hint
}

/**
 * Settings → Playback: crossfade, the same settings (and JSON) as the desktop's.
 * Changes apply from the next transition.
 */
@Composable
internal fun PlaybackSettings() {
    val store = LocalContext.current.homefi.settings
    val s by store.crossfade.collectAsStateWithLifecycle()
    val unavailable by Crossfading.unavailable.collectAsStateWithLifecycle()
    val set: (CrossfadeSettings) -> Unit = { store.setCrossfade(crossfadeClamped(it)) }

    SettingsGroup(title = stringResource(R.string.crossfade_title)) {
        SwitchRow(
            stringResource(R.string.crossfade_on), s.mode == CrossfadeMode.ON,
            hint = stringResource(R.string.crossfade_on_hint),
        ) { on -> set(s.copy(mode = if (on) CrossfadeMode.ON else CrossfadeMode.OFF)) }
        val reason = unavailable
        if (reason != null && s.mode == CrossfadeMode.ON && s.lowPowerFallback) SettingsNote(stringResource(reason))
        Reveal(s.mode == CrossfadeMode.ON) {
            RowDivider()
            SecondsSlider(R.string.crossfade_seconds, s.seconds, 1f..12f, steps = 21) { set(s.copy(seconds = it)) }
            RowDivider()
            ChoiceList(
                title = stringResource(R.string.crossfade_curve),
                options = listOf(FadeCurve.EQUAL_POWER, FadeCurve.S_CURVE, FadeCurve.LINEAR),
                selected = s.curve,
                label = { stringResource(it.labelRes()) },
                hint = { stringResource(it.hintRes()) },
                onSelect = { c -> set(s.copy(curve = c)) },
            )
        }
    }
    Reveal(s.mode == CrossfadeMode.ON) {
        SettingsGroup(title = stringResource(R.string.settings_playback_when)) {
            SwitchRow(stringResource(R.string.crossfade_smart), s.smart, hint = stringResource(R.string.crossfade_smart_hint)) { set(s.copy(smart = it)) }
            RowDivider()
            SwitchRow(stringResource(R.string.crossfade_album_runs), s.skipAlbumRuns, hint = stringResource(R.string.crossfade_album_runs_hint)) { set(s.copy(skipAlbumRuns = it)) }
            RowDivider()
            SwitchRow(stringResource(R.string.crossfade_manual), s.onManualSkip, hint = stringResource(R.string.crossfade_manual_hint)) { set(s.copy(onManualSkip = it)) }
            Reveal(s.onManualSkip) {
                RowDivider()
                SecondsSlider(R.string.crossfade_manual_seconds, s.manualSeconds, 0.5f..3f, steps = 9) { set(s.copy(manualSeconds = it)) }
            }
            RowDivider()
            val auto = stringResource(R.string.crossfade_min_auto, seconds(s.seconds * 2))
            val minLabel = if (s.minTrackSeconds > 0f) "${seconds(s.minTrackSeconds)} s" else auto
            var minTrack by remember(s.minTrackSeconds) { mutableFloatStateOf(s.minTrackSeconds) }
            SliderRow(
                stringResource(R.string.crossfade_min_track, minLabel), minTrack, 0f..120f,
                onChange = { minTrack = (it / 5f).roundToInt() * 5f },
                onFinished = { set(s.copy(minTrackSeconds = minTrack)) },
                hint = stringResource(R.string.crossfade_min_track_hint),
            )
            RowDivider()
            SwitchRow(stringResource(R.string.crossfade_low_power), s.lowPowerFallback, hint = stringResource(R.string.crossfade_low_power_hint)) { set(s.copy(lowPowerFallback = it)) }
        }
    }
}

/** A seconds slider that saves when let go (the audio picks it up at the next transition). */
@Composable
private fun SecondsSlider(title: Int, value: Float, range: ClosedFloatingPointRange<Float>, steps: Int, onSet: (Float) -> Unit) {
    var v by remember(value) { mutableFloatStateOf(value) }
    SliderRow(
        stringResource(title, seconds(v)), v, range,
        onChange = { v = (it * 4f).roundToInt() / 4f },
        onFinished = { onSet(v) },
        steps = steps,
    )
}
