package com.perseusoft.homefi.ui.settings

import android.text.format.Formatter
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.data.ApiCacheDefaults
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.ButtonSpinner
import com.perseusoft.homefi.ui.rememberLoadingTask

private const val MB_PER_GB = 1024

/** One size step: "Off", "200 MB", "1 GB". */
@Composable
internal fun apiCacheStepText(mb: Int): String = when {
    mb == 0 -> stringResource(R.string.api_cache_off)
    mb >= MB_PER_GB && mb % MB_PER_GB == 0 -> stringResource(R.string.api_cache_step_gb, mb / MB_PER_GB)
    else -> stringResource(R.string.api_cache_step_mb, mb)
}

/** What the saved answers take now, in the phone's words ("37 MB", "Nothing saved"). */
@Composable
internal fun apiCacheUsedText(bytes: Long?): String = when (bytes) {
    null -> stringResource(R.string.api_cache_checking)
    0L -> stringResource(R.string.api_cache_nothing)
    else -> Formatter.formatShortFileSize(LocalContext.current, bytes)
}

/** Settings → Tidal account: how much room the saved Tidal answers get, what they take, and "Clear". */
@Composable
internal fun ApiCacheSettings() {
    val app = LocalContext.current.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val used by app.apiCache.usedBytes.collectAsStateWithLifecycle()
    val clearing = rememberLoadingTask()
    var failed by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { app.apiCache.refresh() }

    SettingsGroup(title = stringResource(R.string.api_cache_title)) {
        SegmentedRow(
            stringResource(R.string.api_cache_size),
            ApiCacheDefaults.STEPS_MB, settings.apiCacheMb,
            label = { apiCacheStepText(it) },
            onSelect = { mb -> app.settings.update { it.copy(apiCacheMb = mb) } },
            hint = stringResource(R.string.api_cache_size_hint),
        )
        RowDivider()
        SettingRow(
            label = stringResource(R.string.api_cache_used) + " · " + apiCacheUsedText(used),
            hint = stringResource(if (failed) R.string.api_cache_clear_failed else R.string.api_cache_clear_hint),
            trailing = {
                TextButton(
                    onClick = { clearing.run { failed = !app.apiCache.clear() } },
                    enabled = !clearing.active && used != 0L,
                ) {
                    if (clearing.active) ButtonSpinner(Modifier.padding(end = 8.dp))
                    Text(stringResource(R.string.api_cache_clear))
                }
            },
        )
    }
}
