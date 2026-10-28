import SwiftUI

// The HomeFi mascot (a white house wearing headphones) drawn on a Canvas in
// its own units: the art is 1000 wide, shown through the desktop's viewBox
// "40 40 1000 900". Port of the Android `ui/player/Mascot.kt` (which follows
// the desktop's `components/Mascot.svelte`); the body moves with [Choreographer].

private let VIEW_X: CGFloat = 40
private let VIEW_Y: CGFloat = 40
private let VIEW_W: CGFloat = 1000
private let VIEW_H: CGFloat = 900

/// Feet on the ground: the body turns and squashes around this point.
private let FEET = CGPoint(x: 512, y: 896)
private let TILT_PIVOT = CGPoint(x: 512, y: 560)
private let TILT: CGFloat = -6

// limbs (keep in sync with the desktop and Android)
private let SHOULDER_L = CGPoint(x: 268, y: 730)
private let SHOULDER_R = CGPoint(x: 756, y: 730)
private let ARM: CGFloat = 130
private let ARM_W: CGFloat = 56
private let HAND: CGFloat = 38
private let HIP_L = CGPoint(x: 420, y: 800)
private let HIP_R = CGPoint(x: 604, y: 780)
private let GROUND: CGFloat = 896
private let LEG_W: CGFloat = 62
private let FOOT_RX: CGFloat = 50
private let FOOT_RY: CGFloat = 28
private let FOOT_SHIFT: CGFloat = 20
private let OUTLINE: CGFloat = 8

private let LX: CGFloat = 440
private let LY: CGFloat = 600
private let RX: CGFloat = 584
private let RY: CGFloat = 600
private let EYE_R: CGFloat = 34

// turning sideways (keep in sync with the desktop and Android)
private let HEAD_R: CGFloat = 270
private let BEHIND: CGFloat = -0.3
private let BAND_W: CGFloat = 50

private extension Color {
    init(hex: UInt32) {
        self.init(
            red: Double((hex >> 16) & 0xFF) / 255,
            green: Double((hex >> 8) & 0xFF) / 255,
            blue: Double(hex & 0xFF) / 255
        )
    }
}

/// Art colours, same as the desktop's MASCOT_COLORS and costume palette.
private enum Ink {
    static let navy = Color(hex: 0x0B1E3A)
    static let white = Color.white
    static let cushion = Color(hex: 0x34D399)
    static let cheek = Color(hex: 0xF472B6)
    static let note = Color(hex: 0xFDE68A)
    static let yellow = Color(hex: 0xFDE047)
    static let pink = Color(hex: 0xF472B6)
    static let lightPink = Color(hex: 0xF9A8D4)
    static let hotPink = Color(hex: 0xEC4899)
    static let red = Color(hex: 0xEF4444)
    static let darkRed = Color(hex: 0xDC2626)
    static let maroon = Color(hex: 0x7F1D1D)
    static let brick = Color(hex: 0x991B1B)
    static let crimson = Color(hex: 0xB91C1C)
    static let sky = Color(hex: 0x38BDF8)
    static let amber = Color(hex: 0xFCD34D)
    static let gold = Color(hex: 0xFBBF24)
    static let bronze = Color(hex: 0xB45309)
    static let brown = Color(hex: 0x7C2D12)
    static let shades = Color(hex: 0x05070D)
    static let leather = Color(hex: 0x111827)
    static let steel = Color(hex: 0xD1D5DB)
    static let silver = Color(hex: 0xCBD5E1)
    static let green = Color(hex: 0x22C55E)
    static let darkGreen = Color(hex: 0x15803D)
    static let beret = Color(hex: 0x1E3A8A)
}

/// A colour that cycles through the hues, like the desktop's `hue-rotate` neon.
private struct Neon {
    private let h: Double
    private let s: Double
    private let v: Double

    init(_ hex: UInt32) {
        let r = Double((hex >> 16) & 0xFF) / 255
        let g = Double((hex >> 8) & 0xFF) / 255
        let b = Double(hex & 0xFF) / 255
        let maxC = max(r, g, b)
        let minC = min(r, g, b)
        let d = maxC - minC
        var hue = 0.0
        if d > 0 {
            if maxC == r {
                hue = 60 * ((g - b) / d).truncatingRemainder(dividingBy: 6)
            } else if maxC == g {
                hue = 60 * ((b - r) / d + 2)
            } else {
                hue = 60 * ((r - g) / d + 4)
            }
        }
        h = hue < 0 ? hue + 360 : hue
        s = maxC == 0 ? 0 : d / maxC
        v = maxC
    }

    func at(_ shift: Double) -> Color {
        Color(hue: (h + shift).truncatingRemainder(dividingBy: 360) / 360, saturation: s, brightness: v)
    }
}

private func heart(_ x: CGFloat, _ y: CGFloat, _ s: CGFloat) -> String {
    "M\(x) \(y + s * 0.3)C\(x) \(y - s * 0.2) \(x - s) \(y - s * 0.2) \(x - s) \(y + s * 0.35)" +
        "C\(x - s) \(y + s * 0.8) \(x) \(y + s * 1.1) \(x) \(y + s * 1.3)" +
        "C\(x) \(y + s * 1.1) \(x + s) \(y + s * 0.8) \(x + s) \(y + s * 0.35)" +
        "C\(x + s) \(y - s * 0.2) \(x) \(y - s * 0.2) \(x) \(y + s * 0.3)Z"
}

