package com.perseusoft.homefi.ui.dj

import android.os.SystemClock
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Fill
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.SfxKind
import com.perseusoft.homefi.player.dj.DJ_DECKS
import com.perseusoft.homefi.player.dj.DjDeckUi
import com.perseusoft.homefi.player.dj.DjMixerUi
import com.perseusoft.homefi.ui.player.Art
import com.perseusoft.homefi.ui.player.Ink
import com.perseusoft.homefi.ui.theme.LocalMotion
import com.perseusoft.homefi.data.Motion
import kotlinx.coroutines.flow.Flow
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.max
import kotlin.math.min
import kotlin.math.sin

// Homifito DJ: the mascot behind his booth, drawn on a Canvas with the app's mascot paths. The maths is
// in HomifitoChoreography.kt (same as the desktop's); this keeps the per-frame state and draws it.

private const val MAX_DT_S = 0.1f
private const val MS_PER_S = 1000.0
private const val NS_PER_MS = 1_000_000L

/** A deck dragged this recently still counts as being scratched (positions arrive every 60 ms). */
private const val SCRATCH_HOLD_MS = 260L

/** The crossfader's hand stays on it this long after the knob last moved. */
private const val XF_HOLD_MS = 1100L
private const val XF_MOVED = 0.002f

/** How far a published playhead is extrapolated while it plays (ms). */
private const val EXTRAPOLATE_MAX_MS = 250.0
private const val BARS = 14
private const val BAR_FLOOR = 0.07f
private const val BAR_HZ = 5f
private const val BAR_SPREAD = 0.8f
private const val BAR_SIDE_SHIFT = 1.7f
private const val BEAT_GLOW = 0.5f
private const val LEDS = 5
private const val FALLBACK_LEVEL = 0.75f
private val STRIP_RADIUS = 16.dp
private val BAR_GAP = 4.dp
private val BAR_MAX_WIDTH = 14.dp

/** The mutable state of the frame loop: plain fields, drawn each frame (only [tick] is observed). */
private class StageState {
    var tick by mutableLongStateOf(0L)
    var pose = REST_POSE
    val deg = FloatArray(DJ_DECKS)
    val glow = FloatArray(DJ_DECKS)
    var xf = 0.5f
    var beat = 0f
    var pulse = 0f
    var seconds = 0.0
    var full = false
    var reaction: Reaction? = null
    val playing = BooleanArray(DJ_DECKS)
    val loaded = BooleanArray(DJ_DECKS)

    // what the loop remembers between frames
    var lastFrameMs = 0L
    var reactionAt = 0L
    val published = DoubleArray(DJ_DECKS)
    val publishedAt = LongArray(DJ_DECKS)
    val positionWas = LongArray(DJ_DECKS)
    val scratchUntil = LongArray(DJ_DECKS)
    var xfWas = 0.5f
    var xfTouchUntil = 0L
    var xfHand: Int? = null

    /** The playhead as of [nowMs]: the last published one, carried on while the deck plays. */
    fun positionMs(deck: DjDeckUi, i: Int, nowMs: Long): Double {
        val base = deck.positionMs.toDouble()
        if (base != published[i]) {
            published[i] = base
            publishedAt[i] = nowMs
        }
        return if (deck.playing) base + min(EXTRAPOLATE_MAX_MS, (nowMs - publishedAt[i]).toDouble()) * deck.speed else base
    }

