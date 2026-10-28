package com.perseusoft.homefi.ui.player

import com.perseusoft.homefi.R
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.ceil
import kotlin.math.cos
import kotlin.math.floor
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.sin
import kotlin.random.Random

// The mascot's choreography: a pose for every moment of the beat. Each genre
// mood has its own groove, and every few bars it throws in a famous dance.
// Port of the desktop's `lib/dance.ts` (same math, same numbers): keep both in sync.

enum class DanceId(val label: Int, val hint: Int) {
    JAGGER(R.string.dance_jagger, R.string.dance_jagger_hint),
    MOONWALK(R.string.dance_moonwalk, R.string.dance_moonwalk_hint),
    TWIST(R.string.dance_twist, R.string.dance_twist_hint),
    ROBOT(R.string.dance_robot, R.string.dance_robot_hint),
    DISCO(R.string.dance_disco, R.string.dance_disco_hint),
    YMCA(R.string.dance_ymca, R.string.dance_ymca_hint),
    FLOSS(R.string.dance_floss, R.string.dance_floss_hint),
    MACARENA(R.string.dance_macarena, R.string.dance_macarena_hint),
    RUNNING(R.string.dance_running, R.string.dance_running_hint),
    HORSE(R.string.dance_horse, R.string.dance_horse_hint),
    GUITAR(R.string.dance_guitar, R.string.dance_guitar_hint),
}

/** The famous dances each genre picks from. */
val SPECIALS: Map<Mood, List<DanceId>> = mapOf(
    Mood.BOP to listOf(DanceId.JAGGER, DanceId.TWIST, DanceId.MOONWALK, DanceId.DISCO),
    Mood.MOSH to listOf(DanceId.GUITAR, DanceId.RUNNING),
    Mood.POGO to listOf(DanceId.RUNNING, DanceId.TWIST, DanceId.GUITAR),
    Mood.CUTE to listOf(DanceId.FLOSS, DanceId.MACARENA, DanceId.YMCA),
    Mood.PARTY to listOf(DanceId.ROBOT, DanceId.DISCO, DanceId.YMCA, DanceId.FLOSS),
    Mood.SALSA to listOf(DanceId.MACARENA, DanceId.TWIST, DanceId.JAGGER),
    Mood.GROOVE to listOf(DanceId.RUNNING, DanceId.MOONWALK, DanceId.HORSE, DanceId.ROBOT),
    Mood.CHILL to listOf(DanceId.JAGGER, DanceId.MOONWALK),
)

/** How often the famous dances come up. */
enum class Specials { OFF, SOMETIMES, OFTEN }

/** When it may turn sideways: never, only in the famous dances, or also in the grooves. */
enum class Turns { OFF, DANCES, ALL }

/**
 * Where everything is, in the mascot's own units (the art is 1000 wide).
 * Limb angles are how far the limb is raised away from the body: 0 hangs
 * straight down, 90 points sideways, 180 straight up, below 0 crosses in front.
 */
data class Pose(
    val x: Double = 0.0,
    val y: Double = 0.0,
    /** Body tilt in degrees, around the feet. */
    val rot: Double = 0.0,
    val sx: Double = 1.0,
    val sy: Double = 1.0,
    val armL: Double = 30.0,
    val armR: Double = 30.0,
    val legL: Double = 0.0,
    val legR: Double = 0.0,
    /** How high each foot is lifted. */
    val liftL: Double = 0.0,
    val liftR: Double = 0.0,
    /** Facing the other way (for turns when it may not turn sideways). */
    val flip: Boolean = false,
    /**
     * How far it has turned to show its side: 0 faces you, 1 is a profile
     * facing right, -1 facing left. In profile the limb angles swing forward
     * (positive) and back (negative) instead of out to the sides.
     */
    val turn: Double = 0.0,
    /**
     * Holding a guitar (0 none, 1 playing it): the arms then reach for it
     * instead of following [armL] / [armR].
     */
    val guitar: Double = 0.0,
    /** Where the picking hand is in its stroke across the strings: 1 down, -1 up. */
    val strum: Double = 0.0,
    /** Where the fretting hand is along the neck: 0 near the body, 1 further up. */
    val fret: Double = 0.0,
)

val REST = Pose()

private fun pos(v: Double) = max(0.0, v)
private fun frac(v: Double) = v - floor(v)

/** 1 on the beat, 0 half way between beats. */
private fun onBeat(b: Double) = 0.5 + 0.5 * cos(2 * PI * b)

