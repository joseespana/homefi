package com.perseusoft.homefi.ui.exercise

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.os.SystemClock
import android.widget.Toast
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.tween
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import com.perseusoft.homefi.ui.tvpair.ConnectToTvButton
import com.perseusoft.homefi.ui.tvpair.PairedTvLine
import com.perseusoft.homefi.pair.TvSendState
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.KeyboardArrowDown
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.SkipNext
import androidx.compose.material.icons.rounded.Stop
import androidx.compose.material.icons.rounded.Tv
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.compose.material.icons.rounded.AccessibilityNew
import androidx.compose.material.icons.rounded.EventSeat
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.launch
import com.perseusoft.homefi.R
import com.perseusoft.homefi.player.exercise.asTypedAddress
import com.perseusoft.homefi.core.IntervalPhase
import com.perseusoft.homefi.core.RidePlan
import com.perseusoft.homefi.core.RidePosition
import com.perseusoft.homefi.data.PlayerLayout
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.exercise.RideFrame
import com.perseusoft.homefi.player.exercise.RideSession
import com.perseusoft.homefi.player.exercise.RideTvStatus
import com.perseusoft.homefi.player.exercise.RideUi
import com.perseusoft.homefi.ui.haptics
import com.perseusoft.homefi.ui.theme.HomefiTheme
import com.perseusoft.homefi.ui.theme.skinOf

/** Hold this long on End to end the ride (a wet thumb never ends it by accident). */
private const val HOLD_TO_END_MS = 1_200
private const val WIDE_DP = 600
private const val RING_DP = 250
private const val RING_WIDE_DP = 260

/** The smallest ring a phone ride shows (its numbers still read from the saddle). */
private const val RING_MIN_DP = 140

/** Height a phone ride needs besides the ring: top row, effort, song, next card, mountain, controls. */
// the mountain needs about 100 dp to draw its ribbon, ticks and pin
private const val COMPACT_MOUNTAIN_DP = 128
private const val MOUNTAIN_DP = 150
private const val PHONE_FIXED_DP = 480 + (MOUNTAIN_DP - 130)
private const val CONTROL_DP = 56
private const val PAUSE_DP = 64
private const val RPM_SP = 96
private const val RPM_SP_THREE_DIGITS = 80

/**
 * The ride and, after it, its summary. Always dark: the screen is looked at while sweating, in any room.
 * Dark unless `ride_always_dark` is off.
 */
@Composable
fun RideOverlay() {
    val app = LocalContext.current.homefi
    val ui by app.ride.ui.collectAsStateWithLifecycle()
    val alwaysDark by app.settings.exercise.collectAsStateWithLifecycle().let { s -> androidx.compose.runtime.derivedStateOf { s.value.rideAlwaysDark } }
    val body: @Composable () -> Unit = {
        Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
            when (val state = ui) {
                is RideUi.Riding -> RideScreen(state.plan, app.ride)
                is RideUi.Finished -> RideSummary(state, app.ride)
                else -> Unit
            }
        }
    }
    if (alwaysDark) HomefiTheme(remember { skinOf(PlayerLayout.CLASSIC, true) }, body) else body()
}