    fun step(decks: List<DjDeckUi>, mixer: DjMixerUi, motion: StageMotion, nowMs: Long, dt: Float) {
        val seconds = nowMs / MS_PER_S
        full = motion == StageMotion.FULL
        val master = masterDeck(decks, mixer.crossfader)
        var phase = freePhase(seconds)
        var beatIndex = (seconds * FREE_BPM / 60.0).toInt()
        val position = DoubleArray(DJ_DECKS) { positionMs(decks[it], it, nowMs) }
        if (master != null) {
            beatPhase(decks[master], position[master])?.let {
                phase = it
                beatIndex = beatNumber(decks[master], position[master])
            }
        }
        if (reaction != null && nowMs - reactionAt >= reaction!!.durationMs) reaction = null

        val scratching = BooleanArray(DJ_DECKS)
        for (i in 0 until DJ_DECKS) {
            val moved = decks[i].positionMs - positionWas[i]
            if (isScratching(decks[i], moved)) scratchUntil[i] = nowMs + SCRATCH_HOLD_MS
            positionWas[i] = decks[i].positionMs
            scratching[i] = nowMs < scratchUntil[i]
            playing[i] = decks[i].playing
            loaded[i] = decks[i].loaded
        }

        // a hand on the crossfader while it moves or a mix runs
        val xfNow = mixer.crossfader
        if (abs(xfNow - xfWas) > XF_MOVED || mixer.mix != null) {
            if (xfHand == null || nowMs >= xfTouchUntil) xfHand = if (xfNow < 0.5f) 0 else 1
            xfTouchUntil = nowMs + XF_HOLD_MS
        }
        xfWas = xfNow
        if (nowMs >= xfTouchUntil) xfHand = null

        val r = reaction
        val target = aim(
            StageInput(
                motion, phase.toFloat(), beatIndex, playing, scratching, cueingDeck(decks), xfNow, xfHand,
                r, if (r != null) (nowMs - reactionAt).toFloat() / r.durationMs else 0f, seconds,
            ),
        )
        pose = approach(pose, target, dt)
        xf = xfNow
        this.seconds = seconds
        val lively = playing[0] || playing[1]
        pulse = if (lively) 1f - phase.toFloat() else 0f
        beat = if (full && lively) 1f - min(1f, phase.toFloat() * 4f) else 0f
        for (i in 0 until DJ_DECKS) {
            deg[i] = if (full && loaded[i]) recordDegrees(position[i]) else 0f
            val weight = if (i == 0) 1f - xfNow else xfNow
            glow[i] = if (playing[i]) 0.3f + 0.5f * min(1f, weight * 1.6f) + (if (full) BEAT_GLOW * 0.4f * pulse * pulse else 0f) else 0f
        }
        tick++
    }

    fun react(kind: SfxKind, nowMs: Long) {
        reaction = Reaction.of(kind)
        reactionAt = nowMs
    }
}

private fun path(d: String): Path = PathParser().parsePathString(d).toPath()

private object StageArt {
    val band = path("M226 560C226 128 798 128 798 560")
    val happyEyes = path("M406 610Q440 566 474 610M550 610Q584 566 618 610")
    val openSmile = path("M436 668Q512 790 588 668Q512 700 436 668Z")
    val tongue = path("M470 740Q512 760 554 740Q512 722 470 740Z")
}

private val INK_CONSOLE = Color(0xFF17263F)
private val INK_TOP = Color(0xFF2C4260)
private val INK_WELL = Color(0xFF101C30)
private val INK_PLATTER = Color(0xFF0A1220)
private val INK_VINYL = Color(0xFF05090F)
private val INK_STEEL = Color(0xFF33475F)
private val INK_SPEAKER = Color(0xFF14223A)
private val INK_WOOFER = Color(0xFF0B1524)
private val INK_CONE = Color(0xFF26384F)
private val INK_METAL = Color(0xFF9FB3C8)
private val INK_KNOB = Color(0xFFE8EEF5)
private val INK_LED_HOT = Color(0xFFF87171)
private val HOUSE_TOP_COLOR = Color(0xFFFFFFFF)
private val HOUSE_BOTTOM_COLOR = Color(0xFFDDEFEA)
private val BAND_SHINE = Color(0xFF5B86B8)

/**
 * Homifito DJ: the strip above or below the decks. [decks] / [mixer] are the session's state, [pads] its
 * sampler events, [motion] the app's motion setting (FULL is the show, SUBTLE a gentle sway, OFF a still picture).
 */
