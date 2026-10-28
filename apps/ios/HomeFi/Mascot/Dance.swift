import Foundation
import QuartzCore

// The mascot's choreography: a pose for every moment of the beat. Each genre
// mood has its own groove, and every few bars it throws in a famous dance.
// Port of the Android `ui/player/Dance.kt` and the desktop's `lib/dance.ts`
// (same math, same numbers): keep all three in sync.

enum DanceId: String, CaseIterable, Identifiable {
    case jagger = "JAGGER"
    case moonwalk = "MOONWALK"
    case twist = "TWIST"
    case robot = "ROBOT"
    case disco = "DISCO"
    case ymca = "YMCA"
    case floss = "FLOSS"
    case macarena = "MACARENA"
    case running = "RUNNING"
    case horse = "HORSE"
    case guitar = "GUITAR"

    var id: String { rawValue }

    var label: String {
        switch self {
        case .jagger: String(localized: "dance.jagger", defaultValue: "Like Jagger")
        case .moonwalk: String(localized: "dance.moonwalk", defaultValue: "Moonwalk")
        case .twist: String(localized: "dance.twist", defaultValue: "The Twist")
        case .robot: String(localized: "dance.robot", defaultValue: "Robot")
        case .disco: String(localized: "dance.disco", defaultValue: "Disco fever")
        case .ymca: String(localized: "dance.ymca", defaultValue: "YMCA")
        case .floss: String(localized: "dance.floss", defaultValue: "Floss")
        case .macarena: String(localized: "dance.macarena", defaultValue: "Macarena")
        case .running: String(localized: "dance.running", defaultValue: "Running man")
        case .horse: String(localized: "dance.horse", defaultValue: "Horse ride")
        case .guitar: String(localized: "dance.guitar", defaultValue: "Guitar solo")
        }
    }

    var hint: String {
        switch self {
        case .jagger: String(localized: "dance.jagger.hint", defaultValue: "Rooster strut, hips and a point to the sky")
        case .moonwalk: String(localized: "dance.moonwalk.hint", defaultValue: "Gliding backwards")
        case .twist: String(localized: "dance.twist.hint", defaultValue: "Hips twisting low")
        case .robot: String(localized: "dance.robot.hint", defaultValue: "Stiff moves, one per beat")
        case .disco: String(localized: "dance.disco.hint", defaultValue: "Point up, point down")
        case .ymca: String(localized: "dance.ymca.hint", defaultValue: "Spelling it with the arms")
        case .floss: String(localized: "dance.floss.hint", defaultValue: "Arms swinging past the hips")
        case .macarena: String(localized: "dance.macarena.hint", defaultValue: "Arms, head, hips, turn")
        case .running: String(localized: "dance.running.hint", defaultValue: "Running on the spot")
        case .horse: String(localized: "dance.horse.hint", defaultValue: "Galloping with the reins")
        case .guitar: String(localized: "dance.guitar.hint", defaultValue: "Rocking out on a guitar")
        }
    }
}

/// The famous dances each genre picks from.
let SPECIALS: [Mood: [DanceId]] = [
    .bop: [.jagger, .twist, .moonwalk, .disco],
    .mosh: [.guitar, .running],
    .pogo: [.running, .twist, .guitar],
    .cute: [.floss, .macarena, .ymca],
    .party: [.robot, .disco, .ymca, .floss],
    .salsa: [.macarena, .twist, .jagger],
    .groove: [.running, .moonwalk, .horse, .robot],
    .chill: [.jagger, .moonwalk],
]

/// How often the famous dances come up.
enum Specials: String, CaseIterable, Identifiable {
    case off = "OFF"
    case sometimes = "SOMETIMES"
    case often = "OFTEN"
    var id: String { rawValue }

    var label: String {
        switch self {
        case .off: String(localized: "specials.off", defaultValue: "Off")
        case .sometimes: String(localized: "specials.sometimes", defaultValue: "Sometimes")
        case .often: String(localized: "specials.often", defaultValue: "Often")
        }
    }
}

/// When it may turn sideways: never, only in the famous dances, or also in the grooves.
enum Turns: String, CaseIterable, Identifiable {
    case off = "OFF"
    case dances = "DANCES"
    case all = "ALL"
    var id: String { rawValue }

    var label: String {
        switch self {
        case .off: String(localized: "turns.off", defaultValue: "Always facing you")
        case .dances: String(localized: "turns.dances", defaultValue: "In famous dances")
        case .all: String(localized: "turns.all", defaultValue: "Always")
        }
    }
}