private func star(_ x: CGFloat, _ y: CGFloat, _ r: CGFloat, _ inner: CGFloat = 0.45) -> String {
    "M" + (0..<10).map { i in
        let a = CGFloat.pi / 5 * CGFloat(i) - CGFloat.pi / 2
        let d = i % 2 == 1 ? r * inner : r
        return "\(x + d * cos(a)) \(y + d * sin(a))"
    }.joined(separator: "L") + "Z"
}

private func bolt(_ x: CGFloat, _ y: CGFloat, _ s: CGFloat) -> String {
    "M\(x) \(y)L\(x - s * 0.5) \(y + s * 1.1)L\(x - s * 0.05) \(y + s * 1.1)L\(x - s * 0.35) \(y + s * 2)" +
        "L\(x + s * 0.55) \(y + s * 0.75)L\(x + s * 0.08) \(y + s * 0.75)L\(x + s * 0.35) \(y)Z"
}

/// Every path, parsed once (from the desktop's `lib/mascot.ts` and Mascot.svelte).
private enum Art {
    static let house = Path(svg: "M542.1 288L748.1 466.2A46 46 0 0 1 764 501L764 760A46 46 0 0 1 718 806L306 806A46 46 0 0 1 260 760L260 501A46 46 0 0 1 275.9 466.2L481.9 288A46 46 0 0 1 542.1 288Z")
    // one cup / pad per side, so each can slide round the head on its own
    static let cupL = Path(svg: "M236 508H248A40 40 0 0 1 288 548V664A40 40 0 0 1 248 704H236A40 40 0 0 1 196 664V548A40 40 0 0 1 236 508Z")
    static let cupR = Path(svg: "M776 508H788A40 40 0 0 1 828 548V664A40 40 0 0 1 788 704H776A40 40 0 0 1 736 664V548A40 40 0 0 1 776 508Z")
    static let padL = Path(svg: "M277 540H277A15 15 0 0 1 292 555V657A15 15 0 0 1 277 672H277A15 15 0 0 1 262 657V555A15 15 0 0 1 277 540Z")
    static let padR = Path(svg: "M747 540H747A15 15 0 0 1 762 555V657A15 15 0 0 1 747 672H747A15 15 0 0 1 732 657V555A15 15 0 0 1 747 540Z")
    static let eyes = Path(svg: "M406 600A34 34 0 1 0 474 600A34 34 0 1 0 406 600ZM550 600A34 34 0 1 0 618 600A34 34 0 1 0 550 600Z")
    static let sparkles = Path(svg: "M441 586A11 11 0 1 0 463 586A11 11 0 1 0 441 586ZM585 586A11 11 0 1 0 607 586A11 11 0 1 0 585 586Z")
    static let smile = Path(svg: "M432 668Q512 760 592 668Q512 716 432 668Z")
    static let cheeks = Path(svg: "M354 676A28 28 0 1 0 410 676A28 28 0 1 0 354 676ZM614 676A28 28 0 1 0 670 676A28 28 0 1 0 614 676Z")
    static let notes = Path(svg: "M810.5 330.7A34 25 -22 1 0 873.5 305.3A34 25 -22 1 0 810.5 330.7ZM918.5 294.7A34 25 -22 1 0 981.5 269.3A34 25 -22 1 0 918.5 294.7ZM868 156H868A8 8 0 0 1 876 164V308A8 8 0 0 1 868 316H868A8 8 0 0 1 860 308V164A8 8 0 0 1 868 156ZM976 120H976A8 8 0 0 1 984 128V272A8 8 0 0 1 976 280H976A8 8 0 0 1 968 272V128A8 8 0 0 1 976 120ZM860 150L984 114L984 152L860 188ZM103.7 309A24 18 -22 1 0 148.3 291A24 18 -22 1 0 103.7 309ZM144 196H144A6 6 0 0 1 150 202V294A6 6 0 0 1 144 300H144A6 6 0 0 1 138 294V202A6 6 0 0 1 144 196ZM138 190Q178 208 176 250Q166 226 150 222Z")

    // faces
    static let closedEyes = Path(svg: "M\(LX - EYE_R) \(LY)Q\(LX) \(LY + EYE_R * 0.9) \(LX + EYE_R) \(LY)M\(RX - EYE_R) \(RY)Q\(RX) \(RY + EYE_R * 0.9) \(RX + EYE_R) \(RY)")
    static let happyEyes = Path(svg: "M\(LX - EYE_R) \(LY + 8)Q\(LX) \(LY - EYE_R * 0.9) \(LX + EYE_R) \(LY + 8)M\(RX - EYE_R) \(RY + 8)Q\(RX) \(RY - EYE_R * 0.9) \(RX + EYE_R) \(RY + 8)")
    static let coolEyes = Path(svg: "M\(LX - EYE_R) \(LY)A\(EYE_R) \(EYE_R) 0 0 0 \(LX + EYE_R) \(LY)ZM\(RX - EYE_R) \(RY)A\(EYE_R) \(EYE_R) 0 0 0 \(RX + EYE_R) \(RY)Z")
    static let coolLids = Path(svg: "M\(LX - EYE_R) \(LY)L\(LX + EYE_R) \(LY)M\(RX - EYE_R) \(RY)L\(RX + EYE_R) \(RY)")
    static let angryBrows = Path(svg: "M398 548L478 578M626 548L546 578")
    static let smirk = Path(svg: "M470 690Q512 712 560 684")
    static let calmMouth = Path(svg: "M478 688Q512 712 546 688")
    static let shadesBridge = Path(svg: "M\(LX + EYE_R + 22) \(LY - 10)L\(RX - EYE_R - 22) \(RY - 10)")
    static let shadesGlint = Path(svg: "M\(LX - EYE_R - 4) \(LY - 14)L\(LX - 6) \(LY - 14)M\(RX - EYE_R - 4) \(RY - 14)L\(RX - 6) \(RY - 14)")