@Composable
fun HomifitoStrip(
    decks: List<DjDeckUi>,
    mixer: DjMixerUi,
    pads: Flow<SfxKind>,
    height: Dp,
    modifier: Modifier = Modifier,
) {
    val level = LocalMotion.current.level
    val motion = when (level) {
        Motion.OFF -> StageMotion.OFF
        Motion.SUBTLE -> StageMotion.SUBTLE
        else -> StageMotion.FULL
    }
    val state = remember { StageState() }
    val latestDecks by rememberUpdatedState(decks)
    val latestMixer by rememberUpdatedState(mixer)
    val colorA = deckColor(0)
    val colorB = deckColor(1)
    val scheme = MaterialTheme.colorScheme
    val description = stringResource(R.string.dj_homifito_desc)

    if (motion == StageMotion.OFF) {
        // a still picture of the state, redrawn when the session changes it
        LaunchedEffect(decks, mixer) { state.step(decks, mixer, motion, SystemClock.uptimeMillis(), Float.POSITIVE_INFINITY) }
    } else {
        LaunchedEffect(motion) {
            state.lastFrameMs = 0L
            while (true) {
                androidx.compose.runtime.withFrameNanos { nanos ->
                    val now = nanos / NS_PER_MS
                    val dt = if (state.lastFrameMs == 0L) 0f else min(MAX_DT_S, (now - state.lastFrameMs) / 1000f)
                    state.lastFrameMs = now
                    state.step(latestDecks, latestMixer, motion, now, dt)
                }
            }
        }
        LaunchedEffect(pads) {
            pads.collect { state.react(it, state.lastFrameMs) }
        }
    }

    val density = LocalDensity.current
    val barGap = with(density) { BAR_GAP.toPx() }
    val barMax = with(density) { BAR_MAX_WIDTH.toPx() }
    Canvas(
        modifier
            .fillMaxWidth()
            .height(height)
            .clip(RoundedCornerShape(STRIP_RADIUS))
            .border(1.dp, scheme.outlineVariant, RoundedCornerShape(STRIP_RADIUS))
            .semantics { contentDescription = description },
    ) {
        state.tick // redraw every frame
        drawRect(scheme.surfaceContainerLow)
        drawGlow(state, colorA, colorB)
        drawWalls(state, colorA, colorB, barGap, barMax)
        val scale = min(size.width / STAGE_W, size.height / (STAGE_H - STAGE_TOP))
        withTransform({
            translate((size.width - STAGE_W * scale) / 2f, size.height - STAGE_H * scale)
            scale(scale, scale, Offset.Zero)
        }) {
            drawBeams(state, colorA, colorB)
            drawSpeakers(state)
            drawHomifito(state)
            drawBooth(state, colorA, colorB)
            drawArms(state)
            if (state.reaction == Reaction.HANDS_UP || state.reaction == Reaction.JUMP) drawSparkles()
        }
    }
}

private fun DrawScope.drawGlow(s: StageState, a: Color, b: Color) {
    val r = size.width * 0.34f
    fun spill(color: Color, x: Float, g: Float) {
        if (g <= 0f) return
        drawRect(Brush.radialGradient(listOf(color.copy(alpha = g * 0.42f), Color.Transparent), Offset(x, size.height * 1.05f), r))
    }
    spill(a, size.width * 0.14f, s.glow[0])
    spill(b, size.width * 0.86f, s.glow[1])
}

/** The walls of equalizer bars on both sides of the stage (they fade towards the middle). */
private fun DrawScope.drawWalls(s: StageState, a: Color, b: Color, gap: Float, maxBar: Float) {
    val wall = size.width / 2f - size.height * 1.3f
    if (wall < gap * 2) return
    val bar = ((wall - gap * (BARS + 1)) / BARS).coerceIn(0f, maxBar)
    if (bar < 2f) return
    val full = size.height * 0.78f
    for (d in 0 until DJ_DECKS) {
        val color = if (d == 0) a else b
        for (n in 0 until BARS) {
            val h = barHeight(s, d, n)
            val fromEdge = gap + n * (bar + gap)
            val t = n / (BARS - 1f)
            val x = if (d == 0) fromEdge else size.width - fromEdge - bar
            drawRect(color.copy(alpha = 0.8f * (1f - max(0f, (t - 0.4f) / 0.6f))), Offset(x, size.height - h * full), Size(bar, h * full))
        }
    }
}