/// Colour of the mascot's arms and legs; OFF draws none.
enum MascotLimbs: String, CaseIterable, Identifiable {
    case white = "WHITE"
    case navy = "NAVY"
    case accent = "ACCENT"
    case off = "OFF"
    var id: String { rawValue }

    var label: String {
        switch self {
        case .white: String(localized: "limbs.white", defaultValue: "White")
        case .navy: String(localized: "limbs.navy", defaultValue: "Navy")
        case .accent: String(localized: "limbs.accent", defaultValue: "Accent")
        case .off: String(localized: "limbs.off", defaultValue: "None")
        }
    }
}

/**
 * Where everything is, in the mascot's own units (the art is 1000 wide).
 * Limb angles are how far the limb is raised away from the body: 0 hangs
 * straight down, 90 points sideways, 180 straight up, below 0 crosses in front.
 */
struct Pose {
    var x: Double = 0
    var y: Double = 0
    /// Body tilt in degrees, around the feet.
    var rot: Double = 0
    var sx: Double = 1
    var sy: Double = 1
    var armL: Double = 30
    var armR: Double = 30
    var legL: Double = 0
    var legR: Double = 0
    /// How high each foot is lifted.
    var liftL: Double = 0
    var liftR: Double = 0
    /// Facing the other way (for turns when it may not turn sideways).
    var flip: Bool = false
    /**
     * How far it has turned to show its side: 0 faces you, 1 is a profile
     * facing right, -1 facing left. In profile the limb angles swing forward
     * (positive) and back (negative) instead of out to the sides.
     */
    var turn: Double = 0
    /**
     * Holding a guitar (0 none, 1 playing it): the arms then reach for it
     * instead of following `armL` / `armR`.
     */
    var guitar: Double = 0
    /// Where the picking hand is in its stroke across the strings: 1 down, -1 up.
    var strum: Double = 0
    /// Where the fretting hand is along the neck: 0 near the body, 1 further up.
    var fret: Double = 0
}

let REST = Pose()

private func pos(_ v: Double) -> Double { max(0, v) }
private func frac(_ v: Double) -> Double { v - floor(v) }

/// 1 on the beat, 0 half way between beats.
private func onBeat(_ b: Double) -> Double { 0.5 + 0.5 * cos(2 * .pi * b) }

/// A snappy move into a new position at the start of each beat.
private func snap(_ b: Double, _ speed: Double = 5) -> Double { min(1, frac(b) * speed) }
private func ease(_ t: Double) -> Double { t * t * (3 - 2 * t) }
private func mix(_ a: Double, _ b: Double, _ t: Double) -> Double { a + (b - a) * t }

/// JS-style modulo for beat indexes (always positive).
private func idx(_ b: Double, _ n: Int) -> Int {
    let i = Int(floor(b)) % n
    return i < 0 ? i + n : i
}

/// Steps through a table of [left arm, right arm], one row per beat.
private func arms(_ table: [(Double, Double)], _ b: Double, _ speed: Double = 4) -> (Double, Double) {
    let i = idx(b, table.count)
    let prev = table[(i + table.count - 1) % table.count]
    let t = ease(snap(b, speed))
    return (mix(prev.0, table[i].0, t), mix(prev.1, table[i].1, t))
}

/// Steps through one value per beat, moving snappily into each.
private func steps(_ table: [Double], _ b: Double, _ speed: Double = 4) -> Double {
    let i = idx(b, table.count)
    let prev = table[(i + table.count - 1) % table.count]
    return mix(prev, table[i], ease(snap(b, speed)))
}

private let GUITAR_SLIDE: [Double] = [0, 0, 0, 0, 0, 0, 1, 1]
private let ROBOT_TURNS: [Double] = [0, 0, 1, 1, 0, 0, -1, -1]
private let ROBOT: [(Double, Double)] = [(90, 10), (90, 90), (170, 90), (10, 170)]
private let YMCA: [(Double, Double)] = [
    (150, 150), (195, 195), (160, 70), (175, 175),
    (170, 170), (178, 178), (170, 170), (178, 178),
]
private let MACARENA: [(Double, Double)] = [
    (35, 90), (90, 90), (90, -30), (-30, -30),
    (-30, 175), (175, 175), (175, 20), (20, 20),
]

/**
 * The everyday groove for a mood. `b` counts beats, `k` (0.5..1.3) is how
 * loud it is; `side` lets it turn sideways.
 */
