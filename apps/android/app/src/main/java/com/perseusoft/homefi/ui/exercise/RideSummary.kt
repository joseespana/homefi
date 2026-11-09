package com.perseusoft.homefi.ui.exercise

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.SegmentKind
import com.perseusoft.homefi.player.exercise.RideSession
import com.perseusoft.homefi.player.exercise.RideUi
import com.perseusoft.homefi.ui.theme.Card

private val SUMMARY_MOUNTAIN_HEIGHT = 140.dp

/** The finished (or ended) ride: the whole mountain and four numbers. */
@Composable
fun RideSummary(state: RideUi.Finished, session: RideSession) {
    val plan = state.plan
    val segments = plan.workout.segments
    val climbs = segments.count { it.kind == SegmentKind.SEATED_CLIMB || it.kind == SegmentKind.STANDING_CLIMB }
    val sprints = segments.count { it.kind == SegmentKind.SPRINT }
    Column(
        Modifier.fillMaxSize().systemBarsPadding().verticalScroll(rememberScrollState()).padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Text(
            stringResource(if (state.completed) R.string.ex_done else R.string.ex_ended),
            style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.Bold,
        )
        Card(Modifier.fillMaxWidth()) {
            MountainProfile(
                profile = plan.profile, marker = null, segments = segments,
                modifier = Modifier.fillMaxWidth().height(SUMMARY_MOUNTAIN_HEIGHT),
            )
        }
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Stat(R.string.ex_stat_time, clock(state.elapsedS), Modifier.weight(1f))
            Stat(R.string.ex_stat_songs, "${plan.songs.size}", Modifier.weight(1f))
        }
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Stat(R.string.ex_stat_climbs, "$climbs", Modifier.weight(1f))
            Stat(R.string.ex_stat_sprints, "$sprints", Modifier.weight(1f))
        }
        Text(stringResource(R.string.ex_guide), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Button(onClick = session::again, modifier = Modifier.fillMaxWidth().height(64.dp)) { Text(stringResource(R.string.ex_again)) }
        OutlinedButton(onClick = session::leave, modifier = Modifier.fillMaxWidth().height(56.dp)) { Text(stringResource(R.string.ex_finish)) }
    }
}

@Composable
private fun Stat(label: Int, value: String, modifier: Modifier) {
    Card(modifier) {
        Column(Modifier.padding(16.dp), horizontalAlignment = Alignment.Start) {
            Text(value, style = MaterialTheme.typography.headlineMedium, fontWeight = FontWeight.ExtraBold)
            Text(stringResource(label), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}