private fun barHeight(s: StageState, d: Int, n: Int): Float {
    if (!s.playing[d]) return BAR_FLOOR
    val reach = 0.25f + 0.75f * FALLBACK_LEVEL * (0.6f + 0.4f * s.glow[d].coerceAtMost(1f))
    if (!s.full) return max(BAR_FLOOR, reach * (0.3f + 0.35f * (n % 3) / 2f))
    val wave = abs(sin(s.seconds * BAR_HZ + n * BAR_SPREAD + d * BAR_SIDE_SHIFT)).toFloat()
    return max(BAR_FLOOR, reach * (0.3f + 0.7f * wave) * (0.65f + 0.35f * s.pulse))
}

private fun polygon(vararg p: Float): Path = Path().apply {
    moveTo(p[0], p[1])
    for (i in 2 until p.size step 2) lineTo(p[i], p[i + 1])
    close()
}

/** A spotlight in each deck's colour, brighter on the beat while that deck plays. */
private fun DrawScope.drawBeams(s: StageState, a: Color, b: Color) {
    fun beam(color: Color, g: Float, path: Path) {
        if (g <= 0f) return
        drawPath(path, Brush.verticalGradient(listOf(color.copy(alpha = 0.55f * g), Color.Transparent), startY = STAGE_TOP, endY = 300f))
    }
    beam(a, s.glow[0], polygon(150f, STAGE_TOP, 210f, STAGE_TOP, 400f, 300f, 20f, 300f))
    beam(b, s.glow[1], polygon(790f, STAGE_TOP, 850f, STAGE_TOP, 980f, 300f, 600f, 300f))
}

/** Two speakers: the woofers push out on the beat. */
private fun DrawScope.drawSpeakers(s: StageState) {
    for (x in floatArrayOf(22f, 912f)) {
        drawRoundRect(INK_SPEAKER, Offset(x, 176f), Size(66f, 230f), androidx.compose.ui.geometry.CornerRadius(10f))
        drawRoundRect(Color.White.copy(alpha = 0.1f), Offset(x, 176f), Size(66f, 230f), androidx.compose.ui.geometry.CornerRadius(10f), style = Stroke(1f))
        listOf(236f to 18f, 330f to 24f).forEachIndexed { n, (cy, r) ->
            drawCircle(INK_WOOFER, r, Offset(x + 33f, cy))
            drawCircle(INK_STEEL, r, Offset(x + 33f, cy), style = Stroke(2f))
            drawCircle(INK_CONE, (if (n == 1) 11f else 8f) * (1f + 0.28f * s.beat), Offset(x + 33f, cy))
        }
        drawCircle(Ink.cushion.copy(alpha = if (s.beat > 0f) 1f else 0.35f), 3.5f, Offset(x + 33f, 194f))
    }
}

