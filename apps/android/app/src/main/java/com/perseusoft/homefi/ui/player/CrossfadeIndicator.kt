package com.perseusoft.homefi.ui.player

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.SwapHoriz
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.player.Crossfading
import com.perseusoft.homefi.ui.theme.LocalMotion

/** "Crossfading into …" with a thin bar while two songs overlap; nothing otherwise. */
@Composable
internal fun CrossfadeIndicator(modifier: Modifier = Modifier) {
    val fade by Crossfading.state.collectAsStateWithLifecycle()
    val motion = LocalMotion.current
    AnimatedVisibility(fade != null, modifier, enter = motion.reveal(), exit = motion.conceal()) {
        val f = fade ?: return@AnimatedVisibility
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Icon(Icons.Rounded.SwapHoriz, null, tint = MaterialTheme.colorScheme.primary)
            Text(
                if (f.toTitle.isBlank()) stringResource(R.string.crossfade_now_untitled) else stringResource(R.string.crossfade_now, f.toTitle),
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f, fill = false),
            )
            LinearProgressIndicator(progress = { f.progress }, modifier = Modifier.width(48.dp))
        }
    }
}
