package com.perseusoft.homefi.ui.exercise

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.DirectionsBike
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.homefi

private const val BAR_TINT_ALPHA = 0.16f

/**
 * The ride while its screen is hidden: one bar over the mini player on every tab, like the song that plays. The part
 * and its colour, the time left in it and the target; a tap opens the ride again, the button pauses or resumes it.
 */
@Composable
fun RideBar(modifier: Modifier = Modifier) {
    val app = LocalContext.current.homefi
    val frame by app.ride.frame.collectAsStateWithLifecycle()
    val current = frame ?: return
    val v = current.view
    val zone = v.kind.zoneColor()
    val open = stringResource(R.string.ex_open_ride)
    Row(
        modifier
            .fillMaxWidth()
            .background(zone.copy(alpha = BAR_TINT_ALPHA))
            .clickable(onClickLabel = open, role = Role.Button, onClick = app.ride::open)
            .heightIn(min = 56.dp)
            .padding(horizontal = 16.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Icon(
            Icons.AutoMirrored.Rounded.DirectionsBike, null, tint = Color.Black,
            modifier = Modifier.size(36.dp).background(zone, CircleShape).padding(7.dp),
        )
        Column(Modifier.weight(1f)) {
            Text(
                "${stringResource(v.kind.nameRes())} · ${clock(v.remainingS.toInt())}",
                style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis,
            )
            Text(
                "${v.shown.withUnit()} · $open",
                style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1, overflow = TextOverflow.Ellipsis,
            )
        }
        FilledIconButton(
            onClick = app.ride::togglePause,
            colors = IconButtonDefaults.filledIconButtonColors(containerColor = zone, contentColor = Color.Black),
        ) {
            Icon(
                if (current.playing) Icons.Rounded.Pause else Icons.Rounded.PlayArrow,
                stringResource(if (current.playing) R.string.ex_pause else R.string.ex_resume),
            )
        }
    }
}
