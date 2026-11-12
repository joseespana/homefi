package com.perseusoft.homefi.ui.exercise

import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
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
import androidx.compose.foundation.layout.RowScope
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
import androidx.compose.material.icons.rounded.SwapHoriz
import androidx.compose.material.icons.rounded.MusicNote
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.SkipNext
import androidx.compose.material.icons.rounded.Stop
import androidx.compose.material.icons.rounded.Tv
import androidx.compose.material3.Button
import com.perseusoft.homefi.ui.DialogAction
import com.perseusoft.homefi.ui.HomefiDialog
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
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
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
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.foundation.clickable
import androidx.compose.material.icons.rounded.KeyboardDoubleArrowUp
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.semantics.role
import com.perseusoft.homefi.core.HotPoint
import com.perseusoft.homefi.core.RideView
import com.perseusoft.homefi.core.StepView
import com.perseusoft.homefi.core.TargetUnit
import com.perseusoft.homefi.core.exerciseHotPoints
import com.perseusoft.homefi.ui.theme.LocalMotion
import kotlinx.coroutines.delay
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

/** The ring's range on a phone (its numbers still read from the saddle). */
private const val RING_MIN_DP = 160
private const val RING_MAX_DP = 260
private const val RING_PORTRAIT_MAX_DP = 250
private const val RING_PORTRAIT_MIN_DP = 140

/** Landscape: padding 12, the two columns weigh 1 : 1.5; below the ring come the effort, the song and (with a next part) its card. */
private const val SCREEN_PAD_DP = 12
private const val COLUMN_GAP_DP = 12
private const val COLUMN_I_SHARE = 1f / 2.5f
private const val BELOW_RING_DP = 92
private const val NEXT_CARD_DP = 60
private const val LEVEL_LINE_DP = 20

/** Portrait: the height everything but the ring and the mountain takes, and the mountain (smaller on a short phone). */
private const val COMPACT_MOUNTAIN_DP = 128
private const val MOUNTAIN_DP = 172
private const val SHORT_PHONE_DP = 700
private const val PROGRESS_ROW_DP = 24
private const val PHONE_FIXED_DP = 520
private const val ICON_CONTROL_DP = 48
private const val BIG_CONTROL_DP = 56
private const val PAUSE_DP = 64
private const val BIG_PAUSE_WIDTH_DP = 160
private const val RPM_SP = 96
private const val CLOCK_SP = 36
private const val RPM_SP_THREE_DIGITS = 80
private const val SPEED_SP_PER_RING = 0.30f
private const val STEP_CHIP_MS = 4_000L
private const val STEP_CHIP_FADE_MS = 150
private const val MIN_STEP_INDEX = 2

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

/** What the controls do; the screen builds it once and the icon row, the big pill and the paused overlay share it. */
private class RideActions(
    val zone: Color,
    val paused: Boolean,
    val lastPart: Boolean,
    val onPause: () -> Unit,
    val onSkip: () -> Unit,
    val onEnd: () -> Unit,
    val onEndHold: () -> Unit,
)

