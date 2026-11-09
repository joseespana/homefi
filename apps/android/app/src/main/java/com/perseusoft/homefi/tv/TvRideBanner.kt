package com.perseusoft.homefi.tv

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.DirectionsBike
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.perseusoft.homefi.R
import kotlinx.coroutines.delay

private const val TICK_MS = 1_000L

/**
 * "Ride from Pixel 10 · Watch": a phone sent its ride while the user is elsewhere. Watch has focus; with
 * [autoOpenSeconds] > 0 it opens by itself after that many seconds (the caller leaves it out while the
 * player is in front, so music is never interrupted by surprise).
 */
@Composable
fun TvRideBanner(phone: String, autoOpenSeconds: Int, paused: Boolean, onWatch: () -> Unit, onLater: () -> Unit, modifier: Modifier = Modifier) {
    val look = LocalTvLook.current
    var left by remember(phone) { mutableIntStateOf(autoOpenSeconds) }
    LaunchedEffect(phone, autoOpenSeconds, paused) {
        if (autoOpenSeconds <= 0 || paused) return@LaunchedEffect
        while (left > 0) {
            delay(TICK_MS)
            left--
        }
        onWatch()
    }
    TvGlass(modifier.width(460.dp), solid = true) {
        Row(Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Icon(Icons.Rounded.DirectionsBike, null, tint = look.accent2)
            Column(Modifier.weight(1f)) {
                Text(stringResource(R.string.tv_ride_from, phone), style = TvType.section, color = look.ink, maxLines = 1)
                if (autoOpenSeconds > 0 && !paused && left > 0) {
                    Text(stringResource(R.string.tv_ride_opening, left), style = TvType.secondary, color = look.ink2)
                }
            }
            TvPill(stringResource(R.string.tv_ride_watch), onWatch, Modifier.tvInitialFocus(), primary = true, height = 34.dp)
            TvPill(stringResource(R.string.update_later), onLater, height = 34.dp)
        }
    }
}
