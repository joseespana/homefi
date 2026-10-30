package com.perseusoft.homefi.ui.settings

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.TasteIntensity
import com.perseusoft.homefi.core.tasteIntensityLimits
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.eq.formatDb

private fun TasteIntensity.labelRes(): Int = when (this) {
    TasteIntensity.SUBTLE -> R.string.intensity_subtle
    TasteIntensity.BALANCED -> R.string.intensity_balanced
    TasteIntensity.BOLD -> R.string.intensity_bold
}

private fun TasteIntensity.hintRes(): Int = when (this) {
    TasteIntensity.SUBTLE -> R.string.intensity_subtle_hint
    TasteIntensity.BALANCED -> R.string.intensity_balanced_hint
    TasteIntensity.BOLD -> R.string.intensity_bold_hint
}

/**
 * How strong the assistant's, the reactions' and the wizard's changes are (stored in the
 * taste settings, like on the desktop). The hint states the core's own limits for the choice.
 * Used in Settings → Sound and in the EQ screen's taste rules header.
 */
@Composable
internal fun TasteIntensityRow() {
    val sound = LocalContext.current.homefi.sound
    val taste by sound.taste.collectAsStateWithLifecycle()
    val limits = remember(taste.intensity) { runCatching { tasteIntensityLimits(taste.intensity) }.getOrNull() }
    val range = limits?.let { formatDb(it.mainMoveMinDb).removePrefix("+") to formatDb(it.mainMoveMaxDb).removePrefix("+") }
    SegmentedRow(
        stringResource(R.string.intensity_title), TasteIntensity.entries, taste.intensity,
        label = { stringResource(it.labelRes()) },
        onSelect = sound::setIntensity,
        hint = if (range != null) stringResource(taste.intensity.hintRes(), range.first, range.second)
        else stringResource(R.string.intensity_hint_plain),
    )
}