@Composable
private fun RideScreen(plan: RidePlan, session: RideSession) {
    val app = LocalContext.current.homefi
    val frame by session.frame.collectAsStateWithLifecycle()
    val tv by session.tv.collectAsStateWithLifecycle()
    val swapping by session.swapping.collectAsStateWithLifecycle()
    val settings by app.settings.exercise.collectAsStateWithLifecycle()
    val view = LocalView.current
    val haptics = haptics()
    var confirmEnd by remember { mutableStateOf(false) }
    var confirmSkip by remember { mutableStateOf(false) }
    var confirmSwap by remember { mutableStateOf(false) }
    val hotPoints = remember(plan) { exerciseHotPoints(plan.workout) }

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
    val paused = !current.playing && !tv.waitingForTv
    val actions = RideActions(
        zone, paused, v.segmentIndex.toInt() >= v.segmentCount.toInt() - 1,
        onPause = session::togglePause, onSkip = { confirmSkip = true }, onEnd = { confirmEnd = true }, onEndHold = session::end,
    )
    val swap = SwapState(settings.rideSwap, swapping != null) { confirmSwap = true }
    val bigControls = settings.rideBigControls

    BoxWithConstraints(Modifier.fillMaxSize().systemBarsPadding()) {
        val wide = maxWidth >= WIDE_DP.dp
        if (wide) {
            // the two columns: the ring with what the rider reads at arm's length, and the mountain with the controls over it
            val contentH = maxHeight.value - 2 * SCREEN_PAD_DP
            val columnI = (maxWidth.value - 2 * SCREEN_PAD_DP - COLUMN_GAP_DP) * COLUMN_I_SHARE
            val below = BELOW_RING_DP + (if (v.next != null) NEXT_CARD_DP else 0) + (if (v.level != null) LEVEL_LINE_DP else 0)
            val ringDp = minOf(columnI, contentH - below).toInt().coerceIn(RING_MIN_DP, RING_MAX_DP)
            Row(Modifier.fillMaxSize().padding(SCREEN_PAD_DP.dp), horizontalArrangement = Arrangement.spacedBy(COLUMN_GAP_DP.dp)) {
                Column(
                    Modifier.weight(1f).fillMaxHeight(),
                    verticalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterVertically),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Ring(current, settings, zone, ringDp)
                    EffortLine(v, zone)
                    NextCard(current)
                    PositionAndSong(current, swap)
                }
                Column(Modifier.weight(1.5f), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    TopRow(current, zone, session::close, showClock = false) { if (!bigControls) IconControls(actions) }
                    MountainBlock(plan, hotPoints, current, settings.hapticCues, Modifier.weight(1f), clockOver = true)
                    TvChip(tv)
                }
                if (bigControls) BigControls(actions, vertical = true)
            }
        } else {
            // the controls stay pinned at the bottom on any phone; the ring gives up size first
            val ringDp = (maxHeight.value - PHONE_FIXED_DP).toInt().coerceIn(RING_PORTRAIT_MIN_DP, RING_PORTRAIT_MAX_DP)
            val mountainDp = if (maxHeight < SHORT_PHONE_DP.dp) COMPACT_MOUNTAIN_DP else MOUNTAIN_DP
            Column(Modifier.fillMaxSize().padding(SCREEN_PAD_DP.dp)) {
                Column(
                    Modifier.weight(1f).verticalScroll(rememberScrollState()),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    TopRow(current, zone, session::close)
                    Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) { Ring(current, settings, zone, ringDp) }
                    EffortLine(v, zone)
                    PositionAndSong(current, swap)
                    NextCard(current)
                    MountainBlock(plan, hotPoints, current, settings.hapticCues, Modifier.height((mountainDp + PROGRESS_ROW_DP).dp))
                    TvChip(tv)
                }
                if (bigControls) BigControls(actions, vertical = false)
                else Row(Modifier.fillMaxWidth().padding(vertical = 8.dp), horizontalArrangement = Arrangement.SpaceEvenly, verticalAlignment = Alignment.CenterVertically) {
                    // stop on the left (thumb-safe), pause in the middle, skip on the right
                    HoldToEnd(actions.onEnd, actions.onEndHold)
                    PauseIcon(actions)
                    SkipIcon(actions)
                }
            }
        }
        if (paused) PausedOverlay(session::togglePause, onEnd = { confirmEnd = true }, onEndHold = session::end)
    }

    if (confirmSkip) {
        val next = v.next
        HomefiDialog(
            title = if (next != null) stringResource(R.string.ex_skip_q, stringResource(next.kind.nameRes())) else stringResource(R.string.ex_skip),
            text = next?.let { stringResource(R.string.ex_next_line_unit, it.shown.withUnit(), stringResource(it.position.nameRes())) },
            icon = Icons.Rounded.SkipNext,
            accent = next?.kind?.zoneColor() ?: zone,
            primary = DialogAction(stringResource(R.string.ex_skip_do)) { confirmSkip = false; session.skipSegment() },
            dismiss = stringResource(R.string.ex_keep),
            onDismiss = { confirmSkip = false },
        )
    }
    if (confirmEnd) {
        // the screen-reader path: holding is hard, so a dialog confirms
        HomefiDialog(
            title = stringResource(R.string.ex_end_q),
            icon = Icons.Rounded.Stop,
            accent = MaterialTheme.colorScheme.error,
            primary = DialogAction(stringResource(R.string.ex_end), danger = true) { confirmEnd = false; session.end() },
            dismiss = stringResource(R.string.ex_keep),
            onDismiss = { confirmEnd = false },
        )
    }
    if (confirmSwap) {
        val now = current.song?.let { plan.songs.indexOf(it) }?.takeIf { it >= 0 }
        val later = current.nextSong?.let { plan.songs.indexOf(it) }?.takeIf { it >= 0 }
        val swapNow = now?.let { i -> DialogAction(stringResource(R.string.ex_swap_do)) { confirmSwap = false; session.swapSong(i, true) } }
        val swapLater = later?.let { i -> DialogAction(stringResource(R.string.ex_swap_next)) { confirmSwap = false; session.swapSong(i, false) } }
        HomefiDialog(
            title = stringResource(R.string.ex_swap_title),
            text = stringResource(R.string.ex_swap_body, v.shown.rangeWithUnit()),
            icon = Icons.Rounded.SwapHoriz,
            accent = zone,
            primary = swapNow ?: swapLater,
            others = if (swapNow != null) listOfNotNull(swapLater) else emptyList(),
            dismiss = stringResource(R.string.ex_swap_keep),
            onDismiss = { confirmSwap = false },
        )
    }
}