@Composable
private fun RideScreen(plan: RidePlan, session: RideSession) {
    val app = LocalContext.current.homefi
    val frame by session.frame.collectAsStateWithLifecycle()
    val tv by session.tv.collectAsStateWithLifecycle()
    val settings by app.settings.exercise.collectAsStateWithLifecycle()
    val view = LocalView.current
    val haptics = haptics()
    var confirmEnd by remember { mutableStateOf(false) }
    var confirmSkip by remember { mutableStateOf(false) }

    ImmersiveWhileShown()
    DisposableEffect(settings.keepScreenOn) {
        view.keepScreenOn = settings.keepScreenOn
        onDispose { view.keepScreenOn = false }
    }
    // a buzz on every part change (not on every stroke: battery and annoyance)
    val segment = frame?.view?.segmentIndex
    LaunchedEffect(segment) { if (settings.hapticCues && segment != null && segment > 0u) haptics.confirm() }

    // a fallback ("the TV did not join") shows once as a toast
    val notice by session.notice.collectAsStateWithLifecycle()
    val context = LocalContext.current
    LaunchedEffect(notice) {
        notice?.let {
            Toast.makeText(context, it, Toast.LENGTH_LONG).show()
            session.dismissNotice()
        }
    }

    val current = frame
    if (current == null) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { Text(stringResource(R.string.ex_building)) }
        return
    }
    val v = current.view
    val zone = v.kind.zoneColor()
    val segments = plan.workout.segments
    val paused = !current.playing && !tv.waitingForTv

    BoxWithConstraints(Modifier.fillMaxSize().systemBarsPadding()) {
        val wide = maxWidth >= WIDE_DP.dp
        if (wide) {
            Row(Modifier.fillMaxSize().padding(16.dp), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(12.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    Ring(current, settings, zone, RING_WIDE_DP)
                    EffortLine(v.effort.toInt(), zone)
                }
                Column(Modifier.weight(1.4f), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    TopRow(current, zone, session::close)
                    NextCard(current)
                    PositionAndSong(current)
                    MountainBlock(plan, current, Modifier.weight(1f))
                    TvChip(tv)
                }
                Controls(zone, paused, v.segmentIndex.toInt() >= v.segmentCount.toInt() - 1, vertical = true,
                    onPause = session::togglePause, onSkip = { confirmSkip = true }, onEnd = { confirmEnd = true }, onEndHold = session::end)
            }
        } else {
            // the controls stay pinned at the bottom on any phone; the ring gives up size first
            val ringDp = (maxHeight.value - PHONE_FIXED_DP).toInt().coerceIn(RING_MIN_DP, RING_DP)
            val mountainDp = if (maxHeight < 700.dp) COMPACT_MOUNTAIN_DP.dp else MOUNTAIN_DP.dp
            Column(Modifier.fillMaxSize().padding(12.dp)) {
                Column(
                    Modifier.weight(1f).verticalScroll(rememberScrollState()),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    TopRow(current, zone, session::close)
                    Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) { Ring(current, settings, zone, ringDp) }
                    EffortLine(v.effort.toInt(), zone)
                    PositionAndSong(current)
                    NextCard(current)
                    MountainBlock(plan, current, Modifier.height(mountainDp))
                    TvChip(tv)
                }
                Controls(zone, paused, v.segmentIndex.toInt() >= v.segmentCount.toInt() - 1, vertical = false,
                    onPause = session::togglePause, onSkip = { confirmSkip = true }, onEnd = { confirmEnd = true }, onEndHold = session::end)
            }
        }
        if (paused) PausedOverlay(session::togglePause, onEnd = { confirmEnd = true }, onEndHold = session::end)
    }

    if (confirmSkip) {
        val next = v.next
        AlertDialog(
            onDismissRequest = { confirmSkip = false },
            title = {
                Text(
                    if (next != null) stringResource(R.string.ex_skip_q, stringResource(next.kind.nameRes()))
                    else stringResource(R.string.ex_skip),
                )
            },
            confirmButton = { TextButton(onClick = { confirmSkip = false; session.skipSegment() }) { Text(stringResource(R.string.ex_skip_do)) } },
            dismissButton = { TextButton(onClick = { confirmSkip = false }) { Text(stringResource(R.string.ex_keep)) } },
        )
    }
    if (confirmEnd) {
        // the screen-reader path: holding is hard, so a dialog confirms
        AlertDialog(
            onDismissRequest = { confirmEnd = false },
            title = { Text(stringResource(R.string.ex_end_q)) },
            confirmButton = { TextButton(onClick = { confirmEnd = false; session.end() }) { Text(stringResource(R.string.ex_end)) } },
            dismissButton = { TextButton(onClick = { confirmEnd = false }) { Text(stringResource(R.string.ex_keep)) } },
        )
    }
}

/** System bars away while the ride shows (swipe from the edge brings them back). */
@Composable
private fun ImmersiveWhileShown() {
    val view = LocalView.current
    DisposableEffect(view) {
        val window = view.context.findActivity()?.window
        val controller = window?.let { WindowCompat.getInsetsController(it, view) }
        controller?.hide(WindowInsetsCompat.Type.systemBars())
        controller?.systemBarsBehavior = androidx.core.view.WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        onDispose { controller?.show(WindowInsetsCompat.Type.systemBars()) }
    }
}

