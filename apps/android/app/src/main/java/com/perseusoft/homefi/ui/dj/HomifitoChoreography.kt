package com.perseusoft.homefi.ui.dj

import androidx.compose.ui.geometry.Offset
import com.perseusoft.homefi.core.SfxKind
import com.perseusoft.homefi.player.dj.DjDeckUi
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.exp
import kotlin.math.floor
import kotlin.math.hypot
import kotlin.math.min
import kotlin.math.sin

// Homifito DJ: the pure part (no Compose state, no clock). Where he stands, what he does for what DJ
// mode is doing (bob on the master beat, hands on the decks, scratching, a hand to the crossfader,
// a reaction to a sampler pad, one headphone cup off the ear while the other deck is cued) and the
// damping that makes it smooth. Same numbers as the desktop's `lib/homifito-dj.ts` (keep in sync);
// `HomifitoStage.kt` draws it.

/** How much he is allowed to move: the app's motion setting resolved. */
enum class StageMotion { FULL, SUBTLE, OFF }

// ------------------------------------------------------------------ the stage (stage units)
const val STAGE_W = 1000f
const val STAGE_H = 400f

/** Negative: the headphone band and a raised hand reach above the console's row. */
const val STAGE_TOP = -84f

/** Homifito's house is the logo's 1024 drawing scaled by this and hung from [HOUSE_TOP_Y]. */
const val HOUSE_SCALE = 0.46f
const val HOUSE_TOP_Y = 26f
private const val HOUSE_ART_TOP = 288f
private const val HOUSE_ART_BOTTOM = 806f
private const val HOUSE_ART_MID_X = 512f

/** Where the house's bottom middle is on the stage: the pivot of his sway. */
val PIVOT = Offset(STAGE_W / 2, HOUSE_TOP_Y + (HOUSE_ART_BOTTOM - HOUSE_ART_TOP) * HOUSE_SCALE)

private fun houseToStage(x: Float, y: Float) =
    Offset(STAGE_W / 2 + (x - HOUSE_ART_MID_X) * HOUSE_SCALE, HOUSE_TOP_Y + (y - HOUSE_ART_TOP) * HOUSE_SCALE)

val SHOULDER_L = houseToStage(268f, 730f)
val SHOULDER_R = houseToStage(756f, 730f)

/** The decks' platters (centre) and the crossfader's slot, on the console. */
val PLATTER = listOf(Offset(240f, 322f), Offset(760f, 322f))
const val PLATTER_RX = 98f

/** The platter is seen from above at an angle: its height over its width. */
const val PLATTER_SQUASH = 0.36f
const val XF_X1 = 438f
const val XF_X2 = 562f
const val XF_Y = 354f
const val HAND_R = 15f

private val REST_L = Offset(292f, 304f)
private val REST_R = Offset(708f, 304f)
private val UP_L = Offset(330f, 52f)
private val UP_R = Offset(670f, 52f)
private val JUMP_L = Offset(372f, 140f)
private val JUMP_R = Offset(628f, 140f)
private const val SCRATCH_REACH = 34f
private const val SCRATCH_DROP = 14f
private const val TAP_PX = 7f

// ------------------------------------------------------------------ beat
/** A track's degrees per second on a 33 1/3 rpm record. */
const val DEG_PER_SECOND = 200f
private const val DIP_ATTACK = 0.12f
private const val BOB_PX = 14f
private const val SWAY_DEG = 3.2f
private const val CUE_TILT_DEG = 5f
private const val SUBTLE_AMP = 0.3f

/** Without a beat grid the stage still bounces, at this tempo. */
const val FREE_BPM = 112.0
private const val SECONDS_PER_MINUTE = 60.0
private const val MS_PER_SECOND = 1000.0

/** The beat phase of a deck at [positionMs] (0 on the beat, up to 1), or null when its beat isn't known. */
fun beatPhase(deck: DjDeckUi, positionMs: Double): Double? {
    val grid = deck.analysis?.grid ?: return null
    if (grid.beatMs <= 0.0) return null
    val p = ((positionMs - grid.firstBeatMs) / grid.beatMs) % 1.0
    return if (p < 0) p + 1.0 else p
}