func moodPose(_ mood: Mood, _ b: Double, _ k: Double, side: Bool = false) -> Pose {
    var p = groove(mood, b, k)
    if !side { return p }
    let s = sin(.pi * b)
    let slow = sin((.pi * b) / 4) // one look each way every eight beats
    switch mood {
    case .mosh, .pogo: p.turn = 0
    case .cute: p.turn = 0.25 * sin((2 * .pi * b) / 3)
    case .party: p.turn = 0.3 * s
    case .salsa: p.turn = 0.45 * s
    case .groove: p.turn = 0.35 * slow
    case .chill: p.turn = 0.3 * sin((.pi * b) / 2)
    case .bop: p.turn = 0.2 * slow
    }
    return p
}

private func groove(_ mood: Mood, _ b: Double, _ k: Double) -> Pose {
    let s = sin(.pi * b) // one swing every two beats
    let d = onBeat(b)
    switch mood {
    case .mosh:
        // headbanging with the horns up
        return Pose(y: 10 * k * d, rot: k * (-6 + 24 * d), armL: 150 + 12 * k * d, armR: 150 + 12 * k * d, legL: 16, legR: 16)
    case .pogo:
        let j = sin(.pi * frac(b)) // in the air between beats
        let land = pow(1 - j, 4)
        return Pose(y: -70 * k * j, sx: 1 + 0.06 * land, sy: 1 - 0.08 * land, armL: 140 + 30 * sin(4 * .pi * b), armR: 140 - 30 * sin(4 * .pi * b), legL: -4, legR: -4)
    case .cute:
        let w = sin((2 * .pi * b) / 3)
        return Pose(y: -14 * k * pos(w), rot: 7 * k * w, armL: 125 + 30 * sin(2 * .pi * b), armR: 125 + 30 * sin(2 * .pi * b + .pi), liftL: 10 * pos(w), liftR: 10 * pos(-w))
    case .party:
        // fist pump on every beat
        return Pose(x: 22 * k * s, y: -8 * k * d, rot: 10 * k * s, armL: 40 + 10 * s, armR: 95 + 75 * k * d, liftL: 12 * pos(s), liftR: 12 * pos(-s))
    case .salsa:
        // the basic step: hips one way, a step the other
        return Pose(x: -22 * k * s, y: -6 * pos(-cos(2 * .pi * b)), rot: 9 * k * s, armL: 80 + 18 * s, armR: 80 - 18 * s, legL: 6 * s, legR: -6 * s, liftL: 16 * pos(s), liftR: 16 * pos(-s))
    case .groove:
        // knees bounce down on the beat
        return Pose(y: 12 * k * d, rot: k * (-3 + 9 * d), armL: 20 + 28 * k * s, armR: 20 - 28 * k * s, legL: 8 * d, legR: 8 * d)
    case .chill:
        let w = sin((.pi * b) / 2)
        return Pose(x: 6 * w, rot: 5 * k * w, armL: 22 + 8 * w, armR: 22 - 8 * w, liftR: 8 * d)
    case .bop:
        // a nod on the beat, a little lean and a foot tap
        return Pose(y: 14 * k * d, rot: 4 * k * s, armL: 35 + 20 * k * s, armR: 35 - 20 * k * s, liftL: 14 * k * pos(s), liftR: 14 * k * pos(-s))
    }
}

/**
 * A famous dance. `b` counts beats from the start of the dance; with `side`
 * the dances that look better from the side turn to show it.
 */