private tailrec fun Context.findActivity(): Activity? = when (this) {
    is Activity -> this
    is ContextWrapper -> baseContext.findActivity()
    else -> null
}

/** Hide (back to the app, the ride goes on), the zone and the time left: one row that fits 360 dp wide. */
@Composable
private fun TopRow(frame: RideFrame, zone: Color, onHide: () -> Unit) {
    val v = frame.view
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        IconButton(onClick = onHide, modifier = Modifier.size(CONTROL_DP.dp)) {
            Icon(Icons.Rounded.KeyboardArrowDown, stringResource(R.string.ex_minimize))
        }
        // the part's name takes all the room the timer leaves (it shortens only on a tiny screen)
        Box(Modifier.weight(1f)) {
            Text(
                stringResource(v.kind.nameRes()),
                color = zone, fontSize = 15.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis,
                modifier = Modifier.border(BorderStroke(2.dp, zone), RoundedCornerShape(50)).padding(horizontal = 12.dp, vertical = 8.dp),
            )
        }
        Text(
            clock(v.remainingS.toInt()),
            fontSize = 40.sp, fontWeight = FontWeight.ExtraBold, maxLines = 1,
            modifier = Modifier.semantics { contentDescription = "" },
        )
    }
}

@Composable
private fun PositionPill(position: RidePosition) {
    Row(
        Modifier.background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(50)).padding(horizontal = 10.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Icon(if (position == RidePosition.Seated) Icons.Rounded.EventSeat else Icons.Rounded.AccessibilityNew, null, Modifier.size(18.dp))
        Text(stringResource(position.nameRes()), fontSize = 13.sp, maxLines = 1)
    }
}

/** Seated / standing and the song playing, on one line. */
@Composable
private fun PositionAndSong(frame: RideFrame) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        PositionPill(frame.view.position)
        Box(Modifier.weight(1f)) { SongLine(frame) }
    }
}

/**
 * What comes next: always there (dashed outline in the next part's colour). In the last seconds before the
 * change it fills with the colour and counts down ("Jumps in 0:10"); a screen reader announces it then.
 */