/** Whether the song line can open the swap dialog, whether a swap is already running, and what a tap does. */
private class SwapState(val enabled: Boolean, val busy: Boolean, val onAsk: () -> Unit)

/**
 * Hide (back to the app, the ride goes on), the zone, the time left and, in landscape, the icon controls: one row.
 * In landscape the time left sits over the mountain instead ([showClock] false), so the part's name never shortens.
 */
@Composable
private fun TopRow(
    frame: RideFrame, zone: Color, onHide: () -> Unit, showClock: Boolean = true,
    trailing: @Composable RowScope.() -> Unit = {},
) {
    val v = frame.view
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        IconButton(onClick = onHide, modifier = Modifier.size(ICON_CONTROL_DP.dp)) {
            Icon(Icons.Rounded.KeyboardArrowDown, stringResource(R.string.ex_minimize))
        }
        // the part's name, with seated / standing as its icon, takes all the room the timer leaves
        Box(Modifier.weight(1f)) { PartChip(v, zone) }
        if (showClock) PartClock(v, CLOCK_SP, Modifier.padding(horizontal = 4.dp))
        trailing()
    }
}

/** The part's time left, big enough to read at arm's length. */
@Composable
private fun PartClock(v: RideView, sizeSp: Int, modifier: Modifier = Modifier) {
    Text(
        clock(v.remainingS.toInt()),
        fontSize = sizeSp.sp, fontWeight = FontWeight.ExtraBold, maxLines = 1,
        modifier = modifier.semantics { contentDescription = "" },
    )
}