/** Which beat of the track a position is on (to alternate the sway). */
fun beatNumber(deck: DjDeckUi, positionMs: Double): Int {
    val grid = deck.analysis?.grid ?: return 0
    return if (grid.beatMs > 0.0) floor((positionMs - grid.firstBeatMs) / grid.beatMs).toInt() else 0
}

/** The body's drop on a beat: 0 at rest, 1 right after the beat, easing back by the next. */
fun beatDip(phase: Float): Float {
    if (phase < DIP_ATTACK) return phase / DIP_ATTACK
    val back = (phase - DIP_ATTACK) / (1f - DIP_ATTACK)
    return (1f - back) * (1f - back)
}

/** Free-running beat phase at [FREE_BPM]. */
fun freePhase(seconds: Double): Double = (seconds * (FREE_BPM / SECONDS_PER_MINUTE)) % 1.0

/** The deck the room hears most among the playing ones (the crossfader decides when both play). */
fun masterDeck(decks: List<DjDeckUi>, crossfader: Float): Int? {
    val on = decks.map { it.playing && it.loaded }
    return when {
        on[0] && on[1] -> if (crossfader < 0.5f) 0 else 1
        on[0] -> 0
        on[1] -> 1
        else -> null
    }
}

/** A loaded deck that is not playing while the other one does: the DJ is cueing it in his headphones. */
fun cueingDeck(decks: List<DjDeckUi>): Int? {
    val (a, b) = decks
    return when {
        a.playing && b.loaded && !b.playing && !b.loading -> 1
        b.playing && a.loaded && !a.playing && !a.loading -> 0
        else -> null
    }
}

// ------------------------------------------------------------------ reactions
/** What a sampler pad makes him do. */
enum class Reaction(val durationMs: Long) {
    HANDS_UP(1400), JUMP(800), SCRATCH(1000);

    companion object {
        fun of(sound: SfxKind): Reaction = when (sound) {
            SfxKind.AIR_HORN, SfxKind.SIREN, SfxKind.LASER, SfxKind.RISER -> HANDS_UP
            SfxKind.DROP, SfxKind.HIT -> JUMP
            SfxKind.SCRATCH, SfxKind.REWIND -> SCRATCH
        }
    }
}

private const val JUMP_PX = 40f
private const val JUMP_HANDS_PX = 40f
private const val WAVE_HZ = 1.6f
private const val WAVE_PX = 22f
private const val SCRATCH_HZ = 2.6f

/** A paused playhead that moves more than this between two frames is being dragged by a hand... */
private const val SCRATCH_MOVE_MS = 4L

/** ...unless it jumped this far, which is a seek (a hot cue, a tap on the overview). */
private const val SEEK_MS = 1500L

/** A deck is "scratched" when it is paused but its playhead moves a little between two frames. */
fun isScratching(deck: DjDeckUi, movedMs: Long): Boolean {
    val dragged = abs(movedMs) > SCRATCH_MOVE_MS && abs(movedMs) < SEEK_MS
    return deck.loaded && !deck.playing && dragged
}

// ------------------------------------------------------------------ what to aim at
/** The inputs of one frame, already read from the DJ state and the clock. */
class StageInput(
    val motion: StageMotion,
    /** Master beat phase 0..1 (free-running when the beat isn't known). */
    val phase: Float,
    val beatIndex: Int,
    val playing: BooleanArray,
    val scratching: BooleanArray,
    val cueing: Int?,
    val crossfader: Float,
    /** A hand is on the crossfader (it moved just now, or a mix runs), and which one. */
    val xfHand: Int?,
    val reaction: Reaction?,
    val reactionProgress: Float,
    val seconds: Double,
)

/** Where everything should be; the stage eases the drawn pose towards it. */
class Pose(
    val bodyY: Float = 0f,
    val tilt: Float = 0f,
    val handL: Offset = REST_L,
    val handR: Offset = REST_R,
    /** 0..1: how far a headphone cup is off the ear (0 on the ear). */
    val cupLift: Float = 0f,
    /** The cup that lifts: 0 = Homifito's left (as drawn), 1 = right. */
    val cupSide: Int? = null,
    val happy: Boolean = false,
)