func dancePose(_ id: DanceId, _ b: Double, _ k: Double, side: Bool = false) -> Pose {
    let s = sin(.pi * b)
    let d = onBeat(b)
    switch id {
    case .jagger:
        // the strut: hips across the floor, flapping elbows, a point up every bar
        let point = ease(min(1, pos(frac(b / 4) * 4 - 3) * 3))
        // turned three quarters towards where the strut is going
        return Pose(x: 40 * k * sin((.pi * b) / 2), y: -6 * d, rot: -6 + 8 * s, armL: 60 + 80 * pos(s), armR: mix(45 + 30 * pos(-s), 165, point), legL: 14 * pos(s), legR: 14 * pos(-s), liftL: 24 * pos(s), liftR: 24 * pos(-s), turn: side ? 0.6 * cos((.pi * b) / 2) : 0)
    case .moonwalk:
        // glide one way for four beats, turn, glide back; one heel up at a time
        let leg = idx(b / 4, 2) == 1
        let t = frac(b / 4)
        let heel = idx(b, 2) == 0
        let x = (leg ? -1.0 : 1.0) * (70 - 140 * t)
        if side {
            // in profile, facing away from the glide, spinning round at each end
            let dir = leg ? -1.0 : 1.0
            let turn = mix(-dir, dir, ease(min(1, (t * 4) / 0.6)))
            return Pose(x: x, rot: -3, armL: 18 + 14 * s, armR: 18 - 14 * s, legL: heel ? 12 : -16, legR: heel ? -16 : 12, liftL: heel ? 20 : 0, liftR: heel ? 0 : 20, turn: turn)
        }
        return Pose(x: x, rot: -4, armL: 28 + 8 * s, armR: 28 - 8 * s, legL: heel ? -8 : 4, legR: heel ? 4 : -8, liftL: heel ? 18 : 0, liftR: heel ? 0 : 18, flip: leg)
    case .twist:
        let w = sin(2 * .pi * b)
        return Pose(y: 14 + 8 * abs(w), rot: 14 * k * w, sx: 1 + 0.03 * abs(w), armL: 60 + 32 * w, armR: 60 - 32 * w, legL: 12 * w, legR: 12 * w, turn: side ? 0.35 * w : 0)
    case .robot:
        let (l, r) = arms(ROBOT, b, 8)
        let i = idx(b, 4)
        // a stiff quarter turn every couple of beats
        let turn = side ? steps(ROBOT_TURNS, b, 8) : 0
        return Pose(x: [0, 10, 0, -10][i], rot: [0, -6, 0, 6][i], armL: l, armR: r, legL: 2, legR: 2, turn: turn)
    case .disco:
        // the famous point: up to the sky, down across, hand on the hip
        let up = idx(b, 2) == 0
        let t = ease(snap(b, 4))
        let turn = !side ? 0 : (up ? mix(-0.15, 0.45, t) : mix(0.45, -0.15, t))
        return Pose(x: 14 * s, rot: 6 * s, armL: 28, armR: up ? mix(-20, 165, t) : mix(165, -20, t), legL: up ? 0 : 10, liftL: up ? 0 : 14, turn: turn)
    case .ymca:
        // Y, M, C, A, then four beats of clapping overhead
        let (l, r) = arms(YMCA, b, 5)
        return Pose(y: idx(b, 8) >= 4 ? -10 * d : 0, armL: l, armR: r)
    case .floss:
        let w = sin(2 * .pi * b)
        return Pose(x: -14 * k * w, rot: -4 * w, armL: 30 + 45 * w, armR: 30 - 45 * w, turn: side ? 0.2 * w : 0)
    case .macarena:
        // arms out, palms, cross, head, hips... then a hip wiggle and a quarter turn
        let (l, r) = arms(MACARENA, b, 5)
        let wiggle = idx(b, 8) == 7 ? sin(4 * .pi * b) : 0
        let turned = idx(b / 8, 2) == 1
        if side {
            // the quarter turn at the end of each round, shown as a real turn
            let into = ease(min(1, frac(b / 8) * 16))
            let turn = turned ? into : (b < 8 ? 0 : 1 - into)
            return Pose(x: 18 * wiggle, y: -4 * d, rot: 6 * wiggle, armL: l, armR: r, turn: turn)
        }
        return Pose(x: 18 * wiggle, y: -4 * d, rot: 6 * wiggle, armL: l, armR: r, flip: turned)
    case .running:
        if side {
            // in profile: knee up in front, the other foot sliding back, arms pumping
            return Pose(y: -8 * d, rot: 7 + 2 * s, armL: 75 * s, armR: -75 * s, legL: 42 * s, legR: -42 * s, liftL: 34 * pos(s), liftR: 34 * pos(-s), turn: 1)
        }
        return Pose(y: -8 * d, rot: 3 * s, armL: 40 + 34 * s, armR: 40 - 34 * s, legL: -14 * pos(s), legR: -14 * pos(-s), liftL: 32 * pos(s), liftR: 32 * pos(-s))
    case .horse:
        // holding the reins, galloping; every other bar the lasso goes round
        let lasso = idx(b / 4, 2) == 1
        let hop = abs(s)
        if side {
            // riding sideways: reins held out in front, legs galloping
            return Pose(y: -26 * k * hop, rot: 5 + 3 * s, armL: 70 + 6 * d, armR: lasso ? 165 + 15 * sin(4 * .pi * b) : 70 + 6 * d, legL: 24 * s, legR: 24 * sin(.pi * b + 1), liftL: 14 * pos(s), liftR: 14 * pos(-s), turn: 1)
        }
        return Pose(y: -26 * k * hop, rot: 3 * s, armL: -25 + 6 * d, armR: lasso ? 165 + 15 * sin(4 * .pi * b) : -25 + 6 * d, liftL: 14 * pos(s), liftR: 14 * pos(-s))
    case .guitar:
        // a real guitar held across the belly: the picking hand strokes across the
        // strings (down on the beat, up in between), the fretting hand slides up
        // the neck every other bar, the body rocks back on the beat and there is
        // a jump at the end of every four bars. Never further round than three
        // quarters, so the guitar always stays in front.
        let jump = idx(b, 16) == 15 ? sin(.pi * frac(b)) : 0
        let loud: Double = idx(b / 2, 2) == 1 ? 2 : 1 // every other bar strums twice as fast
        return Pose(
            y: 3 * d - 40 * k * jump, rot: 1 - 5 * k * d, armL: -60, armR: 150, legL: 16, legR: 16,
            turn: side ? 0.36 + 0.08 * sin(.pi * b / 4) : 0,
            guitar: 1, strum: cos(2 * .pi * b * loud), fret: steps(GUITAR_SLIDE, b, 3)
        )
    }
}

