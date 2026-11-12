package com.perseusoft.homefi.ui.exercise

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.background
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.PresetId
import com.perseusoft.homefi.core.ProfilePoint
import com.perseusoft.homefi.core.PulseStyle
import com.perseusoft.homefi.core.RidePosition
import com.perseusoft.homefi.core.RidePulse
import com.perseusoft.homefi.core.RideSegment
import com.perseusoft.homefi.core.ShownTarget
import com.perseusoft.homefi.core.TargetUnit
import com.perseusoft.homefi.core.SegmentKind
import kotlin.math.max
import kotlin.math.pow
import kotlin.math.roundToInt
import androidx.compose.ui.res.stringResource

// The look of Exercise lives in this file and a few small composables, so a restyle touches little.
// Zone colours: one per kind of segment, readable on dark and light surfaces.
// On dark surfaces (the ride screen is always dark) the zones are the mountain's own colours
// (`ZONE_COLOR` in CourseGeometry.kt, docs/MOUNTAIN.md), so chips, cards and the course agree.
private val ZONE_DARK = ZONE_COLOR
private val ZONE_LIGHT = mapOf(
    SegmentKind.WARM_UP to Color(0xFF1D4ED8),
    SegmentKind.SEATED_FLAT to Color(0xFF15803D),
    SegmentKind.SEATED_CLIMB to Color(0xFFB45309),
    SegmentKind.STANDING_CLIMB to Color(0xFFC2410C),
    SegmentKind.JUMPS to Color(0xFF7E22CE),
    SegmentKind.SPRINT to Color(0xFFBE123C),
    SegmentKind.RECOVERY to Color(0xFF0F766E),
    SegmentKind.COOL_DOWN to Color(0xFF4338CC),
)

/** The zone colour of a segment on the current surface (light or dark). */
@Composable
fun SegmentKind.zoneColor(): Color {
    val dark = MaterialTheme.colorScheme.background.luminance() < DARK_LUMINANCE
    return (if (dark) ZONE_DARK else ZONE_LIGHT).getValue(this)
}

fun SegmentKind.nameRes(): Int = when (this) {
    SegmentKind.WARM_UP -> R.string.ex_kind_warm_up
    SegmentKind.SEATED_FLAT -> R.string.ex_kind_flat
    SegmentKind.SEATED_CLIMB -> R.string.ex_kind_climb
    SegmentKind.STANDING_CLIMB -> R.string.ex_kind_standing_climb
    SegmentKind.JUMPS -> R.string.ex_kind_jumps
    SegmentKind.SPRINT -> R.string.ex_kind_sprint
    SegmentKind.RECOVERY -> R.string.ex_kind_recovery
    SegmentKind.COOL_DOWN -> R.string.ex_kind_cool_down
}

fun RidePosition.nameRes(): Int = when (this) {
    RidePosition.Seated -> R.string.ex_pos_seated
    RidePosition.Standing -> R.string.ex_pos_standing
    is RidePosition.Alternating -> R.string.ex_pos_alternating
}

fun TargetUnit.labelRes(): Int = when (this) {
    TargetUnit.KPH -> R.string.ex_console_kph
    TargetUnit.MPH -> R.string.ex_console_mph
    TargetUnit.RPM -> R.string.ex_rpm
}

/** The big number of a target: the cadence in RPM, the speed with one decimal otherwise. */
fun ShownTarget.valueText(): String = if (unit == TargetUnit.RPM) rpm.toString() else "%.1f".format(value)

/** "75 RPM" or "17.5 km/h". */
@Composable
fun ShownTarget.withUnit(): String = "${valueText()} ${stringResource(unit.labelRes())}"

/** "70-80 RPM" or "17.4-19.2 km/h" (one number when the range is a single value). */
@Composable
fun ShownTarget.rangeWithUnit(): String {
    val unitText = stringResource(unit.labelRes())
    val (lo, hi) = if (unit == TargetUnit.RPM) min.roundToInt().toString() to max.roundToInt().toString() else "%.1f".format(min) to "%.1f".format(max)
    return if (lo == hi) "$lo $unitText" else "$lo-$hi $unitText"
}

/** Effort as words plus the number ("Comfortable 4"): a resistance cue, not a measurement. */
fun effortWordRes(effort: Int): Int = when {
    effort <= EFFORT_VERY_EASY -> R.string.ex_effort_very_easy
    effort <= EFFORT_COMFORTABLE -> R.string.ex_effort_comfortable
    effort <= EFFORT_STEADY -> R.string.ex_effort_steady
    effort <= EFFORT_HARD -> R.string.ex_effort_hard
    else -> R.string.ex_effort_all_out
}

private const val EFFORT_VERY_EASY = 2
private const val EFFORT_COMFORTABLE = 4
private const val EFFORT_STEADY = 6
private const val EFFORT_HARD = 8
const val EFFORT_STEPS = 10
private const val DARK_LUMINANCE = 0.5f
private const val SECONDS_PER_MINUTE = 60

