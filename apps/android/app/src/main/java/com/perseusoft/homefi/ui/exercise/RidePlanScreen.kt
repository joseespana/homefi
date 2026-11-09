package com.perseusoft.homefi.ui.exercise

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material3.Button
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.BeatRatio
import com.perseusoft.homefi.core.PlannedSong
import com.perseusoft.homefi.core.RideBuilt
import com.perseusoft.homefi.core.RideProgress
import com.perseusoft.homefi.core.RideSegment
import com.perseusoft.homefi.core.SavedRideSummary
import com.perseusoft.homefi.ui.NameDialog
import com.perseusoft.homefi.library.EmbeddedCover
import com.perseusoft.homefi.ui.library.CoverArt
import com.perseusoft.homefi.player.exercise.RideSession
import com.perseusoft.homefi.player.exercise.RideUi
import com.perseusoft.homefi.ui.theme.Card

private const val PROGRESS_FULL = 1f

/** Step 2: the build, with what it is doing. Cancelling goes back to the setup. */
@Composable
fun RideBuilding(state: RideUi.Building, onCancel: () -> Unit) {
    Column(
        Modifier.fillMaxWidth().padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Spacer(Modifier.height(48.dp))
        Text(stringResource(R.string.ex_building), style = MaterialTheme.typography.titleLarge)
        val progress = state.progress
        val fraction = when (progress) {
            is RideProgress.Resolving -> progress.done.toFloat() / progress.total.coerceAtLeast(1u).toFloat()
            is RideProgress.Analyzing -> progress.done.toFloat() / progress.total.coerceAtLeast(1u).toFloat()
            RideProgress.Matching -> PROGRESS_FULL
            else -> null
        }
        if (fraction != null) LinearProgressIndicator(progress = { fraction }, modifier = Modifier.fillMaxWidth())
        else LinearProgressIndicator(Modifier.fillMaxWidth())
        Text(
            when (progress) {
                null -> stringResource(R.string.ex_step_starting)
                is RideProgress.Searching ->
                    if (progress.withAi) stringResource(R.string.ex_step_ai, state.aiName.orEmpty()) else stringResource(R.string.ex_step_search)
                is RideProgress.Resolving -> stringResource(R.string.ex_step_resolving, progress.done.toInt(), progress.total.toInt())
                is RideProgress.Analyzing -> stringResource(R.string.ex_step_analyzing, progress.done.toInt(), progress.total.toInt())
                RideProgress.Matching -> stringResource(R.string.ex_step_matching)
            },
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        OutlinedButton(onClick = onCancel) { Text(stringResource(R.string.ex_cancel)) }
    }
}

/** A build that failed, in plain words. */
@Composable
fun RideFailed(message: String, onBack: () -> Unit) {
    Column(Modifier.fillMaxWidth().padding(24.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
        Spacer(Modifier.height(32.dp))
        Text(stringResource(R.string.ex_failed), style = MaterialTheme.typography.titleLarge)
        Text(message, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Button(onClick = onBack) { Text(stringResource(R.string.ex_back_to_setup)) }
    }
}

/** Step 3: the plan: the mountain, then every part with its song and how the song's tempo fits the cadence. */
@Composable
fun RidePreview(built: RideBuilt, saved: SavedRideSummary?, session: RideSession) {
    var renaming by rememberSaveable { mutableStateOf(false) }
    val plan = built.plan
    val segments = plan.workout.segments
    val totalS = segments.sumOf { it.seconds.toInt() }
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(plan.workout.name, style = MaterialTheme.typography.headlineSmall)
        Text(
            stringResource(R.string.ex_plan_summary, clock(totalS), segments.size, plan.songs.size),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Card(Modifier.fillMaxWidth()) {
            MountainProfile(
                profile = plan.profile,
                marker = null,
                segments = segments,
                modifier = Modifier.fillMaxWidth().height(PLAN_MOUNTAIN_HEIGHT),
            )
        }
        if (saved != null) SavedLine(saved, onKeep = session::keepSaved, onRename = { renaming = true })
        plan.notes.forEach { note ->
            Text(note, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        segments.forEachIndexed { i, segment ->
            SegmentRow(segment, plan.songs.filter { it.segmentIndex.toInt() == i })
        }
        GradientButton(
            text = stringResource(R.string.ex_start),
            onClick = { session.start(built) },
            icon = Icons.Rounded.PlayArrow,
            modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
        )
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            TextButton(onClick = session::build) { Text(stringResource(R.string.ex_another_mix)) }
            TextButton(onClick = session::backToSetup) { Text(stringResource(R.string.ex_change)) }
        }
        Spacer(Modifier.height(16.dp))
    }
    if (renaming && saved != null) {
        NameDialog(
            title = stringResource(R.string.rides_rename),
            initial = saved.name,
            confirm = stringResource(R.string.rides_save),
            onConfirm = { name -> renaming = false; session.renameSaved(name) },
            onDismiss = { renaming = false },
        )
    }
}

/** "Saved automatically · Keep · Rename", or "Saved in My rides · Rename" once kept. */
@Composable
private fun SavedLine(saved: SavedRideSummary, onKeep: () -> Unit, onRename: () -> Unit) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Text(
            stringResource(if (saved.auto) R.string.rides_saved_auto else R.string.rides_saved_kept),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.weight(1f),
        )
        if (saved.auto) TextButton(onClick = onKeep, modifier = Modifier.heightIn(min = 48.dp)) { Text(stringResource(R.string.rides_keep)) }
        TextButton(onClick = onRename, modifier = Modifier.heightIn(min = 48.dp)) { Text(stringResource(R.string.rides_rename_short)) }
    }
}

private val PLAN_MOUNTAIN_HEIGHT = 140.dp
private val FIT_OK = androidx.compose.ui.graphics.Color(0xFF22C55E)
private val FIT_OFF = androidx.compose.ui.graphics.Color(0xFFF59E0B)
private val COVER_SIZE = 40.dp

@Composable
private fun SegmentRow(segment: RideSegment, songs: List<PlannedSong>) {
    val zone = segment.kind.zoneColor()
    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Spacer(Modifier.size(10.dp).background(zone, CircleShape))
                Text(stringResource(segment.kind.nameRes()), style = MaterialTheme.typography.titleSmall, modifier = Modifier.weight(1f))
                Text(
                    stringResource(R.string.ex_segment_line, clock(segment.seconds.toInt()), rpmText(segment), segment.effort.toInt()),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            songs.forEach { song ->
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    CoverArt(song.entry.localPath?.let { EmbeddedCover(it) } ?: song.entry.coverUrl, COVER_SIZE, radius = 8.dp)
                    Column(Modifier.weight(1f)) {
                        Text(song.entry.title, maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.bodyMedium)
                        Text(
                            song.entry.artist, maxLines = 1, overflow = TextOverflow.Ellipsis,
                            style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    BpmFit(song)
                }
            }
        }
    }
}

@Composable
private fun rpmText(segment: RideSegment): String =
    if (segment.minRpm == segment.maxRpm) "${segment.minRpm}" else "${segment.minRpm}-${segment.maxRpm}"

/** "128 BPM · 1:1 → 128", warning-coloured when the song is outside the cadence range. */
@Composable
private fun BpmFit(song: PlannedSong) {
    val bpm = song.bpm
    val fit = song.fit
    val text = when {
        bpm == null || fit == null -> stringResource(R.string.ex_bpm_metronome)
        else -> stringResource(
            R.string.ex_bpm_fit, bpm.toInt(), stringResource(fit.ratio.labelRes()), Math.round(fit.rpm),
        )
    }
    val off = fit != null && fit.errorRpm > 0f
    // green when the song's beat fits the cadence, amber when it is off or only the metronome is left
    TintedPill(text, if (off || fit == null) FIT_OFF else FIT_OK)
}

private fun BeatRatio.labelRes(): Int = when (this) {
    BeatRatio.WHOLE -> R.string.ex_ratio_whole
    BeatRatio.HALF -> R.string.ex_ratio_half
    BeatRatio.DOUBLE -> R.string.ex_ratio_double
}