/** The logo's house, swaying and dropping on the beat (the art is 1024 wide, scaled onto the stage). */
private fun DrawScope.drawHomifito(s: StageState) {
    val pose = s.pose
    withTransform({
        translate(0f, pose.bodyY)
        rotate(pose.tilt, PIVOT)
        translate(STAGE_W / 2 - 512f * HOUSE_SCALE, HOUSE_TOP_Y - 288f * HOUSE_SCALE)
        scale(HOUSE_SCALE, HOUSE_SCALE, Offset.Zero)
        rotate(-6f, Offset(512f, 560f))
    }) {
        drawPath(Art.house, Brush.verticalGradient(listOf(HOUSE_TOP_COLOR, HOUSE_BOTTOM_COLOR), startY = 288f, endY = 806f))
        drawPath(Art.house, Ink.navy.copy(alpha = 0.18f), style = Stroke(10f))
        drawPath(StageArt.band, Ink.navy, style = Stroke(50f, cap = StrokeCap.Round))
        drawPath(StageArt.band, BAND_SHINE.copy(alpha = 0.5f), style = Stroke(10f, cap = StrokeCap.Round))
        drawCup(pose, 0, Art.cupL, Art.padL, 242f)
        drawCup(pose, 1, Art.cupR, Art.padR, 782f)
        if (pose.happy) {
            drawPath(StageArt.happyEyes, Ink.navy, style = Stroke(18f, cap = StrokeCap.Round))
            drawPath(StageArt.openSmile, Ink.navy)
            drawPath(StageArt.tongue, Ink.pink)
        } else {
            drawPath(Art.eyes, Ink.navy)
            drawPath(Art.sparkles, Ink.white)
            drawPath(Art.smile, Ink.navy)
        }
        drawPath(Art.cheeks, Ink.pink.copy(alpha = 0.6f))
    }
}

private const val CUP_Y = 606f
private const val CUP_LIFT_X = 34f
private const val CUP_LIFT_Y = 78f
private const val CUP_LIFT_DEG = 26f

private fun DrawScope.drawCup(pose: Pose, side: Int, cup: Path, pad: Path, cupX: Float) {
    val lift = if (pose.cupSide == side) pose.cupLift else 0f
    val sign = if (side == 0) -1f else 1f
    withTransform({
        translate(sign * CUP_LIFT_X * lift, -CUP_LIFT_Y * lift)
        rotate(sign * CUP_LIFT_DEG * lift, Offset(cupX, CUP_Y))
    }) {
        drawPath(cup, Ink.navy)
        drawPath(pad, Ink.cushion)
    }
}

/** The console: two turntables whose records turn with the playheads, a mixer with its crossfader. */
private fun DrawScope.drawBooth(s: StageState, a: Color, b: Color) {
    val round = androidx.compose.ui.geometry.CornerRadius(22f)
    drawRoundRect(INK_CONSOLE, Offset(70f, 268f), Size(860f, 132f), round)
    drawRoundRect(Color.White.copy(alpha = 0.14f), Offset(70f, 268f), Size(860f, 132f), round, style = Stroke(1f))
    drawRoundRect(Brush.verticalGradient(listOf(INK_TOP, Color(0xFF1D2F49)), startY = 274f, endY = 378f), Offset(84f, 274f), Size(832f, 104f), androidx.compose.ui.geometry.CornerRadius(16f))
    drawRoundRect(INK_WELL, Offset(84f, 378f), Size(832f, 22f), androidx.compose.ui.geometry.CornerRadius(8f))
    for (d in 0 until DJ_DECKS) {
        val c = PLATTER[d]
        val color = if (d == 0) a else b
        val dir = if (d == 0) 1f else -1f
        drawRoundRect(INK_WELL, Offset(c.x - 128f, 282f), Size(256f, 88f), androidx.compose.ui.geometry.CornerRadius(14f))
        fun oval(r: Float, fill: Color?, stroke: Color? = null, width: Float = 1f) {
            val tl = Offset(c.x - r, c.y - r * PLATTER_SQUASH)
            val sz = Size(r * 2, r * 2 * PLATTER_SQUASH)
            if (fill != null) drawOval(fill, tl, sz)
            if (stroke != null) drawOval(stroke, tl, sz, style = Stroke(width))
        }
        oval(PLATTER_RX + 8f, INK_PLATTER, color.copy(alpha = if (s.playing[d]) 0.95f else 0.3f), 3f)
        oval(PLATTER_RX, INK_VINYL)
        oval(78f, null, Color.White.copy(alpha = 0.07f))
        oval(58f, null, Color.White.copy(alpha = 0.07f))
        for (off in floatArrayOf(0f, 180f)) {
            drawLine(Color.White.copy(alpha = 0.55f), platterDot(d, s.deg[d] + off, 34f), platterDot(d, s.deg[d] + off, 90f), 3f, StrokeCap.Round)
        }
        oval(30f, color.copy(alpha = if (s.loaded[d]) 1f else 0.35f))
        drawCircle(INK_VINYL, 3f, c)
        drawCircle(INK_STEEL, 7f, Offset(c.x + dir * 112f, 298f))
        drawLine(INK_METAL, Offset(c.x + dir * 112f, 298f), Offset(c.x + dir * 60f, c.y + 4f), 4f, StrokeCap.Round)
    }
    // the mixer
    drawRoundRect(INK_WELL, Offset(398f, 284f), Size(204f, 86f), androidx.compose.ui.geometry.CornerRadius(10f))
    floatArrayOf(424f, 452f, 548f, 576f).forEachIndexed { n, kx ->
        drawCircle(INK_STEEL, 8f, Offset(kx, 304f))
        drawLine(INK_KNOB, Offset(kx, 304f), Offset(kx + if (n % 2 == 1) 4f else -4f, 298f), 2.4f, StrokeCap.Round)
    }
    for (d in 0 until DJ_DECKS) for (n in 0 until LEDS) {
        val lit = min(1f, s.glow[d]) * LEDS > n + 0.3f
        val color = if (n >= LEDS - 1) INK_LED_HOT else if (d == 0) a else b
        drawRoundRect(color.copy(alpha = if (lit) 1f else 0.18f), Offset(if (d == 0) 472f else 516f, 338f - n * 8f - 4f), Size(12f, 5f), androidx.compose.ui.geometry.CornerRadius(1.5f))
    }
    drawRoundRect(Color(0xFF05101A), Offset(XF_X1, XF_Y - 3f), Size(XF_X2 - XF_X1, 6f), androidx.compose.ui.geometry.CornerRadius(3f))
    val knob = crossfaderKnob(s.xf)
    drawRoundRect(INK_KNOB, Offset(knob.x - 12f, XF_Y - 11f), Size(24f, 16f), androidx.compose.ui.geometry.CornerRadius(4f))
    drawLine(Ink.navy, Offset(knob.x, XF_Y - 9f), Offset(knob.x, XF_Y + 3f), 2.4f)
}