/** A snappy move into a new position at the start of each beat. */
private fun snap(b: Double, speed: Double = 5.0) = min(1.0, frac(b) * speed)
private fun ease(t: Double) = t * t * (3 - 2 * t)
private fun mix(a: Double, b: Double, t: Double) = a + (b - a) * t

/** JS-style modulo for beat indexes (always positive). */
private fun idx(b: Double, n: Int) = floor(b).toInt().mod(n)

/** Steps through a table of [left arm, right arm], one row per beat. */
private fun arms(table: Array<DoubleArray>, b: Double, speed: Double = 4.0): Pair<Double, Double> {
    val i = idx(b, table.size)
    val prev = table[(i + table.size - 1) % table.size]
    val t = ease(snap(b, speed))
    return mix(prev[0], table[i][0], t) to mix(prev[1], table[i][1], t)
}

private fun row(l: Double, r: Double) = doubleArrayOf(l, r)

/** Steps through one value per beat, moving snappily into each. */
private fun steps(table: DoubleArray, b: Double, speed: Double = 4.0): Double {
    val i = idx(b, table.size)
    val prev = table[(i + table.size - 1) % table.size]
    return mix(prev, table[i], ease(snap(b, speed)))
}

private val GUITAR_SLIDE = doubleArrayOf(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0)

private val ROBOT_TURNS = doubleArrayOf(0.0, 0.0, 1.0, 1.0, 0.0, 0.0, -1.0, -1.0)

private val ROBOT = arrayOf(row(90.0, 10.0), row(90.0, 90.0), row(170.0, 90.0), row(10.0, 170.0))
private val YMCA = arrayOf(
    row(150.0, 150.0), row(195.0, 195.0), row(160.0, 70.0), row(175.0, 175.0),
    row(170.0, 170.0), row(178.0, 178.0), row(170.0, 170.0), row(178.0, 178.0),
)
private val MACARENA = arrayOf(
    row(35.0, 90.0), row(90.0, 90.0), row(90.0, -30.0), row(-30.0, -30.0),
    row(-30.0, 175.0), row(175.0, 175.0), row(175.0, 20.0), row(20.0, 20.0),
)

/**
 * The everyday groove for a mood. `b` counts beats, `k` (0.5..1.3) is how
 * loud it is; `side` lets it turn sideways.
 */
fun moodPose(mood: Mood, b: Double, k: Double, side: Boolean = false): Pose {
    val p = groove(mood, b, k)
    if (!side) return p
    val s = sin(PI * b)
    val slow = sin((PI * b) / 4) // one look each way every eight beats
    val turn = when (mood) {
        Mood.MOSH, Mood.POGO -> 0.0
        Mood.CUTE -> 0.25 * sin((2 * PI * b) / 3)
        Mood.PARTY -> 0.3 * s
        Mood.SALSA -> 0.45 * s
        Mood.GROOVE -> 0.35 * slow
        Mood.CHILL -> 0.3 * sin((PI * b) / 2)
        Mood.BOP -> 0.2 * slow
    }
    return p.copy(turn = turn)
}

private fun groove(mood: Mood, b: Double, k: Double): Pose {
    val s = sin(PI * b) // one swing every two beats
    val d = onBeat(b)
    return when (mood) {
        // headbanging with the horns up
        Mood.MOSH -> Pose(y = 10 * k * d, rot = k * (-6 + 24 * d), armL = 150 + 12 * k * d, armR = 150 + 12 * k * d, legL = 16.0, legR = 16.0)
        Mood.POGO -> {
            val j = sin(PI * frac(b)) // in the air between beats
            val land = (1 - j).pow(4)
            Pose(y = -70 * k * j, sx = 1 + 0.06 * land, sy = 1 - 0.08 * land, armL = 140 + 30 * sin(4 * PI * b), armR = 140 - 30 * sin(4 * PI * b), legL = -4.0, legR = -4.0)
        }
        Mood.CUTE -> {
            val w = sin((2 * PI * b) / 3)
            Pose(y = -14 * k * pos(w), rot = 7 * k * w, armL = 125 + 30 * sin(2 * PI * b), armR = 125 + 30 * sin(2 * PI * b + PI), liftL = 10 * pos(w), liftR = 10 * pos(-w))
        }
        // fist pump on every beat
        Mood.PARTY -> Pose(x = 22 * k * s, y = -8 * k * d, rot = 10 * k * s, armL = 40 + 10 * s, armR = 95 + 75 * k * d, liftL = 12 * pos(s), liftR = 12 * pos(-s))
        // the basic step: hips one way, a step the other
        Mood.SALSA -> Pose(x = -22 * k * s, y = -6 * pos(-cos(2 * PI * b)), rot = 9 * k * s, armL = 80 + 18 * s, armR = 80 - 18 * s, legL = 6 * s, legR = -6 * s, liftL = 16 * pos(s), liftR = 16 * pos(-s))
        // knees bounce down on the beat
        Mood.GROOVE -> Pose(y = 12 * k * d, rot = k * (-3 + 9 * d), armL = 20 + 28 * k * s, armR = 20 - 28 * k * s, legL = 8 * d, legR = 8 * d)
        Mood.CHILL -> {
            val w = sin((PI * b) / 2)
            Pose(x = 6 * w, rot = 5 * k * w, armL = 22 + 8 * w, armR = 22 - 8 * w, liftR = 8 * d)
        }
        // bop: a nod on the beat, a little lean and a foot tap
        Mood.BOP -> Pose(y = 14 * k * d, rot = 4 * k * s, armL = 35 + 20 * k * s, armR = 35 - 20 * k * s, liftL = 14 * k * pos(s), liftR = 14 * k * pos(-s))
    }
}