@Composable
private fun NextCard(frame: RideFrame) {
    val v = frame.view
    val next = v.next ?: return
    val zone = next.kind.zoneColor()
    val soon = v.notice
    val shape = RoundedCornerShape(12.dp)
    val line = stringResource(R.string.ex_next_line, next.targetRpm.toInt(), stringResource(next.position.nameRes()))
    val announce = stringResource(R.string.ex_next_in, stringResource(next.kind.nameRes()), clock(v.remainingS.toInt()), next.targetRpm.toInt())
    val outline = if (soon) null else PathEffect.dashPathEffect(floatArrayOf(DASH_PX, DASH_GAP_PX))
    Row(
        Modifier
            .fillMaxWidth()
            .background(if (soon) zone.copy(alpha = SOON_ALPHA) else Color.Transparent, shape)
            .drawBehind {
                drawRoundRect(
                    zone, cornerRadius = CornerRadius(12.dp.toPx()),
                    style = Stroke(2.dp.toPx(), pathEffect = outline),
                )
            }
            .padding(horizontal = 12.dp, vertical = 8.dp)
            .then(if (soon) Modifier.semantics { liveRegion = LiveRegionMode.Polite; contentDescription = announce } else Modifier),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Column(Modifier.weight(1f)) {
            Text(stringResource(R.string.ex_next_label), fontSize = 11.sp, color = MaterialTheme.colorScheme.onSurfaceVariant, fontWeight = FontWeight.Bold)
            Text(stringResource(next.kind.nameRes()), color = zone, fontSize = 18.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(line, fontSize = 13.sp, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        if (soon) {
            Text(stringResource(R.string.ex_next_countdown, clock(v.remainingS.toInt())), fontWeight = FontWeight.ExtraBold, fontSize = 18.sp)
        }
    }
}

private const val SOON_ALPHA = 0.14f
private const val RING_GLOW_ALPHA = 0.28f
private const val DASH_PX = 14f
private const val DASH_GAP_PX = 10f

/** The big target RPM inside the beat pulse; in a sprint block the work or rest cadence and GO / REST. */
@Composable
private fun Ring(frame: RideFrame, settings: com.perseusoft.homefi.core.ExerciseSettings, zone: Color, sizeDp: Int) {
    val v = frame.view
    val phase = v.interval?.phase
    val tint = when (phase) {
        IntervalPhase.REST -> com.perseusoft.homefi.core.SegmentKind.RECOVERY.zoneColor()
        else -> zone
    }
    val digits = v.targetRpm.toString()
    val description = stringResource(R.string.ex_target_rpm, v.targetRpm.toInt())
    BeatPulse(
        pulse = v.pulse,
        active = frame.playing,
        positionNow = { v.pulse.songPositionMs.toDouble() + (SystemClock.elapsedRealtime() - frame.takenAtMs) },
        offsetMs = settings.pulseOffsetMs.toInt(),
        style = settings.pulseStyle,
        color = tint,
        modifier = Modifier
            .size(sizeDp.dp)
            .drawBehind {
                // a soft glow of the zone colour behind the ring
                drawCircle(Brush.radialGradient(listOf(tint.copy(alpha = RING_GLOW_ALPHA), Color.Transparent)))
            }
            .semantics { contentDescription = description },
        ringWidth = 9.dp,
    ) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            if (phase != null) {
                Text(
                    stringResource(if (phase == IntervalPhase.WORK) R.string.ex_go else R.string.ex_rest),
                    color = tint, fontWeight = FontWeight.Black, fontSize = 20.sp,
                )
            }
            Text(
                digits,
                // the number grows and shrinks with the ring (a short phone gets a smaller ring)
                fontSize = ((if (digits.length > 2) RPM_SP_THREE_DIGITS else RPM_SP) * sizeDp.toFloat() / RING_DP).sp,
                fontWeight = FontWeight.ExtraBold, color = tint, textAlign = TextAlign.Center,
            )
            Text(stringResource(R.string.ex_rpm), fontSize = 14.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

@Composable
private fun EffortLine(effort: Int, zone: Color) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        EffortBars(effort, zone, Modifier.weight(1f, fill = false), barWidth = 16.dp, barHeight = 24.dp)
        Text(
            "${stringResource(effortWordRes(effort))} $effort",
            fontSize = 20.sp, fontWeight = FontWeight.Bold,
        )
    }
}

@Composable
private fun SongLine(frame: RideFrame) {
    val song = frame.song ?: return
    Text(
        "${song.entry.title} · ${song.entry.artist}",
        maxLines = 1, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant,
        overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
    )
}

@Composable
private fun MountainBlock(plan: RidePlan, frame: RideFrame, modifier: Modifier) {
    val v = frame.view
    Column(modifier, verticalArrangement = Arrangement.spacedBy(4.dp)) {
        MountainProfile(
            profile = plan.profile, marker = v.marker, segments = plan.workout.segments,
            paused = !frame.playing, modifier = Modifier.fillMaxWidth().weight(1f),
        )
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(clock(v.elapsedS.toInt()), fontSize = 12.sp)
            Box(
                Modifier.weight(1f).height(6.dp).clip(CircleShape).background(MaterialTheme.colorScheme.surfaceVariant),
            ) { Box(Modifier.fillMaxHeight().fillMaxWidth(v.progress.coerceIn(0f, 1f)).background(v.kind.zoneColor())) }
            Text(clock(v.totalS.toInt()), fontSize = 12.sp)
        }
    }
}

@Composable
private fun TvChip(tv: RideTvStatus) {
    val app = LocalContext.current.homefi
    val paired by app.tvPairings.paired.collectAsStateWithLifecycle()
    val send by app.tvPairings.sendState.collectAsStateWithLifecycle()
    val text = when {
        tv.waitingForTv -> stringResource(R.string.ex_tv_waiting_join)
        tv.active && tv.audioOnTv -> stringResource(R.string.ex_tv_playing_on_tv)
        tv.active -> stringResource(R.string.ex_tv_visual)
        tv.error != null -> tv.error
        else -> null
    }
    if (text == null && paired == null) return
    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
        if (text != null) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Icon(Icons.Rounded.Tv, null, Modifier.size(18.dp))
                Text(text, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                tv.url?.takeIf { tv.active && paired == null }?.let { Text(stringResource(R.string.ex_tv_address_on_phone, it.asTypedAddress()), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
            }
        }
        PairedTvLine()
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            // not paired yet: scan the TV's code; paired but the TV did not answer: send again
            if (paired == null) ConnectToTvButton()
            else if (tv.active && send == TvSendState.UNREACHABLE) {
                TextButton(onClick = app.tvPairings::resend, modifier = Modifier.heightIn(min = 48.dp)) { Text(stringResource(R.string.tv_send_again)) }
            }
        }
    }
}

@Composable
private fun Controls(
    zone: Color,
    paused: Boolean,
    lastPart: Boolean,
    vertical: Boolean,
    onPause: () -> Unit,
    onSkip: () -> Unit,
    onEnd: () -> Unit,
    onEndHold: () -> Unit,
) {
    val end = @Composable { HoldToEnd(onEnd, onEndHold) }
    val pause = @Composable { modifier: Modifier ->
        Button(
            onClick = onPause,
            modifier = modifier.height(PAUSE_DP.dp),
            shape = RoundedCornerShape(PAUSE_DP.dp / 2),
            colors = ButtonDefaults.buttonColors(containerColor = zone, contentColor = Color.Black),
        ) {
            Icon(if (paused) Icons.Rounded.PlayArrow else Icons.Rounded.Pause, null)
            Spacer(Modifier.width(8.dp))
            Text(stringResource(if (paused) R.string.ex_resume else R.string.ex_pause), fontSize = 18.sp)
        }
    }
    val skip = @Composable {
        IconButton(onClick = onSkip, enabled = !lastPart, modifier = Modifier.size(CONTROL_DP.dp)) {
            Icon(Icons.Rounded.SkipNext, stringResource(R.string.ex_skip))
        }
    }
    if (vertical) {
        Column(Modifier.fillMaxHeight().padding(end = 8.dp), verticalArrangement = Arrangement.spacedBy(16.dp, Alignment.CenterVertically)) {
            pause(Modifier.width(160.dp)); skip(); end()
        }
    } else {
        Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            end(); pause(Modifier.weight(1f)); skip()
        }
    }
}