/** His arms, in front of the booth. */
private fun DrawScope.drawArms(s: StageState) {
    val pose = s.pose
    val arms = listOf(shoulderAt(SHOULDER_L, pose) to pose.handL, shoulderAt(SHOULDER_R, pose) to pose.handR)
    val paths = arms.map { (from, to) ->
        val elbow = armElbow(from, to)
        Path().apply { moveTo(from.x, from.y); quadraticTo(elbow.x, elbow.y, to.x, to.y) }
    }
    paths.forEach { drawPath(it, Ink.navy, style = Stroke(25f, cap = StrokeCap.Round)) }
    paths.forEach { drawPath(it, Ink.white, style = Stroke(16f, cap = StrokeCap.Round)) }
    arms.forEach { (_, hand) ->
        drawCircle(Ink.navy, HAND_R + 4f, hand)
        drawCircle(Ink.white, HAND_R, hand)
    }
}

/** Three small stars while his hands are up or he jumps. */
private fun DrawScope.drawSparkles() {
    fun star(angle: Float, radius: Float, size: Float) {
        val cx = PIVOT.x + cos(angle) * radius
        val cy = HOUSE_TOP_Y + 40f + sin(angle) * radius * 0.55f
        val p = Path()
        for (i in 0 until 8) {
            val r = if (i % 2 == 0) size else size * 0.35f
            val a = (i * PI / 4).toFloat()
            val x = cx + sin(a) * r
            val y = cy - cos(a) * r
            if (i == 0) p.moveTo(x, y) else p.lineTo(x, y)
        }
        p.close()
        drawPath(p, Ink.note, style = Fill)
    }
    star(-2.6f, 190f, 18f)
    star(-0.55f, 200f, 22f)
    star(-1.6f, 150f, 14f)
}