    // fx
    static let boltBig = Path(svg: bolt(880, 150, 90))
    static let boltSmall = Path(svg: bolt(150, 220, 70))
    static let shock = Path(svg: "M120 520q-40 60 0 120M80 500q-60 90 0 180M904 520q40 60 0 120M944 500q60 90 0 180")
    static let heartBig = Path(svg: heart(870, 160, 60))
    static let heartMid = Path(svg: heart(160, 250, 42))
    static let heartSmall = Path(svg: heart(950, 420, 30))

    // makeup
    static let glamBolt = Path(svg: "M392 500L470 500L438 590L494 590L388 726L416 620L366 620Z")
    static let glamEdge = Path(svg: "M404 512L452 512L426 584")
    static let punkBrows = Path(svg: "M392 552Q440 530 488 552M536 552Q584 530 632 552")
    static let starsPink = Path(svg: star(372, 628, 14) + star(652, 628, 14))
    static let starsGold = Path(svg: star(396, 720, 9) + star(628, 720, 9))
    static let uvSmile = Path(svg: "M470 760L512 744L554 760")
    static let salsaBrows = Path(svg: "M400 560Q440 536 480 560M544 560Q584 536 624 560")

    // costumes
    static let spikeXList: [CGFloat] = (0..<9).map { 300 + CGFloat($0) * 53 }
    static let spikes = Path(svg: spikeXList.map { x in "M\(x - 14) 744L\(x) 712L\(x + 14) 744Z" }.joined())
    static let hornL = Path(svg: "M392 262Q340 200 356 128Q384 196 440 226Z")
    static let hornR = Path(svg: "M632 262Q684 200 668 128Q640 196 584 226Z")
    static let mohawk = Path(svg: "M432 224L436 60L478 206L496 10L524 204L552 26L568 208L600 76L594 228Z")
    static let pin = Path(svg: "M300 760H372Q388 760 388 746Q388 732 372 732H312")
    static let bowL = Path(svg: "M640 372L580 330Q560 372 580 414Z")
    static let bowR = Path(svg: "M640 372L700 330Q720 372 700 414Z")
    static let lashes = Path(svg: "M402 582L388 568M414 572L406 554M626 572L634 554M638 582L652 568")
    static let glowNecklace = Path(svg: "M290 770Q512 830 734 770")
    static let capCrown = Path(svg: "M372 230Q380 120 512 112Q644 120 652 230Z")
    static let capBrim = Path(svg: "M372 230Q300 236 268 212Q300 196 380 204Z")
    static let chain = Path(svg: "M320 744Q512 820 704 744")
    static let medalNote = Path(svg: "M504 774V800M504 774L524 768V794")
    static let glassesBridge = Path(svg: "M\(LX + 50) \(LY)Q512 \(LY - 16) \(RX - 50) \(RY)")
    static let bowTie = Path(svg: "M512 784L456 756V812ZM512 784L568 756V812Z")

    /// A "z" for the sleeping mascot, in a unit square.
    static let zed = Path(svg: "M0 0H1L0 1H1")

    static let uvTeal = Neon(0x5EEAD4)
    static let uvPink = Neon(0xE879F9)
}

/// Strokes in mascot units.
private enum Pen {
    static func round(_ w: CGFloat) -> StrokeStyle { StrokeStyle(lineWidth: w, lineCap: .round, lineJoin: .round) }
    static let w5 = round(5)
    static let w6 = round(6)
    static let w7 = round(7)
    static let w8 = round(8)
    static let w9 = StrokeStyle(lineWidth: 9)
    static let w10 = round(10)
    static let w12 = round(12)
    static let w14 = round(14)
    static let w16 = round(16)
    static let w18 = round(18)
    static let plain8 = StrokeStyle(lineWidth: 8, lineJoin: .round)
    static let zed = round(0.2)
    static let chain = StrokeStyle(lineWidth: 10, lineCap: .round, dash: [14, 6])
}

// ------------------------------------------------------------------ view

/// Keeps the choreography between frames (SwiftUI views are rebuilt all the time).
private final class MascotEngine {
    let choreo = Choreographer()
    private var last: Date?
    private(set) var pose = REST
    private(set) var seconds = 0.0

    func advance(to date: Date, playing: Bool, bpm: Double, energy: Double, mood: Mood, specials: Specials, pinned: DanceId?, turns: Turns) {
        let dt = last.map { min(0.1, max(0, date.timeIntervalSince($0))) } ?? 0
        last = date
        pose = choreo.step(dt, playing: playing, bpm: bpm, energy: energy, mood: mood, specials: specials, pinned: pinned, turns: turns)
        seconds = date.timeIntervalSinceReferenceDate
    }
}

/**
 * The HomeFi mascot dancing to the music: its groove and costume follow the
 * genre (or the user's pick), famous dances come up now and then. Tap it to
 * make it dance (or wave while paused), drag to move it, pinch to resize,
 * double-tap to put it back.
 */