val REST_POSE = Pose()

private fun lerp(a: Float, b: Float, t: Float) = a + (b - a) * t

fun crossfaderKnob(xf: Float) = Offset(lerp(XF_X1, XF_X2, xf.coerceIn(0f, 1f)), XF_Y - 4f)

private fun scratchHand(deck: Int, t: Double): Offset {
    val rest = if (deck == 0) REST_L else REST_R
    return Offset(rest.x + sin(t * PI * 2 * SCRATCH_HZ).toFloat() * SCRATCH_REACH, rest.y + SCRATCH_DROP)
}

/** What Homifito aims for: calm on SUBTLE (a gentle sway), still on OFF, the whole show on FULL. */
fun aim(i: StageInput): Pose {
    val full = i.motion == StageMotion.FULL
    val amp = when (i.motion) { StageMotion.FULL -> 1f; StageMotion.SUBTLE -> SUBTLE_AMP; StageMotion.OFF -> 0f }
    val lively = i.playing[0] || i.playing[1]
    val dip = if (lively) beatDip(i.phase) else 0f
    val side = if (i.beatIndex % 2 == 0) -1 else 1
    val cupSide = when (i.cueing) { 0 -> 1; 1 -> 0; else -> null }
    val cueSign = when (i.cueing) { 0 -> -1; 1 -> 1; else -> 0 }

    var bodyY = dip * BOB_PX * amp
    val tilt = (if (lively) side * SWAY_DEG * amp else 0f) + cueSign * CUE_TILT_DEG * (if (i.motion == StageMotion.OFF) 0f else 1f)
    var handL = Offset(REST_L.x, REST_L.y + dip * TAP_PX * amp)
    var handR = Offset(REST_R.x, REST_R.y + dip * TAP_PX * amp * (if (side > 0) 0.4f else 1f))
    var happy = false

    if (full) {
        if (i.scratching[0]) handL = scratchHand(0, i.seconds)
        if (i.scratching[1]) handR = scratchHand(1, i.seconds + 0.3)
    }
    // a hand on the crossfader follows the knob
    if (i.xfHand != null) {
        val knob = crossfaderKnob(i.crossfader)
        if (i.xfHand == 0) handL = Offset(knob.x - 4f, knob.y + 6f) else handR = Offset(knob.x + 4f, knob.y + 6f)
    }
    val reaction = i.reaction
    if (reaction != null) {
        happy = true
        if (full) {
            val wave = sin(i.seconds * PI * 2 * WAVE_HZ).toFloat() * WAVE_PX
            when (reaction) {
                Reaction.HANDS_UP -> {
                    handL = Offset(UP_L.x + wave, UP_L.y + abs(wave) * 0.4f)
                    handR = Offset(UP_R.x + wave, UP_R.y + abs(wave) * 0.4f)
                }
                Reaction.JUMP -> {
                    val arc = sin(PI * min(1f, i.reactionProgress)).toFloat()
                    bodyY -= JUMP_PX * arc
                    handL = Offset(JUMP_L.x, JUMP_L.y - JUMP_HANDS_PX * arc)
                    handR = Offset(JUMP_R.x, JUMP_R.y - JUMP_HANDS_PX * arc)
                }
                Reaction.SCRATCH -> {
                    handL = scratchHand(0, i.seconds)
                    handR = scratchHand(1, i.seconds + 0.25)
                }
            }
        }
    }
    return Pose(bodyY, tilt, handL, handR, if (i.cueing != null) 1f else 0f, cupSide, happy)
}

// ------------------------------------------------------------------ smoothing
/** How fast each part follows (per second): the body is snappy so the beat reads, hands a little looser. */
private const val RATE_BODY = 26f
private const val RATE_TILT = RATE_BODY * 0.4f
private const val RATE_HAND = 16f
private const val RATE_CUP = 9f