/** `m:ss` (or `h:mm:ss` from an hour). */
fun clock(seconds: Int): String {
    val s = seconds.coerceAtLeast(0)
    val h = s / (SECONDS_PER_MINUTE * SECONDS_PER_MINUTE)
    val m = s / SECONDS_PER_MINUTE % SECONDS_PER_MINUTE
    val sec = s % SECONDS_PER_MINUTE
    return if (h > 0) "%d:%02d:%02d".format(h, m, sec) else "%d:%02d".format(m, sec)
}

/** The ten bars of effort: lit up to [effort]. */
@Composable
fun EffortBars(effort: Int, color: Color, modifier: Modifier = Modifier, barWidth: Dp = 14.dp, barHeight: Dp = 22.dp) {
    val off = MaterialTheme.colorScheme.surfaceVariant
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(barWidth / 3)) {
        repeat(EFFORT_STEPS) { i ->
            Box(
                Modifier
                    .size(barWidth, barHeight)
                    .background(if (i < effort) color else off, RoundedCornerShape(barWidth / 4)),
            )
        }
    }
}

/**
 * The beat the rider pedals to. [positionNow] gives the song's position now (ms) every frame; the pulse
 * is computed from the beat grid, not detected. [content] sits inside (the big RPM).
 */
@Composable
fun BeatPulse(
    pulse: RidePulse,
    active: Boolean,
    positionNow: () -> Double,
    offsetMs: Int,
    style: PulseStyle,
    color: Color,
    modifier: Modifier = Modifier,
    ringWidth: Dp = 8.dp,
    content: @Composable () -> Unit,
) {
    var frameNs by remember { mutableLongStateOf(0L) }
    val moving = active && pulse.strokeMs > 0.0
    LaunchedEffect(moving) {
        while (moving) withFrameNanos { frameNs = it }
    }
    // 0 on the stroke, rising to 1 just before the next; reading [frame] in a draw block redraws it every frame
    fun phase(frame: Long): Float {
        if (!moving || frame < 0) return 0f
        val p = (positionNow() - offsetMs - pulse.firstBeatMs) / pulse.strokeMs
        return (((p % 1.0) + 1.0) % 1.0).toFloat()
    }
    // 1 on the stroke, fading out
    fun hit(frame: Long): Float = if (moving) (1.0 - phase(frame)).pow(PULSE_DECAY).toFloat() else 0f
    Box(modifier, contentAlignment = Alignment.Center) {
        when (style) {
            PulseStyle.FLASH -> Canvas(Modifier.fillMaxSize()) {
                drawRect(color.copy(alpha = hit(frameNs) * FLASH_ALPHA))
            }
            PulseStyle.RING -> Canvas(Modifier.fillMaxSize()) {
                val k = hit(frameNs)
                val r = size.minDimension / 2 - ringWidth.toPx() * 2
                drawCircle(color.copy(alpha = k * GLOW_ALPHA), r * (1f + k * RING_SWELL))
                drawCircle(color.copy(alpha = RING_REST_ALPHA), r, style = Stroke(ringWidth.toPx()))
                drawCircle(
                    color.copy(alpha = RING_REST_ALPHA + k * (1 - RING_REST_ALPHA)),
                    r * (1f + k * RING_SWELL),
                    style = Stroke(ringWidth.toPx() * (1f + k)),
                )
            }
            PulseStyle.BAR -> Unit
        }
        content()
        if (style == PulseStyle.BAR) {
            Canvas(Modifier.fillMaxWidth().height(ringWidth).align(Alignment.BottomCenter)) {
                val round = CornerRadius(size.height / 2)
                drawRoundRect(color.copy(alpha = RING_REST_ALPHA), cornerRadius = round)
                drawRoundRect(color, size = Size(size.width * phase(frameNs), size.height), cornerRadius = round)
            }
        }
    }
}

private const val GLOW_ALPHA = 0.34f
private const val PULSE_DECAY = 2.5
private const val FLASH_ALPHA = 0.28f
private const val RING_REST_ALPHA = 0.25f
private const val RING_SWELL = 0.07f

/** The style's name as the setup and the lists show it. */
fun PresetId.labelRes(): Int = when (this) {
    PresetId.FIRST_RIDE -> R.string.ex_style_first
    PresetId.EASY -> R.string.ex_style_easy
    PresetId.CLASSIC -> R.string.ex_style_classic
    PresetId.INTERVALS -> R.string.ex_style_intervals
}

private const val MAX_EFFORT = 10f

/**
 * A rough mountain for a style card, before any ride exists: each part is a plateau as high as its effort.
 * A built ride always draws the core's own profile.
 */
fun stylePreviewProfile(segments: List<RideSegment>): List<ProfilePoint> {
    var at = 0u
    val points = ArrayList<ProfilePoint>(segments.size * 2)
    segments.forEach { seg ->
        val height = seg.effort.toFloat() / MAX_EFFORT
        points += ProfilePoint(at, height)
        at += seg.seconds
        points += ProfilePoint(at, height)
    }
    return points
}