struct MascotView: View {
    var playing: Bool
    var mood: Mood
    /// Beats per minute (120 when unknown).
    var bpm: Double = 120
    /// How loud it is, 0...1.
    var energy: Double = 0.5
    /// A famous dance kept going all the time; nil follows the genre.
    var dance: DanceId?
    var specials: Specials = .sometimes
    var turns: Turns = .all
    var limbs: MascotLimbs = .white
    var accent: Color = .accentColor
    /// Where the user dragged it, as a fraction of its stage (0 = centred).
    @Binding var offset: CGSize
    /// Pinch zoom, 0.4...1.8.
    @Binding var scale: CGFloat

    @State private var engine = MascotEngine()
    @State private var dragStart: CGSize?
    @State private var zoomStart: CGFloat?

    var body: some View {
        GeometryReader { geo in
            let stageW = max(1, geo.size.width)
            let stageH = max(1, geo.size.height)
            let width = min(stageW, stageH * VIEW_W / VIEW_H) * 0.92 * scale
            TimelineView(.animation) { timeline in
                let _ = engine.advance(
                    to: timeline.date, playing: playing, bpm: bpm, energy: energy,
                    mood: mood, specials: specials, pinned: dance, turns: turns
                )
                Canvas { ctx, size in
                    var c = ctx
                    let s = size.width / VIEW_W
                    c.scaleBy(x: s, y: s)
                    c.translateBy(x: -VIEW_X, y: -VIEW_Y)
                    drawMascot(
                        c, engine.pose, mood: mood, playing: playing, limbs: limbs, accent: accent,
                        muted: .secondary, ground: Color.primary.opacity(0.12), t: engine.seconds, beat: engine.choreo.beat
                    )
                }
                .frame(width: width, height: width * VIEW_H / VIEW_W)
            }
            .contentShape(Rectangle())
            .onTapGesture(count: 2) {
                offset = .zero
                scale = 1
            }
            .onTapGesture {
                if playing { engine.choreo.trigger(mood) } else { engine.choreo.greet() }
            }
            .gesture(
                DragGesture()
                    .onChanged { v in
                        let start = dragStart ?? offset
                        dragStart = start
                        offset = CGSize(
                            width: min(0.5, max(-0.5, start.width + v.translation.width / stageW)),
                            height: min(0.5, max(-0.5, start.height + v.translation.height / stageH))
                        )
                    }
                    .onEnded { _ in dragStart = nil }
                    .simultaneously(
                        with: MagnifyGesture()
                            .onChanged { v in
                                let start = zoomStart ?? scale
                                zoomStart = start
                                scale = min(1.8, max(0.4, start * v.magnification))
                            }
                            .onEnded { _ in zoomStart = nil }
                    )
            )
            .offset(x: offset.width * stageW, y: offset.height * stageH)
            .frame(width: stageW, height: stageH)
            .accessibilityElement()
            .accessibilityLabel(String(localized: "mascot.description", defaultValue: "HomeFi mascot. Tap to make it dance, drag to move it, pinch to resize, double-tap to put it back."))
            .accessibilityAddTraits(.isButton)
        }
    }
}

// ------------------------------------------------------------------ drawing

private func rotated(_ ctx: GraphicsContext, _ degrees: CGFloat, around p: CGPoint) -> GraphicsContext {
    var c = ctx
    c.translateBy(x: p.x, y: p.y)
    c.rotate(by: .degrees(Double(degrees)))
    c.translateBy(x: -p.x, y: -p.y)
    return c
}

private func scaled(_ ctx: GraphicsContext, _ sx: CGFloat, _ sy: CGFloat, around p: CGPoint) -> GraphicsContext {
    var c = ctx
    c.translateBy(x: p.x, y: p.y)
    c.scaleBy(x: sx, y: sy)
    c.translateBy(x: -p.x, y: -p.y)
    return c
}

private func translated(_ ctx: GraphicsContext, _ x: CGFloat, _ y: CGFloat) -> GraphicsContext {
    var c = ctx
    c.translateBy(x: x, y: y)
    return c
}

private func circle(_ c: CGPoint, _ r: CGFloat) -> Path {
    Path(ellipseIn: CGRect(x: c.x - r, y: c.y - r, width: 2 * r, height: 2 * r))
}

private func oval(_ x: CGFloat, _ y: CGFloat, _ w: CGFloat, _ h: CGFloat) -> Path {
    Path(ellipseIn: CGRect(x: x, y: y, width: w, height: h))
}

private func roundRect(_ x: CGFloat, _ y: CGFloat, _ w: CGFloat, _ h: CGFloat, _ r: CGFloat) -> Path {
    Path(roundedRect: CGRect(x: x, y: y, width: w, height: h), cornerRadius: r)
}

private func line(_ a: CGPoint, _ b: CGPoint) -> Path {
    var p = Path()
    p.move(to: a)
    p.addLine(to: b)
    return p
}

private func fill(_ ctx: GraphicsContext, _ path: Path, _ color: Color, alpha: Double = 1) {
    ctx.fill(path, with: .color(alpha == 1 ? color : color.opacity(alpha)))
}

private func stroke(_ ctx: GraphicsContext, _ path: Path, _ color: Color, _ style: StrokeStyle, alpha: Double = 1) {
    ctx.stroke(path, with: .color(alpha == 1 ? color : color.opacity(alpha)), style: style)
}