/** [current] moved towards [target] over [dt] seconds (frame-rate independent). */
fun damp(current: Float, target: Float, rate: Float, dt: Float): Float = target + (current - target) * exp(-rate * dt)

private fun step(a: Float, b: Float, rate: Float, dt: Float) = if (dt.isFinite()) damp(a, b, rate, dt) else b

private fun step(a: Offset, b: Offset, rate: Float, dt: Float) = Offset(step(a.x, b.x, rate, dt), step(a.y, b.y, rate, dt))

/** One step of [p] towards [t]; a [dt] of infinity jumps there (the still pose of OFF). */
fun approach(p: Pose, t: Pose, dt: Float) = Pose(
    bodyY = step(p.bodyY, t.bodyY, RATE_BODY, dt),
    tilt = step(p.tilt, t.tilt, RATE_TILT, dt),
    handL = step(p.handL, t.handL, RATE_HAND, dt),
    handR = step(p.handR, t.handR, RATE_HAND, dt),
    cupLift = step(p.cupLift, t.cupLift, RATE_CUP, dt),
    cupSide = t.cupSide ?: p.cupSide,
    happy = t.happy,
)

/** The shoulder as it sits after the body's drop and sway (the arms start from there). */
fun shoulderAt(base: Offset, pose: Pose): Offset {
    val a = pose.tilt * PI.toFloat() / 180f
    val dx = base.x - PIVOT.x
    val dy = base.y - PIVOT.y
    return Offset(PIVOT.x + dx * cos(a) - dy * sin(a), PIVOT.y + dx * sin(a) + dy * cos(a) + pose.bodyY)
}

private const val SAG_BASE = 26f
private const val SAG_MAX_EXTRA = 40f
private const val SAG_PER_PX = 0.12f

/** The elbow of an arm from [from] to [to]: a soft curve whose elbow hangs down. */
fun armElbow(from: Offset, to: Offset): Offset {
    val sag = SAG_BASE + min(SAG_MAX_EXTRA, hypot(to.x - from.x, to.y - from.y) * SAG_PER_PX)
    return Offset((from.x + to.x) / 2, maxOf(from.y, to.y) + sag * 0.6f - abs(to.y - from.y) * 0.1f)
}

/** Where a spot on a platter's label is for a record turned [degrees] (the platter is squashed by the view). */
fun platterDot(deck: Int, degrees: Float, radius: Float): Offset {
    val a = degrees * PI.toFloat() / 180f
    val c = PLATTER[deck]
    return Offset(c.x + cos(a) * radius, c.y + sin(a) * radius * PLATTER_SQUASH)
}

/** How far the record has turned at [positionMs] (the real playhead: a scratch turns it back). */
fun recordDegrees(positionMs: Double): Float = (positionMs / MS_PER_SECOND * DEG_PER_SECOND).toFloat()

// ------------------------------------------------------------------ fitting the screen
/** Below this screen height (dp) the strip is hidden: a phone on its side keeps its room for the controls. */
const val HIDE_HEIGHT_DP = 480

/** Under this height "Big" shrinks to the booth. */
const val TIGHT_HEIGHT_DP = 700

private const val BOOTH_SHARE = 0.13f
private const val BIG_SHARE = 0.26f
private const val BOOTH_MIN_DP = 96
private const val BOOTH_MAX_DP = 140
private const val BIG_MIN_DP = 150
private const val BIG_MAX_DP = 260

/** The strip's height in dp for a screen [screenHeightDp] tall, or null when it doesn't fit and is hidden. */
fun homifitoStripHeightDp(screenHeightDp: Int, big: Boolean): Int? = when {
    screenHeightDp < HIDE_HEIGHT_DP -> null
    big && screenHeightDp >= TIGHT_HEIGHT_DP -> (screenHeightDp * BIG_SHARE).toInt().coerceIn(BIG_MIN_DP, BIG_MAX_DP)
    else -> (screenHeightDp * BOOTH_SHARE).toInt().coerceIn(BOOTH_MIN_DP, BOOTH_MAX_DP)
}
