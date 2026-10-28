package com.perseusoft.homefi.ui.eq

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.EqBandKind
import com.perseusoft.homefi.core.EqProfile
import com.perseusoft.homefi.core.EqSource
import com.perseusoft.homefi.player.AudioOutput
import com.perseusoft.homefi.player.OutputKind
import com.perseusoft.homefi.ui.player.label

fun EqBandKind.labelRes(): Int = when (this) {
    EqBandKind.PEAK -> R.string.eq_kind_peak
    EqBandKind.LOW_SHELF -> R.string.eq_kind_low_shelf
    EqBandKind.HIGH_SHELF -> R.string.eq_kind_high_shelf
    EqBandKind.LOW_PASS -> R.string.eq_kind_low_pass
    EqBandKind.HIGH_PASS -> R.string.eq_kind_high_pass
}

/** The route type behind an equalizer output id (see `eqIdOf`). */
fun eqIdKind(id: String): OutputKind = when (id.substringBefore(':')) {
    "usb" -> OutputKind.USB
    "bt" -> OutputKind.BLUETOOTH
    "wired" -> OutputKind.WIRED
    "speaker" -> OutputKind.SPEAKER
    "hdmi" -> OutputKind.HDMI
    "line" -> OutputKind.LINE
    else -> OutputKind.OTHER
}

/** A readable name for an output id, connected or not ("Phone speaker", "FiiO K7"...). */
@Composable
fun outputLabel(id: String, connected: List<AudioOutput>): String {
    connected.firstOrNull { it.eqId == id }?.let { o -> return o.name.ifBlank { stringResource(o.kind.label()) } }
    val name = id.substringAfter(':', "")
    return name.ifBlank { stringResource(eqIdKind(id).label()) }
}

/** Where a profile came from, or null for one made by hand. */
@Composable
fun EqProfile.sourceLabel(): String? = when (val s = source) {
    is EqSource.AutoEq -> stringResource(R.string.eq_source_autoeq, s.source)
    EqSource.Assistant -> stringResource(R.string.eq_source_assistant)
    EqSource.Guided -> stringResource(R.string.eq_source_guided)
    EqSource.Manual -> null
}

/** `+3.5`, `-12`, `0` */
fun formatDb(db: Float): String {
    val r = Math.round(db * 10) / 10f
    val text = if (r == r.toInt().toFloat()) "${r.toInt()}" else "%.1f".format(r)
    return if (r > 0) "+$text" else text
}