/// The whole mascot in its own units. `t` is a clock in seconds, `beat` the dance's beat count.
private func drawMascot(
    _ ctx: GraphicsContext,
    _ p: Pose,
    mood: Mood,
    playing: Bool,
    limbs: MascotLimbs,
    accent: Color,
    muted: Color,
    ground: Color,
    t: Double,
    beat: Double
) {
    let hue = t.truncatingRemainder(dividingBy: 1.6) / 1.6 * 360

    // a shadow on the floor that shrinks as it jumps
    let air = CGFloat(min(0.5, max(0, -p.y / 200)))
    let rx = 230 * (1 - air)
    fill(ctx, oval(512 + CGFloat(p.x) - rx, GROUND + 8, 2 * rx, 28), ground)

    if playing { drawFx(ctx, mood, t, hue) } else { drawZeds(ctx, t, muted) }

    var body = translated(ctx, CGFloat(p.x), CGFloat(p.y))
    body = rotated(body, CGFloat(p.rot), around: FEET)
    body = scaled(body, CGFloat(p.sx) * (p.flip ? -1 : 1), CGFloat(p.sy), around: FEET)

    let fillColor: Color? = switch limbs {
    case .white: Ink.white
    case .navy: Ink.navy
    case .accent: accent
    case .off: nil
    }
    let outline: Color? = limbs == .white ? Ink.navy : nil
    let turn = Turn(CGFloat(p.turn))
    // legs: outside the tilt, behind the house; toes point where it faces
    if let fillColor {
        [(turn.limb(HIP_L, CGFloat(p.legL), 1), p.liftL), (turn.limb(HIP_R, CGFloat(p.legR), -1), p.liftR)]
            .sorted { $0.0.depth < $1.0.depth }
            .forEach { drawLeg(body, $0.0, CGFloat($0.1), fillColor, outline) }
    }
    let tilt = rotated(body, TILT, around: TILT_PIVOT)
    let arms: [Turn.Limb] = fillColor == nil ? [] :
        [turn.limb(SHOULDER_L, CGFloat(p.armL), 1), turn.limb(SHOULDER_R, CGFloat(p.armR), -1)].sorted { $0.depth < $1.depth }
    let cups = turn.cups()
    let band = turn.bandHalves(cups)
    let bandPen = Pen.round(BAND_W * (1 - 0.45 * abs(turn.side)))
    func drawBand(_ half: Path) {
        stroke(tilt, half, Ink.navy, bandPen)
        if mood == .party {
            // LEDs chasing round the head band
            let chase = playing ? CGFloat((beat / 2).truncatingRemainder(dividingBy: 1) * 80) : 0
            stroke(tilt, half, Art.uvTeal.at(hue), StrokeStyle(lineWidth: 10, lineCap: .round, dash: [6, 34], dashPhase: chase))
        }
    }
    if let fillColor { arms.filter { $0.depth < BEHIND }.forEach { drawArm(tilt, $0, fillColor, outline) } }
    // cups and the band half reaching them go behind as soon as they face away
    band.filter { $0.depth < 0 }.forEach { drawBand($0.path) }
    cups.filter { $0.depth < 0 }.forEach { drawCup(tilt, $0, mood) }

    let house = scaled(tilt, turn.narrow, 1, around: CGPoint(x: 512, y: 0))
    fill(house, Art.house, Ink.white)
    drawBodyDecor(house, mood, hue)
    var face = house
    face.clip(to: Art.house)
    face = translated(face, turn.faceShift, 0)
    face = scaled(face, turn.faceScale, 1, around: CGPoint(x: 512, y: 0))
    drawMakeup(face, mood, hue)
    drawFace(face, mood, playing, t)
    fill(face, Art.cheeks, Ink.cheek, alpha: playing && mood == .cute ? 0.9 : 0.6)
    drawFaceDecor(face, mood)

    band.filter { $0.depth >= 0 }.forEach { drawBand($0.path) }
    cups.filter { $0.depth >= 0 }.forEach { drawCup(tilt, $0, mood) }
    var top = scaled(tilt, turn.narrow, 1, around: CGPoint(x: 512, y: 0))
    top = translated(top, turn.topShift, 0)
    top = scaled(top, turn.topScale, 1, around: CGPoint(x: 512, y: 0))
    drawTopDecor(top, mood)
    // arms in front, so raised arms don't hide behind the headphones
    if let fillColor { arms.filter { $0.depth >= BEHIND }.forEach { drawArm(tilt, $0, fillColor, outline) } }
}

/**
 * Turning sideways, drawn as if the head were a cylinder: the face and the
 * headphone cups slide round it, the house gets narrower (its side is not as
 * wide as its front) and the limbs on the far side go behind the body.
 */
private struct Turn {
    let side: CGFloat
    private let across: CGFloat
    let narrow: CGFloat
    // headphones sit a little behind the middle of the head when seen from the side
    private let back: CGFloat
    let faceShift: CGFloat
    let faceScale: CGFloat
    let topShift: CGFloat
    let topScale: CGFloat

    init(_ turn: CGFloat) {
        side = sin(turn * .pi / 2)
        across = cos(turn * .pi / 2)
        narrow = 1 - 0.2 * abs(side)
        back = -46 * side
        // the face turns less than the body so a profile still shows one eye
        let faceTh = turn * 55 * .pi / 180
        faceShift = 210 * sin(faceTh)
        faceScale = cos(faceTh)
        topShift = 110 * sin(faceTh)
        topScale = 1 - 0.6 * (1 - cos(faceTh))
    }

