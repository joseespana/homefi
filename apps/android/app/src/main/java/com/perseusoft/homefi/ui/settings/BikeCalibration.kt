package com.perseusoft.homefi.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.BikeProfile
import com.perseusoft.homefi.core.ConsoleUnit
import com.perseusoft.homefi.core.exerciseCalibrate
import com.perseusoft.homefi.core.exerciseCalibration
import com.perseusoft.homefi.player.exercise.RideCues
import kotlinx.coroutines.delay

private const val MS_PER_MINUTE = 60_000L
private const val MS_PER_SECOND = 1_000L
private const val CLICK_VOLUME = 0.8f

/** The console unit's name as the rows and the sheet show it. */
internal fun ConsoleUnit.labelRes(): Int = when (this) {
    ConsoleUnit.RPM -> R.string.ex_console_rpm
    ConsoleUnit.KPH -> R.string.ex_console_kph
    ConsoleUnit.MPH -> R.string.ex_console_mph
    ConsoleUnit.NONE -> R.string.ex_console_none
}

/** Whether the console shows a speed, so a speed factor means something. */
internal val ConsoleUnit.isSpeed: Boolean get() = this == ConsoleUnit.KPH || this == ConsoleUnit.MPH

/**
 * "Calibrate speed": clicks at the calibration cadence for [exerciseCalibration]'s seconds; the rider pedals to them
 * and types the speed the console shows. The core turns that into km/h per RPM ([exerciseCalibrate]); one point is
 * enough, a spinning bike's speed sensor sits on the flywheel. [onSave] gets the bike with the new factor
 * (0 = cleared).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun BikeCalibrationSheet(bike: BikeProfile, onSave: (BikeProfile) -> Unit, onDismiss: () -> Unit) {
    val context = LocalContext.current
    val spec = remember { exerciseCalibration() }
    val scope = rememberCoroutineScope()
    val cues = remember { RideCues(context.applicationContext, scope) }
    var running by remember { mutableStateOf(false) }
    var secondsLeft by remember { mutableIntStateOf(spec.seconds.toInt()) }
    var typed by remember { mutableStateOf("") }
    var invalid by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { cues.prepare() }
    LaunchedEffect(running) {
        if (!running) return@LaunchedEffect
        val beatMs = MS_PER_MINUTE / spec.rpm.toInt().coerceAtLeast(1)
        val totalMs = spec.seconds.toLong() * MS_PER_SECOND
        var elapsedMs = 0L
        while (elapsedMs < totalMs) {
            cues.click(0, CLICK_VOLUME)
            secondsLeft = ((totalMs - elapsedMs + MS_PER_SECOND - 1) / MS_PER_SECOND).toInt()
            delay(beatMs)
            elapsedMs += beatMs
        }
        running = false
        secondsLeft = spec.seconds.toInt()
    }
    val unitName = stringResource(bike.console.labelRes())
    ModalBottomSheet(onDismissRequest = onDismiss) {
        Column(
            Modifier.navigationBarsPadding().padding(horizontal = 24.dp).padding(bottom = 16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(stringResource(R.string.ex_set_calibrate), style = MaterialTheme.typography.titleLarge)
            Text(
                stringResource(R.string.ex_cal_body, spec.seconds.toInt(), spec.rpm.toInt()),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            if (running) {
                Button(onClick = { running = false; secondsLeft = spec.seconds.toInt() }, Modifier.fillMaxWidth()) {
                    Text("${stringResource(R.string.ex_cal_stop)} · ${stringResource(R.string.ex_cal_left, secondsLeft)}")
                }
            } else {
                OutlinedButton(onClick = { running = true }, Modifier.fillMaxWidth()) { Text(stringResource(R.string.ex_cal_start)) }
            }
            OutlinedTextField(
                value = typed,
                onValueChange = { typed = it; invalid = false },
                label = { Text(stringResource(R.string.ex_cal_speed, unitName)) },
                singleLine = true,
                isError = invalid,
                supportingText = if (invalid) ({ Text(stringResource(R.string.ex_cal_invalid)) }) else null,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                modifier = Modifier.fillMaxWidth(),
            )
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                TextButton(onClick = { onSave(bike.copy(kphPerRpm = 0f)); onDismiss() }, enabled = bike.kphPerRpm > 0f) {
                    Text(stringResource(R.string.ex_cal_clear))
                }
                Button(
                    onClick = {
                        val speed = typed.replace(',', '.').toFloatOrNull()
                        val factor = speed?.let { exerciseCalibrate(bike.console, spec.rpm, it) }
                        if (factor == null) {
                            invalid = true
                        } else {
                            onSave(bike.copy(kphPerRpm = factor))
                            onDismiss()
                        }
                    },
                    enabled = typed.isNotBlank(),
                ) { Text(stringResource(R.string.ex_cal_save)) }
            }
        }
    }
}