private func blend(_ a: Pose, _ b: Pose, _ t: Double) -> Pose {
    Pose(
        x: mix(a.x, b.x, t),
        y: mix(a.y, b.y, t),
        rot: mix(a.rot, b.rot, t),
        sx: mix(a.sx, b.sx, t),
        sy: mix(a.sy, b.sy, t),
        armL: mix(a.armL, b.armL, t),
        armR: mix(a.armR, b.armR, t),
        legL: mix(a.legL, b.legL, t),
        legR: mix(a.legR, b.legR, t),
        liftL: mix(a.liftL, b.liftL, t),
        liftR: mix(a.liftR, b.liftR, t),
        flip: t < 0.5 ? a.flip : b.flip,
        turn: mix(a.turn, b.turn, t),
        guitar: mix(a.guitar, b.guitar, t),
        strum: mix(a.strum, b.strum, t),
        fret: mix(a.fret, b.fret, t)
    )
}

private let DANCE_BEATS = 16.0

/**
 * Keeps the beat (tempo changes don't make it jump), decides when a famous
 * dance starts and blends smoothly from one move into the next. `now` is a
 * clock in milliseconds (the breathing while asleep follows it).
 */
final class Choreographer {
    private let now: () -> Double
    private(set) var beat = 0.0
    /// The famous dance going on, if any.
    private(set) var dance: DanceId?
    private var danceStart = 0.0
    private var nextAt = 8.0
    private var from = REST
    private var fromAt = -10.0
    private var last = REST
    private var wave = 0.0

    init(now: @escaping () -> Double = { CACurrentMediaTime() * 1000 }) {
        self.now = now
    }

    /// Starts a famous dance now (or a random one that suits the mood).
    func trigger(_ mood: Mood, _ id: DanceId? = nil) {
        let pool = SPECIALS[mood] ?? DanceId.allCases
        start(id ?? pool.randomElement())
    }

    /// Waves hello for a couple of seconds (when poked while paused).
    func greet() {
        wave = 2.2
    }

    private func start(_ id: DanceId?) {
        from = last
        fromAt = beat
        dance = id
        danceStart = ceil(beat)
    }

    /**
     * Advances by `dt` seconds and returns the pose to draw. `pinned` keeps one
     * famous dance going all the time.
     */
    func step(_ dt: Double, playing: Bool, bpm: Double, energy: Double, mood: Mood, specials: Specials, pinned: DanceId?, turns: Turns = .all) -> Pose {
        let k = playing ? 0.5 + min(1, energy * 1.6) * 0.8 : 0
        wave = max(0, wave - dt)
        if !playing {
            // asleep: arms down, breathing; or waving when poked
            let breathe = sin(now() / 900)
            let p = Pose(y: 4 + 3 * breathe, sy: 1 + 0.012 * breathe, armL: 12, armR: wave > 0 ? 150 + 25 * sin(wave * 14) : 12)
            last = blend(last, p, min(1, dt * 6))
            return last
        }
        beat += (dt * bpm) / 60

        if let pinned, dance != pinned { start(pinned) }
        if pinned == nil {
            if dance != nil && beat - danceStart >= DANCE_BEATS {
                start(nil)
                nextAt = beat + (specials == .often ? 16 : 32) + Double(Int.random(in: 0..<8))
            } else if dance == nil && specials != .off && beat >= nextAt {
                trigger(mood)
            }
        }

        let b = beat - danceStart
        var p: Pose
        if let current = dance, b >= 0 {
            p = dancePose(current, b, k, side: turns != .off)
        } else {
            p = moodPose(mood, beat, k, side: turns == .all)
        }
        // half a beat to blend into a new move
        let t = min(1, (beat - fromAt) / 0.5)
        if t < 1 { p = blend(from, p, ease(t)) }
        if wave > 0 { p.armR = 150 + 25 * sin(wave * 14) }
        last = p
        return p
    }
}