    struct Cup {
        let home: CGFloat
        let x: CGFloat
        let depth: CGFloat
        let widen: CGFloat
        let cup: Path
        let pad: Path
    }

    func cups() -> [Cup] {
        [
            Cup(home: 242, x: 512 + back - HEAD_R * narrow * across, depth: side, widen: widen(side), cup: Art.cupL, pad: Art.padL),
            Cup(home: 782, x: 512 + back + HEAD_R * narrow * across, depth: -side, widen: widen(-side), cup: Art.cupR, pad: Art.padR),
        ]
    }

    private func widen(_ depth: CGFloat) -> CGFloat { 1 + 0.45 * max(0, depth) * abs(side) }

    /// The head band, split at its top: each half goes with its cup (and its
    /// depth), so the half reaching the far cup passes behind the head instead
    /// of across the face.
    func bandHalves(_ cups: [Cup]) -> [(path: Path, depth: CGFloat)] {
        let l = cups[0].x - 16
        let r = cups[1].x + 16
        let m = (l + r) / 2
        var left = Path()
        left.move(to: CGPoint(x: l, y: 560))
        left.addCurve(to: CGPoint(x: m, y: 236), control1: CGPoint(x: l, y: 344), control2: CGPoint(x: (3 * l + r) / 4, y: 236))
        var right = Path()
        right.move(to: CGPoint(x: m, y: 236))
        right.addCurve(to: CGPoint(x: r, y: 560), control1: CGPoint(x: (l + 3 * r) / 4, y: 236), control2: CGPoint(x: r, y: 344))
        return [(left, cups[0].depth), (right, cups[1].depth)]
    }

    /// Where a limb joins the body and how it is rotated (degrees, clockwise).
    struct Limb {
        let at: CGPoint
        let rot: CGFloat
        let foot: CGFloat
        let depth: CGFloat
    }

    /// `front` is the rotation direction when facing you (+1 left, -1 right); in profile both swing towards where it faces.
    func limb(_ joint: CGPoint, _ angle: CGFloat, _ front: CGFloat) -> Limb {
        let sgn: CGFloat = side > 0 ? 1 : (side < 0 ? -1 : 0)
        let towards = side == 0 ? front : -sgn
        let s = abs(side)
        return Limb(
            at: CGPoint(x: 512 + (joint.x - 512) * narrow * across + 18 * side, y: joint.y),
            rot: angle * (front + (towards - front) * s),
            foot: front > 0 ? -1 + (sgn + 1) * s : 1 + (sgn - 1) * s,
            depth: front * side
        )
    }
}

/// One headphone cup (and the hoop hanging from that ear in salsa).
private func drawCup(_ ctx: GraphicsContext, _ c: Turn.Cup, _ mood: Mood) {
    var g = translated(ctx, c.x, 0)
    g.scaleBy(x: c.widen, y: 1)
    g.translateBy(x: -c.home, y: 0)
    fill(g, c.cup, Ink.navy)
    fill(g, c.pad, Ink.cushion)
    if mood == .salsa { stroke(g, circle(CGPoint(x: c.home, y: 738), 26), Ink.gold, Pen.w9) }
}

/// One arm from its shoulder, rotated clockwise by `rot` from hanging down.
private func drawArm(_ ctx: GraphicsContext, _ arm: Turn.Limb, _ fillColor: Color, _ outline: Color?) {
    let a = arm.rot * .pi / 180
    let shoulder = arm.at
    let hand = CGPoint(x: shoulder.x - ARM * sin(a), y: shoulder.y + ARM * cos(a))
    if let outline {
        stroke(ctx, line(shoulder, hand), outline, StrokeStyle(lineWidth: ARM_W + 2 * OUTLINE, lineCap: .round))
        fill(ctx, circle(hand, HAND + OUTLINE), outline)
    }
    stroke(ctx, line(shoulder, hand), fillColor, StrokeStyle(lineWidth: ARM_W, lineCap: .round))
    fill(ctx, circle(hand, HAND), fillColor)
}

/// One leg straight down to the ground, lifted and turned (degrees, clockwise) around the hip.
private func drawLeg(_ ctx: GraphicsContext, _ leg: Turn.Limb, _ lift: CGFloat, _ fillColor: Color, _ outline: Color?) {
    let hip = leg.at
    let g = rotated(translated(ctx, 0, -lift), leg.rot, around: hip)
    let end = CGPoint(x: hip.x, y: GROUND)
    let foot = CGPoint(x: hip.x + leg.foot * FOOT_SHIFT, y: GROUND)
    if let outline {
        stroke(g, line(hip, end), outline, StrokeStyle(lineWidth: LEG_W + 2 * OUTLINE, lineCap: .round))
        let rx = FOOT_RX + OUTLINE
        let ry = FOOT_RY + OUTLINE
        fill(g, oval(foot.x - rx, foot.y - ry, 2 * rx, 2 * ry), outline)
    }
    stroke(g, line(hip, end), fillColor, StrokeStyle(lineWidth: LEG_W, lineCap: .round))
    fill(g, oval(foot.x - FOOT_RX, foot.y - FOOT_RY, 2 * FOOT_RX, 2 * FOOT_RY), fillColor)
}