/**
 * A famous dance. `b` counts beats from the start of the dance; with `side`
 * the dances that look better from the side turn to show it.
 */
fun dancePose(id: DanceId, b: Double, k: Double, side: Boolean = false): Pose {
    val s = sin(PI * b)
    val d = onBeat(b)
    return when (id) {
        DanceId.JAGGER -> {
            // the strut: hips across the floor, flapping elbows, a point up every bar
            val point = ease(min(1.0, pos(frac(b / 4) * 4 - 3) * 3))
            // turned three quarters towards where the strut is going
            Pose(x = 40 * k * sin((PI * b) / 2), y = -6 * d, rot = -6 + 8 * s, armL = 60 + 80 * pos(s), armR = mix(45 + 30 * pos(-s), 165.0, point), legL = 14 * pos(s), legR = 14 * pos(-s), liftL = 24 * pos(s), liftR = 24 * pos(-s), turn = if (side) 0.6 * cos((PI * b) / 2) else 0.0)
        }
        DanceId.MOONWALK -> {
            // glide one way for four beats, turn, glide back; one heel up at a time
            val leg = idx(b / 4, 2) == 1
            val t = frac(b / 4)
            val heel = idx(b, 2) == 0
            if (side) {
                // in profile, facing away from the glide, spinning round at each end
                val dir = if (leg) -1.0 else 1.0
                val turn = mix(-dir, dir, ease(min(1.0, (t * 4) / 0.6)))
                return Pose(x = (if (leg) -1 else 1) * (70 - 140 * t), rot = -3.0, armL = 18 + 14 * s, armR = 18 - 14 * s, legL = if (heel) 12.0 else -16.0, legR = if (heel) -16.0 else 12.0, liftL = if (heel) 20.0 else 0.0, liftR = if (heel) 0.0 else 20.0, turn = turn)
            }
            Pose(x = (if (leg) -1 else 1) * (70 - 140 * t), rot = -4.0, armL = 28 + 8 * s, armR = 28 - 8 * s, legL = if (heel) -8.0 else 4.0, legR = if (heel) 4.0 else -8.0, liftL = if (heel) 18.0 else 0.0, liftR = if (heel) 0.0 else 18.0, flip = leg)
        }
        DanceId.TWIST -> {
            val w = sin(2 * PI * b)
            Pose(y = 14 + 8 * abs(w), rot = 14 * k * w, sx = 1 + 0.03 * abs(w), armL = 60 + 32 * w, armR = 60 - 32 * w, legL = 12 * w, legR = 12 * w, turn = if (side) 0.35 * w else 0.0)
        }
        DanceId.ROBOT -> {
            val (l, r) = arms(ROBOT, b, 8.0)
            val i = idx(b, 4)
            // a stiff quarter turn every couple of beats
            val turn = if (side) steps(ROBOT_TURNS, b, 8.0) else 0.0
            Pose(x = doubleArrayOf(0.0, 10.0, 0.0, -10.0)[i], rot = doubleArrayOf(0.0, -6.0, 0.0, 6.0)[i], armL = l, armR = r, legL = 2.0, legR = 2.0, turn = turn)
        }
        DanceId.DISCO -> {
            // the famous point: up to the sky, down across, hand on the hip
            val up = idx(b, 2) == 0
            val t = ease(snap(b, 4.0))
            val turn = if (!side) 0.0 else if (up) mix(-0.15, 0.45, t) else mix(0.45, -0.15, t)
            Pose(x = 14 * s, rot = 6 * s, armL = 28.0, armR = if (up) mix(-20.0, 165.0, t) else mix(165.0, -20.0, t), liftL = if (up) 0.0 else 14.0, legL = if (up) 0.0 else 10.0, turn = turn)
        }
        DanceId.YMCA -> {
            // Y, M, C, A, then four beats of clapping overhead
            val (l, r) = arms(YMCA, b, 5.0)
            Pose(y = if (idx(b, 8) >= 4) -10 * d else 0.0, armL = l, armR = r)
        }
        DanceId.FLOSS -> {
            val w = sin(2 * PI * b)
            Pose(x = -14 * k * w, rot = -4 * w, armL = 30 + 45 * w, armR = 30 - 45 * w, turn = if (side) 0.2 * w else 0.0)
        }
        DanceId.MACARENA -> {
            // arms out, palms, cross, head, hips... then a hip wiggle and a quarter turn
            val (l, r) = arms(MACARENA, b, 5.0)
            val wiggle = if (idx(b, 8) == 7) sin(4 * PI * b) else 0.0
            val turned = idx(b / 8, 2) == 1
            if (side) {
                // the quarter turn at the end of each round, shown as a real turn
                val into = ease(min(1.0, frac(b / 8) * 16))
                val turn = if (turned) into else if (b < 8) 0.0 else 1 - into
                return Pose(x = 18 * wiggle, y = -4 * d, rot = 6 * wiggle, armL = l, armR = r, turn = turn)
            }
            Pose(x = 18 * wiggle, y = -4 * d, rot = 6 * wiggle, armL = l, armR = r, flip = turned)
        }
        // in profile: knee up in front, the other foot sliding back, arms pumping
        DanceId.RUNNING -> if (side) {
            Pose(y = -8 * d, rot = 7 + 2 * s, armL = 75 * s, armR = -75 * s, legL = 42 * s, legR = -42 * s, liftL = 34 * pos(s), liftR = 34 * pos(-s), turn = 1.0)
        } else Pose(y = -8 * d, rot = 3 * s, armL = 40 + 34 * s, armR = 40 - 34 * s, legL = -14 * pos(s), legR = -14 * pos(-s), liftL = 32 * pos(s), liftR = 32 * pos(-s))
        DanceId.HORSE -> {
            // holding the reins, galloping; every other bar the lasso goes round
            val lasso = idx(b / 4, 2) == 1
            val hop = abs(s)
            if (side) {
                // riding sideways: reins held out in front, legs galloping
                return Pose(y = -26 * k * hop, rot = 5 + 3 * s, armL = 70 + 6 * d, armR = if (lasso) 165 + 15 * sin(4 * PI * b) else 70 + 6 * d, legL = 24 * s, legR = 24 * sin(PI * b + 1), liftL = 14 * pos(s), liftR = 14 * pos(-s), turn = 1.0)
            }
            Pose(y = -26 * k * hop, rot = 3 * s, armL = -25 + 6 * d, armR = if (lasso) 165 + 15 * sin(4 * PI * b) else -25 + 6 * d, liftL = 14 * pos(s), liftR = 14 * pos(-s))
        }
        DanceId.GUITAR -> {
            // a real guitar held across the belly: the picking hand strokes across the
            // strings (down on the beat, up in between), the fretting hand slides up
            // the neck every other bar, the body rocks back on the beat and there is
            // a jump at the end of every four bars. Never further round than three
            // quarters, so the guitar always stays in front.
            val jump = if (idx(b, 16) == 15) sin(PI * frac(b)) else 0.0
            val loud = if (idx(b / 2, 2) == 1) 2 else 1 // every other bar strums twice as fast
            Pose(
                y = 3 * d - 40 * k * jump,
                rot = 1 - 5 * k * d,
                armL = -60.0,
                armR = 150.0,
                legL = 16.0,
                legR = 16.0,
                turn = if (side) 0.36 + 0.08 * sin(PI * b / 4) else 0.0,
                guitar = 1.0,
                strum = cos(2 * PI * b * loud),
                fret = steps(GUITAR_SLIDE, b, 3.0),
            )
        }
    }
}

