import SwiftUI

// A small SVG path-data parser for the mascot's art (the same strings as the
// desktop's `lib/mascot.ts` and Android's `Mascot.kt`). Supports M L H V C Q
// A Z, absolute and relative; arcs become cubic Béziers.

extension Path {
    init(svg d: String) {
        self.init()
        var p = SVGPathParser(d)
        p.build(into: &self)
    }
}

private struct SVGPathParser {
    private let chars: [Character]
    private var i = 0

    init(_ d: String) { chars = Array(d) }

    private mutating func skipSeparators() {
        while i < chars.count, chars[i] == " " || chars[i] == "," || chars[i] == "\n" || chars[i] == "\t" { i += 1 }
    }

    /// The next token is a number (not a command letter).
    private mutating func atNumber() -> Bool {
        skipSeparators()
        guard i < chars.count else { return false }
        let c = chars[i]
        return c.isNumber || c == "-" || c == "+" || c == "."
    }

    private mutating func number() -> CGFloat {
        skipSeparators()
        let start = i
        if i < chars.count, chars[i] == "-" || chars[i] == "+" { i += 1 }
        var seenDot = false
        while i < chars.count {
            let c = chars[i]
            if c.isNumber {
                i += 1
            } else if c == ".", !seenDot {
                seenDot = true
                i += 1
            } else if c == "e" || c == "E" {
                i += 1
                if i < chars.count, chars[i] == "-" || chars[i] == "+" { i += 1 }
            } else {
                break
            }
        }
        return CGFloat(Double(String(chars[start..<i])) ?? 0)
    }

    /// Arc flags may be written without separators ("0 1 0" or "010").
    private mutating func flag() -> Bool {
        skipSeparators()
        guard i < chars.count else { return false }
        let c = chars[i]
        i += 1
        return c == "1"
    }

    mutating func build(into path: inout Path) {
        var current = CGPoint.zero
        var start = CGPoint.zero
        var command: Character = "M"
        while true {
            skipSeparators()
            guard i < chars.count else { break }
            if chars[i].isLetter {
                command = chars[i]
                i += 1
            }
            let rel = command.isLowercase
            let base = rel ? current : .zero
            switch command.uppercased() {
            case "M":
                current = CGPoint(x: base.x + number(), y: base.y + number())
                start = current
                path.move(to: current)
                // further pairs are implicit line-tos
                command = rel ? "l" : "L"
            case "L":
                current = CGPoint(x: base.x + number(), y: base.y + number())
                path.addLine(to: current)
            case "H":
                current = CGPoint(x: (rel ? current.x : 0) + number(), y: current.y)
                path.addLine(to: current)
            case "V":
                current = CGPoint(x: current.x, y: (rel ? current.y : 0) + number())
                path.addLine(to: current)
            case "C":
                let c1 = CGPoint(x: base.x + number(), y: base.y + number())
                let c2 = CGPoint(x: base.x + number(), y: base.y + number())
                current = CGPoint(x: base.x + number(), y: base.y + number())
                path.addCurve(to: current, control1: c1, control2: c2)
            case "Q":
                let c = CGPoint(x: base.x + number(), y: base.y + number())
                current = CGPoint(x: base.x + number(), y: base.y + number())
                path.addQuadCurve(to: current, control: c)
            case "A":
                let rx = number()
                let ry = number()
                let angle = number()
                let large = flag()
                let sweep = flag()
                let end = CGPoint(x: base.x + number(), y: base.y + number())
                addArc(&path, from: current, to: end, rx: rx, ry: ry, angle: angle, large: large, sweep: sweep)
                current = end
            case "Z":
                path.closeSubpath()
                current = start
                // numbers right after Z start a new subpath (never loop on Z)
                command = rel ? "m" : "M"
                continue
            default:
                // unknown command: stop rather than loop forever
                return
            }
            // a command letter followed by no numbers would not advance
            if !atNumber(), i < chars.count, !chars[i].isLetter { i += 1 }
        }
    }

    /// SVG endpoint arc to cubic Béziers (SVG spec, appendix F.6).
    private func addArc(_ path: inout Path, from p0: CGPoint, to p1: CGPoint, rx rx0: CGFloat, ry ry0: CGFloat, angle: CGFloat, large: Bool, sweep: Bool) {
        if p0 == p1 { return }
        var rx = abs(rx0)
        var ry = abs(ry0)
        if rx == 0 || ry == 0 {
            path.addLine(to: p1)
            return
        }
        let phi = angle * .pi / 180
        let cosPhi = cos(phi)
        let sinPhi = sin(phi)
        let dx = (p0.x - p1.x) / 2
        let dy = (p0.y - p1.y) / 2
        let x1p = cosPhi * dx + sinPhi * dy
        let y1p = -sinPhi * dx + cosPhi * dy
        // scale up radii that are too small
        let lambda = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry)
        if lambda > 1 {
            let s = sqrt(lambda)
            rx *= s
            ry *= s
        }
        let num = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p
        let den = rx * rx * y1p * y1p + ry * ry * x1p * x1p
        var coef = sqrt(max(0, num / den))
        if large == sweep { coef = -coef }
        let cxp = coef * (rx * y1p / ry)
        let cyp = coef * -(ry * x1p / rx)
        let cx = cosPhi * cxp - sinPhi * cyp + (p0.x + p1.x) / 2
        let cy = sinPhi * cxp + cosPhi * cyp + (p0.y + p1.y) / 2

        func angleOf(_ ux: CGFloat, _ uy: CGFloat, _ vx: CGFloat, _ vy: CGFloat) -> CGFloat {
            let a = atan2(ux * vy - uy * vx, ux * vx + uy * vy)
            return a
        }
        let theta1 = angleOf(1, 0, (x1p - cxp) / rx, (y1p - cyp) / ry)
        var delta = angleOf((x1p - cxp) / rx, (y1p - cyp) / ry, (-x1p - cxp) / rx, (-y1p - cyp) / ry)
        if !sweep && delta > 0 { delta -= 2 * .pi }
        if sweep && delta < 0 { delta += 2 * .pi }

        let segments = Int(ceil(abs(delta) / (.pi / 2)))
        let step = delta / CGFloat(segments)
        let t = 4.0 / 3.0 * tan(step / 4)
        func point(_ a: CGFloat) -> CGPoint {
            CGPoint(x: cx + rx * cos(a) * cosPhi - ry * sin(a) * sinPhi, y: cy + rx * cos(a) * sinPhi + ry * sin(a) * cosPhi)
        }
        func derivative(_ a: CGFloat) -> CGPoint {
            CGPoint(x: -rx * sin(a) * cosPhi - ry * cos(a) * sinPhi, y: -rx * sin(a) * sinPhi + ry * cos(a) * cosPhi)
        }
        var a = theta1
        for n in 0..<segments {
            let b = a + step
            let pa = point(a)
            let pb = n == segments - 1 ? p1 : point(b)
            let da = derivative(a)
            let db = derivative(b)
            path.addCurve(
                to: pb,
                control1: CGPoint(x: pa.x + t * da.x, y: pa.y + t * da.y),
                control2: CGPoint(x: pb.x - t * db.x, y: pb.y - t * db.y)
            )
            a = b
        }
    }
}