/// Things floating around, depending on the mood.
private func drawFx(_ ctx: GraphicsContext, _ mood: Mood, _ t: Double, _ hue: Double) {
    let fx: CGFloat
    let fy: CGFloat
    if mood == .mosh {
        // a jittery shake
        let even = Int(t / 0.105) % 2 == 0
        (fx, fy) = even ? (-6, 4) : (6, -4)
    } else {
        let f = CGFloat(0.5 - 0.5 * cos(2 * .pi * t / 2.2))
        (fx, fy) = (10 * f, -26 * f)
    }
    let g = translated(ctx, fx, fy)
    switch mood {
    case .mosh:
        fill(g, Art.boltBig, Ink.yellow)
        fill(g, Art.boltSmall, Ink.yellow)
        stroke(g, Art.shock, Ink.yellow, Pen.w16, alpha: 0.8)
    case .cute:
        fill(g, Art.heartBig, Ink.pink)
        fill(g, Art.heartMid, Ink.lightPink)
        fill(g, Art.heartSmall, Ink.pink)
    case .party:
        fill(g, Art.notes, Art.uvTeal.at(hue))
    default:
        fill(g, Art.notes, Ink.note)
    }
}

private let ZED_SIZES: [CGFloat] = [40, 52, 80]

/// Three z's drifting up while it sleeps.
private func drawZeds(_ ctx: GraphicsContext, _ t: Double, _ color: Color) {
    for i in 0..<3 {
        let m = (t - Double(i)).truncatingRemainder(dividingBy: 3)
        let phase = CGFloat((m < 0 ? m + 3 : m) / 3)
        let alpha = phase < 0.3 ? phase / 0.3 * 0.9 : (1 - phase) / 0.7 * 0.9
        let e = phase * phase // ease-in
        let h = ZED_SIZES[i]
        var g = translated(ctx, 790 + 110 * e, 300 - 200 * e - h)
        g.scaleBy(x: h, y: h)
        stroke(g, Art.zed, color, Pen.zed, alpha: Double(alpha))
    }
}

/// Makeup, under the eyes.
private func drawMakeup(_ ctx: GraphicsContext, _ mood: Mood, _ hue: Double) {
    switch mood {
    case .mosh:
        // Aladdin Sane bolt across the left eye
        fill(ctx, Art.glamBolt, Ink.red, alpha: 0.92)
        stroke(ctx, Art.glamEdge, Ink.sky, Pen.plain8)
    case .pogo:
        stroke(ctx, Art.punkBrows, Ink.navy, Pen.w12)
        // X plaster
        let g = rotated(ctx, 30, around: CGPoint(x: 660, y: 700))
        fill(g, roundRect(620, 688, 80, 24, 8), Ink.amber)
        fill(g, roundRect(648, 660, 24, 80, 8), Ink.amber)
    case .cute:
        fill(ctx, Art.starsPink, Ink.lightPink)
        fill(ctx, Art.starsGold, Ink.yellow)
    case .party:
        // UV face paint
        let teal = Art.uvTeal.at(hue)
        let pink = Art.uvPink.at(hue)
        fill(ctx, circle(CGPoint(x: 380, y: 660), 9), teal)
        fill(ctx, circle(CGPoint(x: 400, y: 690), 9), pink)
        fill(ctx, circle(CGPoint(x: 360, y: 690), 9), teal)
        fill(ctx, circle(CGPoint(x: 644, y: 660), 9), pink)
        fill(ctx, circle(CGPoint(x: 624, y: 690), 9), teal)
        fill(ctx, circle(CGPoint(x: 664, y: 690), 9), pink)
        stroke(ctx, Art.uvSmile, teal, Pen.w8)
    case .salsa:
        stroke(ctx, Art.salsaBrows, Ink.brown, Pen.w10)
    default:
        break
    }
}

private func drawFace(_ ctx: GraphicsContext, _ mood: Mood, _ playing: Bool, _ t: Double) {
    if !playing {
        stroke(ctx, Art.closedEyes, Ink.navy, Pen.w12)
        fill(ctx, oval(512 - 18, 690 - 12, 36, 24), Ink.navy)
        return
    }
    switch mood {
    case .mosh:
        stroke(ctx, Art.angryBrows, Ink.navy, Pen.w18)
        fill(ctx, circle(CGPoint(x: LX, y: LY + 8), EYE_R * 0.7), Ink.navy)
        fill(ctx, circle(CGPoint(x: RX, y: RY + 8), EYE_R * 0.7), Ink.navy)
        fill(ctx, oval(512 - 46, 704 - 40, 92, 80), Ink.navy)
        fill(ctx, oval(512 - 26, 724 - 14, 52, 28), Ink.pink)
    case .party:
        // sunglasses
        fill(ctx, roundRect(LX - EYE_R - 22, LY - 30, EYE_R * 2 + 44, 62, 22), Ink.shades)
        fill(ctx, roundRect(RX - EYE_R - 22, RY - 30, EYE_R * 2 + 44, 62, 22), Ink.shades)
        stroke(ctx, Art.shadesBridge, Ink.shades, StrokeStyle(lineWidth: 14))
        stroke(ctx, Art.shadesGlint, Art.uvTeal.at(0), Pen.w8, alpha: 0.8)
        fill(ctx, Art.smile, Ink.navy)
    case .groove:
        fill(ctx, Art.coolEyes, Ink.navy)
        stroke(ctx, Art.coolLids, Ink.navy, Pen.w10)
        stroke(ctx, Art.smirk, Ink.navy, Pen.w14)
    case .chill:
        stroke(ctx, Art.happyEyes, Ink.navy, Pen.w12)
        stroke(ctx, Art.calmMouth, Ink.navy, Pen.w12)
    default:
        // cute, pogo, salsa, bop: big shiny eyes that blink now and then
        let c = CGFloat(t.truncatingRemainder(dividingBy: 4.2) / 4.2)
        let blink: CGFloat = c < 0.94 ? 1 : (c < 0.97 ? 1 - (c - 0.94) / 0.03 * 0.9 : 0.1 + (c - 0.97) / 0.03 * 0.9)
        let eyes = scaled(ctx, 1, blink, around: CGPoint(x: 512, y: 600))
        fill(eyes, Art.eyes, Ink.navy)
        fill(eyes, Art.sparkles, Ink.white)
        if mood == .cute {
            fill(eyes, circle(CGPoint(x: LX - 12, y: LY + 14), 6), Ink.white)
            fill(eyes, circle(CGPoint(x: RX - 12, y: RY + 14), 6), Ink.white)
        }
        if mood == .pogo {
            fill(ctx, oval(512 - 30, 700 - 28, 60, 56), Ink.navy)
        } else {
            fill(ctx, Art.smile, Ink.navy)
        }
    }
}