private fun blend(a: Pose, b: Pose, t: Double) = Pose(
    x = mix(a.x, b.x, t),
    y = mix(a.y, b.y, t),
    rot = mix(a.rot, b.rot, t),
    sx = mix(a.sx, b.sx, t),
    sy = mix(a.sy, b.sy, t),
    armL = mix(a.armL, b.armL, t),
    armR = mix(a.armR, b.armR, t),
    legL = mix(a.legL, b.legL, t),
    legR = mix(a.legR, b.legR, t),
    liftL = mix(a.liftL, b.liftL, t),
    liftR = mix(a.liftR, b.liftR, t),
    flip = if (t < 0.5) a.flip else b.flip,
    turn = mix(a.turn, b.turn, t),
    guitar = mix(a.guitar, b.guitar, t),
    strum = mix(a.strum, b.strum, t),
    fret = mix(a.fret, b.fret, t),
)

private const val DANCE_BEATS = 16

/**
 * Keeps the beat (tempo changes don't make it jump), decides when a famous
 * dance starts and blends smoothly from one move into the next. [now] is a
 * clock in milliseconds (the breathing while asleep follows it).
 */
class Choreographer(private val now: () -> Double = { System.nanoTime() / 1e6 }) {
    var beat = 0.0
        private set

    /** The famous dance going on, if any. */
    var dance: DanceId? = null
        private set
    private var danceStart = 0.0
    private var nextAt = 8.0
    private var from = REST
    private var fromAt = -10.0
    private var last = REST
    private var wave = 0.0