/** Stop button that ends the ride after [HOLD_TO_END_MS] of holding, with a ring filling around it. */
@Composable
private fun HoldToEnd(onAccessibleEnd: () -> Unit, onHeld: () -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val fill = remember { Animatable(0f) }
    val hint = stringResource(R.string.ex_end_hold)
    val label = stringResource(R.string.ex_end)
    Box(
        Modifier
            .size(CONTROL_DP.dp)
            .clip(CircleShape)
            .background(MaterialTheme.colorScheme.errorContainer)
            .semantics(mergeDescendants = true) {
                contentDescription = label
                onClick(label) { onAccessibleEnd(); true }
            }
            .pointerInput(Unit) {
                detectTapGestures(
                    onTap = { Toast.makeText(context, hint, Toast.LENGTH_SHORT).show() },
                    onPress = {
                        val job = scope.launch {
                            fill.animateTo(1f, tween(HOLD_TO_END_MS, easing = LinearEasing))
                            onHeld()
                        }
                        tryAwaitRelease()
                        job.cancel()
                        scope.launch { fill.snapTo(0f) }
                    },
                )
            },
        contentAlignment = Alignment.Center,
    ) {
        androidx.compose.material3.CircularProgressIndicator(
            progress = { fill.value }, modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.error,
        )
        Icon(Icons.Rounded.Stop, null, tint = MaterialTheme.colorScheme.onErrorContainer)
    }
}

@Composable
private fun PausedOverlay(onResume: () -> Unit, onEnd: () -> Unit, onEndHold: () -> Unit) {
    Box(Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.78f)), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(16.dp)) {
            Text(stringResource(R.string.ex_paused), fontSize = 32.sp, fontWeight = FontWeight.ExtraBold, color = Color.White)
            Text(stringResource(R.string.ex_paused_d), color = Color.White.copy(alpha = 0.8f))
            Button(onClick = onResume, modifier = Modifier.height(PAUSE_DP.dp).width(220.dp)) {
                Icon(Icons.Rounded.PlayArrow, null)
                Text(stringResource(R.string.ex_resume), fontSize = 18.sp)
            }
            HoldToEnd(onEnd, onEndHold)
        }
    }
}