// Costume: tells the genres apart at a glance (kept while sleeping). Split by
// where it is worn, so it turns the right way.

/// Worn round the body: stays put when it turns.
private func drawBodyDecor(_ ctx: GraphicsContext, _ mood: Mood, _ hue: Double) {
    switch mood {
    case .mosh:
        // studded collar
        fill(ctx, roundRect(262, 744, 500, 40, 8), Ink.leather)
        fill(ctx, Art.spikes, Ink.steel)
        for x in Art.spikeXList { fill(ctx, circle(CGPoint(x: x, y: 764), 7), Ink.steel) }
    case .pogo:
        // safety pin
        let g = rotated(ctx, -20, around: CGPoint(x: 330, y: 760))
        stroke(g, Art.pin, Ink.silver, Pen.w7)
        stroke(g, circle(CGPoint(x: 298, y: 760), 8), Ink.silver, Pen.w7)
    case .party:
        // glow-stick necklace
        stroke(ctx, Art.glowNecklace, Art.uvPink.at(hue), Pen.w14)
    case .groove:
        // gold chain + medallion
        stroke(ctx, Art.chain, Ink.gold, Pen.chain)
        fill(ctx, circle(CGPoint(x: 512, y: 790), 28), Ink.gold)
        stroke(ctx, circle(CGPoint(x: 512, y: 790), 28), Ink.bronze, Pen.w5)
        stroke(ctx, Art.medalNote, Ink.bronze, Pen.w5)
    default:
        break
    }
}

/// On top of the head: turns with the face, a little less.
private func drawTopDecor(_ ctx: GraphicsContext, _ mood: Mood) {
    switch mood {
    case .mosh:
        // devil horns
        fill(ctx, Art.hornL, Ink.darkRed)
        stroke(ctx, Art.hornL, Ink.maroon, Pen.w6)
        fill(ctx, Art.hornR, Ink.darkRed)
        stroke(ctx, Art.hornR, Ink.maroon, Pen.w6)
    case .pogo:
        fill(ctx, Art.mohawk, Ink.green)
        stroke(ctx, Art.mohawk, Ink.darkGreen, Pen.w6)
    case .cute:
        let g = rotated(ctx, 14, around: CGPoint(x: 640, y: 372))
        fill(g, Art.bowL, Ink.pink)
        fill(g, Art.bowR, Ink.pink)
        fill(g, circle(CGPoint(x: 640, y: 372), 18), Ink.hotPink)
    case .salsa:
        // hibiscus
        let flower = CGPoint(x: 372, y: 396)
        for a in 0..<5 {
            let g = rotated(ctx, CGFloat(a) * 72, around: flower)
            fill(g, oval(flower.x - 24, flower.y - 34 - 38, 48, 76), Ink.red)
        }
        fill(ctx, circle(flower, 14), Ink.yellow)
    case .groove:
        // backwards cap
        fill(ctx, Art.capCrown, Ink.darkRed)
        fill(ctx, Art.capBrim, Ink.brick)
        fill(ctx, circle(CGPoint(x: 512, y: 114), 10), Ink.brick)
    case .chill:
        // beret
        let g = rotated(ctx, -8, around: CGPoint(x: 530, y: 214))
        fill(g, oval(530 - 150, 214 - 44, 300, 88), Ink.beret)
        fill(ctx, roundRect(522, 156, 14, 22, 6), Ink.beret)
    default:
        break
    }
}

/// On the face: slides round the head with the eyes.
private func drawFaceDecor(_ ctx: GraphicsContext, _ mood: Mood) {
    switch mood {
    case .cute:
        stroke(ctx, Art.lashes, Ink.navy, Pen.w7)
    case .salsa:
        // red lips
        fill(ctx, Art.smile, Ink.darkRed)
    case .chill:
        // round glasses and a bow tie
        stroke(ctx, circle(CGPoint(x: LX, y: LY), 50), Ink.navy, Pen.plain8)
        stroke(ctx, circle(CGPoint(x: RX, y: RY), 50), Ink.navy, Pen.plain8)
        stroke(ctx, Art.glassesBridge, Ink.navy, Pen.plain8)
        fill(ctx, Art.bowTie, Ink.crimson)
        fill(ctx, circle(CGPoint(x: 512, y: 784), 12), Ink.maroon)
    default:
        break
    }
}
