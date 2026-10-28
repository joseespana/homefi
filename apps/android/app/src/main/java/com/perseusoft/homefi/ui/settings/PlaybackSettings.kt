package com.perseusoft.homefi.ui.settings

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.CrossfadeMode
import com.perseusoft.homefi.core.CrossfadeSettings
import com.perseusoft.homefi.core.FadeCurve
import com.perseusoft.homefi.core.crossfadeClamped
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.Crossfading
import com.perseusoft.homefi.ui.SelectChip
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

    Header(R.string.crossfade_title)
    Toggle(R.string.crossfade_on, s.mode == CrossfadeMode.ON) { on -> set(s.copy(mode = if (on) CrossfadeMode.ON else CrossfadeMode.OFF)) }
    Hint(R.string.crossfade_on_hint)
    val reason = unavailable
    if (reason != null && s.mode == CrossfadeMode.ON && s.lowPowerFallback) Hint(reason)
    Reveal(s.mode == CrossfadeMode.ON) {
        SecondsSlider(R.string.crossfade_seconds, s.seconds, 1f..12f, steps = 21) { set(s.copy(seconds = it)) }

        Label(R.string.crossfade_curve)
        Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
            listOf(FadeCurve.EQUAL_POWER, FadeCurve.S_CURVE, FadeCurve.LINEAR).forEach { c ->
                SelectChip(
                    selected = s.curve == c,
                    onClick = { set(s.copy(curve = c)) },
                    label = { Text(stringResource(c.labelRes())) },
                    modifier = Modifier.padding(end = 8.dp),
                )
            }
        }
        Hint(s.curve.hintRes())

        Toggle(R.string.crossfade_smart, s.smart) { set(s.copy(smart = it)) }
        Hint(R.string.crossfade_smart_hint)
        Toggle(R.string.crossfade_album_runs, s.skipAlbumRuns) { set(s.copy(skipAlbumRuns = it)) }
        Hint(R.string.crossfade_album_runs_hint)
        Toggle(R.string.crossfade_manual, s.onManualSkip) { set(s.copy(onManualSkip = it)) }
        Hint(R.string.crossfade_manual_hint)
        Reveal(s.onManualSkip) {
            SecondsSlider(R.string.crossfade_manual_seconds, s.manualSeconds, 0.5f..3f, steps = 9) { set(s.copy(manualSeconds = it)) }
        }

        val auto = stringResource(R.string.crossfade_min_auto, seconds(s.seconds * 2))
        val minLabel = if (s.minTrackSeconds > 0f) "${seconds(s.minTrackSeconds)} s" else auto
        var minTrack by remember(s.minTrackSeconds) { mutableFloatStateOf(s.minTrackSeconds) }
        ListItem(
            colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surface),
            headlineContent = { Text(stringResource(R.string.crossfade_min_track, minLabel)) },
            supportingContent = {
                Column {
                    Slider(
                        value = minTrack,
                        onValueChange = { minTrack = (it / 5f).roundToInt() * 5f },
                        onValueChangeFinished = { set(s.copy(minTrackSeconds = minTrack)) },
                        valueRange = 0f..120f,
                    )
                    Text(stringResource(R.string.crossfade_min_track_hint), style = MaterialTheme.typography.bodySmall)
                }
            },
        )

        Toggle(R.string.crossfade_low_power, s.lowPowerFallback) { set(s.copy(lowPowerFallback = it)) }
        Hint(R.string.crossfade_low_power_hint)
    }
}

/** A seconds slider that saves when let go (the audio picks it up at the next transition). */
@Composable
private fun SecondsSlider(title: Int, value: Float, range: ClosedFloatingPointRange<Float>, steps: Int, onSet: (Float) -> Unit) {
    var v by remember(value) { mutableFloatStateOf(value) }
    ListItem(
        colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surface),
        headlineContent = { Text(stringResource(title, seconds(v))) },
        supportingContent = {
            Slider(
                value = v,
                onValueChange = { v = (it * 4f).roundToInt() / 4f },
                onValueChangeFinished = { onSet(v) },
                valueRange = range,
                steps = steps,
            )
        },
    )
}