/** The part's name in its zone colour, led by the seated / standing icon (read out as "Warm-up, Seated"). */
@Composable
private fun PartChip(v: RideView, zone: Color) {
    val name = stringResource(v.kind.nameRes())
    val position = stringResource(v.position.nameRes())
    Row(
        Modifier
            .border(BorderStroke(2.dp, zone), RoundedCornerShape(50))
            .padding(start = 10.dp, end = 12.dp, top = 6.dp, bottom = 6.dp)
            .semantics(mergeDescendants = true) { contentDescription = "$name, $position" },
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Icon(if (v.position == RidePosition.Seated) Icons.Rounded.EventSeat else Icons.Rounded.AccessibilityNew, null, Modifier.size(18.dp), tint = zone)
        Text(name, color = zone, fontSize = 15.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

/** The song playing, the whole line to itself; the line is the tap target that swaps the song (the arrows say so). */
@Composable
private fun PositionAndSong(frame: RideFrame, swap: SwapState) {
    val song = frame.song
    val canSwap = swap.enabled && song != null && !swap.busy
    val description = if (swap.enabled && song != null) stringResource(R.string.ex_swap_song_cd, song.entry.title) else null
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = ICON_CONTROL_DP.dp)
            .clip(RoundedCornerShape(12.dp))
            .then(if (canSwap) Modifier.clickable(role = Role.Button, onClick = swap.onAsk) else Modifier)
            .then(if (description != null) Modifier.semantics { contentDescription = description } else Modifier),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Icon(Icons.Rounded.MusicNote, null, Modifier.size(18.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
        Box(Modifier.weight(1f)) { SongLine(frame, swap.busy) }
        if (canSwap) Icon(Icons.Rounded.SwapHoriz, null, Modifier.size(20.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
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
    val line = stringResource(R.string.ex_next_line_unit, next.shown.withUnit(), stringResource(next.position.nameRes()))
    val announce = stringResource(R.string.ex_next_in, stringResource(next.kind.nameRes()), clock(v.remainingS.toInt()), next.shown.rpm.toInt())
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
            .padding(horizontal = 12.dp, vertical = 6.dp)
            .then(if (soon) Modifier.semantics { liveRegion = LiveRegionMode.Polite; contentDescription = announce } else Modifier),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Column(Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(stringResource(R.string.ex_next_label), fontSize = 11.sp, color = MaterialTheme.colorScheme.onSurfaceVariant, fontWeight = FontWeight.Bold)
                Text(stringResource(next.kind.nameRes()), color = zone, fontSize = 16.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
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

/**
 * The big target inside the beat pulse: RPM, or the bike's speed with "≈ N RPM" under it once calibrated. In a sprint
 * block the work or rest cadence and GO / REST.
 */
@Composable
private fun Ring(frame: RideFrame, settings: com.perseusoft.homefi.core.ExerciseSettings, zone: Color, sizeDp: Int) {
    val v = frame.view
    val phase = v.interval?.phase
    val tint = when (phase) {
        IntervalPhase.REST -> com.perseusoft.homefi.core.SegmentKind.RECOVERY.zoneColor()
        else -> zone
    }
    val speed = v.shown.unit != TargetUnit.RPM
    val digits = v.shown.valueText()
    val unitText = stringResource(v.shown.unit.labelRes())
    val description = if (speed) {
        stringResource(R.string.ex_target_speed, digits, unitText, v.shown.rpm.toInt())
    } else {
        stringResource(R.string.ex_target_rpm, v.targetRpm.toInt())
    }
    val unitSp = (RING_UNIT_PER_RING * sizeDp).coerceIn(12f, 16f)
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
                    color = tint, fontWeight = FontWeight.Black, fontSize = maxOf(12f, RING_GO_PER_RING * sizeDp).sp,
                )
            }
            Text(
                digits,
                // the number grows and shrinks with the ring (a short phone gets a smaller ring)
                fontSize = (if (speed) SPEED_SP_PER_RING * sizeDp else (if (digits.length > 2) RPM_SP_THREE_DIGITS else RPM_SP) * sizeDp.toFloat() / RING_DP).sp,
                fontWeight = FontWeight.ExtraBold, color = tint, textAlign = TextAlign.Center, maxLines = 1,
            )
            Text(unitText, fontSize = unitSp.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
            if (speed) {
                Text(
                    stringResource(R.string.ex_ring_approx_rpm, v.shown.rpm.toInt()),
                    fontSize = (RING_SMALL_PER_RING * sizeDp).coerceIn(12f, 18f).sp,
                    fontWeight = FontWeight.Medium, color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

private const val RING_UNIT_PER_RING = 0.075f
private const val RING_SMALL_PER_RING = 0.085f
private const val RING_GO_PER_RING = 0.08f

/** The ten bars, then "Hard 7" with "level 6/8" under it (the level only when the bike's knob positions are known). */
@Composable
private fun EffortLine(v: RideView, zone: Color) {
    val effort = v.effort.toInt()
    val level = v.level
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        EffortBars(effort, zone, Modifier.weight(1f, fill = false), barWidth = 12.dp, barHeight = 20.dp)
        Column(Modifier.weight(1f, fill = false)) {
            Text(
                "${stringResource(effortWordRes(effort))} $effort",
                fontSize = 16.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis,
            )
            if (level != null) {
                Text(
                    stringResource(R.string.ex_level_of, level.toInt(), v.levelCount.toInt()),
                    fontSize = 13.sp, color = zone, maxLines = 1, overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

@Composable
private fun SongLine(frame: RideFrame, swapping: Boolean) {
    val song = frame.song ?: return
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        if (swapping) CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp)
        Text(
            if (swapping) stringResource(R.string.ex_swap_working) else "${song.entry.title} · ${song.entry.artist}",
            maxLines = 1, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

/** The mountain with the step chip over it, and under it the progress row. */
@Composable
private fun MountainBlock(
    plan: RidePlan, hotPoints: List<HotPoint>, frame: RideFrame, haptic: Boolean, modifier: Modifier, clockOver: Boolean = false,
) {
    val v = frame.view
    Column(modifier, verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Box(Modifier.fillMaxWidth().weight(1f)) {
            MountainProfile(
                profile = plan.profile, marker = v.marker, segments = plan.workout.segments,
                paused = !frame.playing, hotPoints = hotPoints, modifier = Modifier.fillMaxSize(),
            )
            StepChip(v.step, v.kind.zoneColor(), haptic, Modifier.align(Alignment.TopCenter).padding(top = 8.dp))
            if (clockOver) PartClock(v, CLOCK_SP, Modifier.align(Alignment.TopEnd).padding(horizontal = 12.dp, vertical = 4.dp))
        }
        Row(Modifier.height(20.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(clock(v.elapsedS.toInt()), fontSize = 12.sp)
            Box(
                Modifier.weight(1f).height(6.dp).clip(CircleShape).background(MaterialTheme.colorScheme.surfaceVariant),
            ) { Box(Modifier.fillMaxHeight().fillMaxWidth(v.progress.coerceIn(0f, 1f)).background(v.kind.zoneColor())) }
            Text(clock(v.totalS.toInt()), fontSize = 12.sp)
        }
    }
}

/** "Resistance +1 · 2 of 3": shown for 4 s after each resistance step (the cue sounds at the same moment). */
@Composable
private fun StepChip(step: StepView?, zone: Color, haptic: Boolean, modifier: Modifier) {
    val moves = LocalMotion.current.moves
    val haptics = haptics()
    var visible by remember { mutableStateOf(false) }
    // the text outlives the step while the chip fades out
    var held by remember { mutableStateOf<StepView?>(null) }
    val index = step?.index
    LaunchedEffect(index) {
        if (step == null || step.index.toInt() < MIN_STEP_INDEX) {
            visible = false
            return@LaunchedEffect
        }
        held = step
        visible = true
        if (haptic) haptics.confirm()
        delay(STEP_CHIP_MS)
        visible = false
    }
    val slidePx = with(LocalDensity.current) { STEP_SLIDE_DP.dp.roundToPx() }
    val shown = held ?: return
    val text = stringResource(R.string.ex_step_chip, 1, shown.index.toInt(), shown.count.toInt())
    AnimatedVisibility(
        visible = visible,
        modifier = modifier,
        enter = if (moves) fadeIn(tween(STEP_CHIP_FADE_MS)) + slideInVertically(tween(STEP_CHIP_FADE_MS)) { -slidePx } else EnterTransition.None,
        exit = if (moves) fadeOut(tween(STEP_CHIP_FADE_MS)) else ExitTransition.None,
    ) {
        Row(
            Modifier
                .height(32.dp)
                .background(MaterialTheme.colorScheme.surface.copy(alpha = STEP_CHIP_SURFACE_ALPHA), RoundedCornerShape(50))
                .background(zone.copy(alpha = SOON_ALPHA), RoundedCornerShape(50))
                .border(BorderStroke(2.dp, zone), RoundedCornerShape(50))
                .padding(horizontal = 12.dp)
                .semantics { liveRegion = LiveRegionMode.Polite; contentDescription = text },
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Icon(Icons.Rounded.KeyboardDoubleArrowUp, null, Modifier.size(18.dp), tint = zone)
            Text(text, color = zone, fontSize = 14.sp, fontWeight = FontWeight.Bold, maxLines = 1)
        }
    }
}

private const val STEP_SLIDE_DP = 4
private const val STEP_CHIP_SURFACE_ALPHA = 0.92f

/** The small icon row: pause (the one filled thing), skip, stop (hold). */
@Composable
private fun IconControls(a: RideActions) {
    Row(horizontalArrangement = Arrangement.spacedBy(4.dp), verticalAlignment = Alignment.CenterVertically) {
        PauseIcon(a)
        SkipIcon(a)
        HoldToEnd(a.onEnd, a.onEndHold)
    }
}

@Composable
private fun PauseIcon(a: RideActions) {
    FilledIconButton(
        onClick = a.onPause,
        modifier = Modifier.size(ICON_CONTROL_DP.dp),
        colors = IconButtonDefaults.filledIconButtonColors(containerColor = a.zone, contentColor = Color.Black),
    ) {
        Icon(if (a.paused) Icons.Rounded.PlayArrow else Icons.Rounded.Pause, stringResource(if (a.paused) R.string.ex_resume else R.string.ex_pause))
    }
}

@Composable
private fun SkipIcon(a: RideActions) {
    IconButton(onClick = a.onSkip, enabled = !a.lastPart, modifier = Modifier.size(ICON_CONTROL_DP.dp)) {
        Icon(Icons.Rounded.SkipNext, stringResource(R.string.ex_skip))
    }
}

/** "Big ride buttons" (a setting): the large pause pill and the 56 dp buttons, as a column (landscape) or a row. */
@Composable
private fun BigControls(a: RideActions, vertical: Boolean) {
    val end = @Composable { HoldToEnd(a.onEnd, a.onEndHold, BIG_CONTROL_DP.dp) }
    val pause = @Composable { modifier: Modifier ->
        Button(
            onClick = a.onPause,
            modifier = modifier.height(PAUSE_DP.dp),
            shape = RoundedCornerShape(PAUSE_DP.dp / 2),
            colors = ButtonDefaults.buttonColors(containerColor = a.zone, contentColor = Color.Black),
        ) {
            Icon(if (a.paused) Icons.Rounded.PlayArrow else Icons.Rounded.Pause, null)
            Spacer(Modifier.width(8.dp))
            Text(stringResource(if (a.paused) R.string.ex_resume else R.string.ex_pause), fontSize = 18.sp)
        }
    }
    val skip = @Composable {
        IconButton(onClick = a.onSkip, enabled = !a.lastPart, modifier = Modifier.size(BIG_CONTROL_DP.dp)) {
            Icon(Icons.Rounded.SkipNext, stringResource(R.string.ex_skip))
        }
    }
    if (vertical) {
        Column(Modifier.fillMaxHeight().padding(end = 8.dp), verticalArrangement = Arrangement.spacedBy(16.dp, Alignment.CenterVertically)) {
            pause(Modifier.width(BIG_PAUSE_WIDTH_DP.dp)); skip(); end()
        }
    } else {
        Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            end(); pause(Modifier.weight(1f)); skip()
        }
    }
}

/** Stop button that ends the ride after [HOLD_TO_END_MS] of holding, with a ring filling around it. */
@Composable
private fun HoldToEnd(onAccessibleEnd: () -> Unit, onHeld: () -> Unit, size: Dp = ICON_CONTROL_DP.dp) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val fill = remember { Animatable(0f) }
    val hint = stringResource(R.string.ex_end_hold)
    val label = stringResource(R.string.ex_end)
    Box(
        Modifier
            .size(size)
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