    /** Starts a famous dance now (or a random one that suits the mood). */
    fun trigger(mood: Mood, id: DanceId? = null) {
        val pool = SPECIALS.getValue(mood)
        start(id ?: pool[Random.nextInt(pool.size)])
    }

    /** Waves hello for a couple of seconds (when poked while paused). */
    fun greet() {
        wave = 2.2
    }

    private fun start(id: DanceId?) {
        from = last
        fromAt = beat
        dance = id
        danceStart = ceil(beat)
    }

    /**
     * Advances by `dt` seconds and returns the pose to draw. `pinned` keeps one
     * famous dance going all the time.
     */
    fun step(dt: Double, playing: Boolean, bpm: Double, energy: Double, mood: Mood, specials: Specials, pinned: DanceId?, turns: Turns = Turns.ALL): Pose {
        val k = if (playing) 0.5 + min(1.0, energy * 1.6) * 0.8 else 0.0
        wave = max(0.0, wave - dt)
        if (!playing) {
            // asleep: arms down, breathing; or waving when poked
            val breathe = sin(now() / 900)
            val p = Pose(y = 4 + 3 * breathe, sy = 1 + 0.012 * breathe, armL = 12.0, armR = if (wave > 0) 150 + 25 * sin(wave * 14) else 12.0)
            last = settle(p, dt)
            return last
        }
        beat += (dt * bpm) / 60

        if (pinned != null && dance != pinned) start(pinned)
        if (pinned == null) {
            if (dance != null && beat - danceStart >= DANCE_BEATS) {
                start(null)
                nextAt = beat + (if (specials == Specials.OFTEN) 16 else 32) + Random.nextInt(8)
            } else if (dance == null && specials != Specials.OFF && beat >= nextAt) {
                trigger(mood)
            }
        }

        val b = beat - danceStart
        val current = dance
        var p = if (current != null && b >= 0) dancePose(current, b, k, turns != Turns.OFF) else moodPose(mood, beat, k, turns == Turns.ALL)
        // half a beat to blend into a new move
        val t = min(1.0, (beat - fromAt) / 0.5)
        if (t < 1) p = blend(from, p, ease(t))
        if (wave > 0) p = p.copy(armR = 150 + 25 * sin(wave * 14))
        last = p
        return p
    }

    /** Smooths changes when nothing drives the beat (e.g. falling asleep). */
    private fun settle(target: Pose, dt: Double): Pose = blend(last, target, min(1.0, dt * 6))
}

// Tapped too many times, too fast: it stomps, shakes its fists and fumes.

/** Quick taps (within [TEMPER_WINDOW] seconds) that make it angry; OFF never. */
enum class Temper(val taps: Int) { OFF(0), PATIENT(10), NORMAL(6), SHORT(4) }

const val TEMPER_WINDOW = 3.0

/** How long a tantrum lasts, in seconds (more taps while angry make it longer). */
const val ANGER_TIME = 3.0

/** The tantrum, [t] seconds in: trembling, stomping in turns, fists shaking. */
fun angryPose(t: Double): Pose {
    val shake = sin(2 * PI * 9 * t)
    val fists = sin(2 * PI * 4 * t)
    val stomp = abs(sin(2 * PI * 2 * t))
    val left = floor(t * 4).toLong() % 2 == 0L
    return Pose(
        x = 9 * shake,
        rot = 3 * shake,
        sx = 1.03,
        sy = 0.97,
        armL = 118 + 26 * fists,
        armR = 118 - 26 * fists,
        liftL = if (left) 26 * stomp else 0.0,
        liftR = if (left) 0.0 else 26 * stomp,
    )
}
