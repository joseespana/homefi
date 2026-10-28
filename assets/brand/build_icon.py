#!/usr/bin/env python3
"""
Generates the HomeFi brand assets from one geometry: a happy little house
wearing headphones, with music notes coming out of it.

    python3 assets/brand/build_icon.py      # then: cargo xtask icons

Writes assets/brand/{logo,mark,wordmark}.svg and the Android vector drawables
(adaptive icon foreground / monochrome layers and the notification icon).
"""

import math
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BRAND = ROOT / "assets/brand"
DRAWABLE = ROOT / "apps/android/app/src/main/res/drawable"

# palette (see .claude/skills/ui-ux-pro-max/SKILL.md)
# brand gradient: deep blue -> teal -> green
BLUE, TEAL, GREEN = "#1D4ED8", "#0891B2", "#10B981"
NAVY, WHITE, CUSHION, CHEEK, NOTE = "#0B1E3A", "#FFFFFF", "#34D399", "#F472B6", "#FDE68A"
SHADE = "#DDEFEA"  # bottom of the house gradient


def f(v: float) -> str:
    return f"{v:.1f}".rstrip("0").rstrip(".")


def rounded_polygon(pts, r):
    """Closed polygon with circular-arc rounded corners."""
    out = []
    n = len(pts)
    for i in range(n):
        p0, p1, p2 = pts[i - 1], pts[i], pts[(i + 1) % n]
        v1 = (p0[0] - p1[0], p0[1] - p1[1])
        v2 = (p2[0] - p1[0], p2[1] - p1[1])
        l1, l2 = math.hypot(*v1), math.hypot(*v2)
        u1, u2 = (v1[0] / l1, v1[1] / l1), (v2[0] / l2, v2[1] / l2)
        ang = math.acos(max(-1.0, min(1.0, u1[0] * u2[0] + u1[1] * u2[1])))
        d = r / math.tan(ang / 2)
        a = (p1[0] + u1[0] * d, p1[1] + u1[1] * d)
        b = (p1[0] + u2[0] * d, p1[1] + u2[1] * d)
        sweep = 0 if u1[0] * u2[1] - u1[1] * u2[0] > 0 else 1
        out.append((a, b, sweep))
    s = f"M{f(out[0][1][0])} {f(out[0][1][1])}"
    for a, b, sweep in out[1:] + out[:1]:
        s += f"L{f(a[0])} {f(a[1])}A{r} {r} 0 0 {sweep} {f(b[0])} {f(b[1])}"
    return s + "Z"


def rrect(x, y, w, h, r):
    return (f"M{f(x + r)} {f(y)}H{f(x + w - r)}A{r} {r} 0 0 1 {f(x + w)} {f(y + r)}V{f(y + h - r)}"
            f"A{r} {r} 0 0 1 {f(x + w - r)} {f(y + h)}H{f(x + r)}A{r} {r} 0 0 1 {f(x)} {f(y + h - r)}"
            f"V{f(y + r)}A{r} {r} 0 0 1 {f(x + r)} {f(y)}Z")


def circle(cx, cy, r):
    return f"M{f(cx - r)} {f(cy)}A{r} {r} 0 1 0 {f(cx + r)} {f(cy)}A{r} {r} 0 1 0 {f(cx - r)} {f(cy)}Z"


def ellipse(cx, cy, rx, ry, deg):
    """Rotated ellipse as two arcs."""
    t = math.radians(deg)
    dx, dy = rx * math.cos(t), rx * math.sin(t)
    return (f"M{f(cx - dx)} {f(cy - dy)}A{rx} {ry} {deg} 1 0 {f(cx + dx)} {f(cy + dy)}"
            f"A{rx} {ry} {deg} 1 0 {f(cx - dx)} {f(cy - dy)}Z")


# ---------------------------------------------------------------- geometry
HOUSE = rounded_polygon([(512, 262), (764, 480), (764, 806), (260, 806), (260, 480)], 46)

# headphones: band over the roof, cups on the walls
BAND = "M226 560C226 128 798 128 798 560"
BAND_W = 50
CUP_L = rrect(196, 508, 92, 196, 40)
CUP_R = rrect(736, 508, 92, 196, 40)
PAD_L = rrect(262, 540, 30, 132, 15)   # cushions touching the house
PAD_R = rrect(732, 540, 30, 132, 15)

# the whole character leans a bit, like it's dancing
TILT = -6
TILT_PIVOT = (512, 560)

# face (eyes look up a little: dancing to the music)
EYE_L, EYE_R = (440, 600, 34), (584, 600, 34)
SPARKLE_L, SPARKLE_R = (452, 586, 11), (596, 586, 11)
SMILE = "M432 668Q512 760 592 668Q512 716 432 668Z"          # filled crescent
CHEEK_L, CHEEK_R = (382, 676, 28), (642, 676, 28)

# beamed eighth notes escaping from the top right
NOTE_HEADS = [ellipse(842, 318, 34, 25, -22), ellipse(950, 282, 34, 25, -22)]
NOTE_STEMS = [rrect(860, 156, 16, 160, 8), rrect(968, 120, 16, 160, 8)]
NOTE_BEAM = "M860 150L984 114L984 152L860 188Z"
# a single small note on the left for balance
NOTE_SMALL = [ellipse(126, 300, 24, 18, -22), rrect(138, 196, 12, 104, 6), "M138 190Q178 208 176 250Q166 226 150 222Z"]

NOTES = NOTE_HEADS + NOTE_STEMS + [NOTE_BEAM] + NOTE_SMALL

# sound waves coming out of the ear cups (stroked arcs)
WAVES = "M168 548Q140 606 168 664M128 520Q88 606 128 692M856 548Q884 606 856 664M896 520Q936 606 896 692"
# highlight along the head band
BAND_SHINE = "M262 470C262 210 762 210 762 470"


def face_holes():
    """Face as cut-outs (for single-color versions)."""
    return circle(*EYE_L) + circle(*EYE_R) + SMILE


def svg_logo(size=1024, rounded=True):
    rx = 232 if rounded else 0
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" width="{size}" height="{size}">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1024" y2="1024" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="{BLUE}"/>
      <stop offset="0.55" stop-color="{TEAL}"/>
      <stop offset="1" stop-color="{GREEN}"/>
    </linearGradient>
    <radialGradient id="glow" cx="420" cy="260" r="720" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#FFFFFF" stop-opacity="0.28"/>
      <stop offset="0.6" stop-color="#FFFFFF" stop-opacity="0.04"/>
      <stop offset="1" stop-color="#FFFFFF" stop-opacity="0"/>
    </radialGradient>
    <linearGradient id="vignette" x1="0" y1="560" x2="0" y2="1024" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#021B2E" stop-opacity="0"/>
      <stop offset="1" stop-color="#021B2E" stop-opacity="0.28"/>
    </linearGradient>
    <linearGradient id="house" x1="0" y1="262" x2="0" y2="806" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="{WHITE}"/>
      <stop offset="1" stop-color="{SHADE}"/>
    </linearGradient>
    <linearGradient id="band" x1="0" y1="128" x2="0" y2="560" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#173A63"/>
      <stop offset="1" stop-color="{NAVY}"/>
    </linearGradient>
    <filter id="shadow" x="-20%" y="-20%" width="140%" height="140%">
      <feDropShadow dx="0" dy="18" stdDeviation="22" flood-color="#021B2E" flood-opacity="0.38"/>
    </filter>
  </defs>
  <rect width="1024" height="1024" rx="{rx}" fill="url(#bg)"/>
  <rect width="1024" height="1024" rx="{rx}" fill="url(#glow)"/>
  <rect width="1024" height="1024" rx="{rx}" fill="url(#vignette)"/>
  <g transform="rotate({TILT} {TILT_PIVOT[0]} {TILT_PIVOT[1]})">
    <path fill="none" stroke="{WHITE}" stroke-opacity="0.75" stroke-width="22" stroke-linecap="round" d="{WAVES}"/>
    <path fill="url(#house)" filter="url(#shadow)" d="{HOUSE}"/>
    <path fill="none" stroke="url(#band)" stroke-width="{BAND_W}" stroke-linecap="round" d="{BAND}"/>
    <path fill="none" stroke="#5B86B8" stroke-opacity="0.55" stroke-width="10" stroke-linecap="round" d="{BAND_SHINE}"/>
    <path fill="{NAVY}" d="{CUP_L}{CUP_R}"/>
    <path fill="{CUSHION}" d="{PAD_L}{PAD_R}"/>
    <path fill="{NAVY}" d="{circle(*EYE_L)}{circle(*EYE_R)}{SMILE}"/>
    <path fill="{WHITE}" d="{circle(*SPARKLE_L)}{circle(*SPARKLE_R)}"/>
    <path fill="{CHEEK}" fill-opacity="0.6" d="{circle(*CHEEK_L)}{circle(*CHEEK_R)}"/>
  </g>
  <path fill="{NOTE}" d="{''.join(NOTES)}"/>
</svg>
'''


def svg_mark():
    """Single color mark (currentColor), face cut out."""
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="80 90 940 760" width="940" height="760">
  <g transform="rotate({TILT} {TILT_PIVOT[0]} {TILT_PIVOT[1]})">
    <path fill="currentColor" fill-rule="evenodd" d="{HOUSE}{face_holes()}"/>
    <path fill="none" stroke="currentColor" stroke-width="{BAND_W}" stroke-linecap="round" d="{BAND}"/>
    <path fill="currentColor" d="{CUP_L}{CUP_R}"/>
  </g>
  <path fill="currentColor" d="{''.join(NOTES)}"/>
</svg>
'''


def svg_wordmark():
    logo = svg_logo().split("\n", 1)[1].rsplit("</svg>", 1)[0]
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 2480 1024" width="2480" height="1024">
{logo}
  <text x="1120" y="640" font-family="Inter, 'SF Pro Display', 'Helvetica Neue', Arial, sans-serif" font-size="340" font-weight="800" letter-spacing="-8" fill="{NAVY}">Home<tspan fill="{TEAL}">Fi</tspan></text>
</svg>
'''


# ---------------------------------------------------------------- android
def vd_path(d, color=WHITE, even_odd=False, stroke=None):
    attrs = [f'android:pathData="{d}"']
    if stroke:
        attrs += [f'android:strokeColor="#FF{stroke[1:]}"', f'android:strokeWidth="{BAND_W}"', 'android:strokeLineCap="round"']
    else:
        attrs.append(f'android:fillColor="#FF{color[1:]}"')
    if even_odd:
        attrs.append('android:fillType="evenOdd"')
    return "        <path\n            " + "\n            ".join(attrs) + " />"


def vector(paths, scale, viewport=1024, size=108, comment="", loose=()):
    """`paths` lean with the character, `loose` (the notes) don't."""
    tilted = "\n".join("    " + p.replace("\n", "\n    ") for p in paths)
    body = f'''        <group
            android:pivotX="{TILT_PIVOT[0]}"
            android:pivotY="{TILT_PIVOT[1]}"
            android:rotation="{TILT}">
{tilted}
        </group>''' + "".join("\n" + p for p in loose)
    return f'''<?xml version="1.0" encoding="utf-8"?>
<!-- Generated by assets/brand/build_icon.py{comment}. Do not edit by hand. -->
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="{size}dp"
    android:height="{size}dp"
    android:viewportWidth="{viewport}"
    android:viewportHeight="{viewport}">
    <group
        android:pivotX="512"
        android:pivotY="512"
        android:scaleX="{scale}"
        android:scaleY="{scale}">
{body}
    </group>
</vector>
'''


def android_foreground():
    waves = (f'        <path\n            android:pathData="{WAVES}"\n            android:strokeColor="#BFFFFFFF"\n'
             f'            android:strokeWidth="22"\n            android:strokeLineCap="round" />')
    paths = [
        waves,
        vd_path(HOUSE),
        vd_path(BAND, stroke=NAVY),
        vd_path(CUP_L + CUP_R, NAVY),
        vd_path(PAD_L + PAD_R, CUSHION),
        vd_path(circle(*EYE_L) + circle(*EYE_R) + SMILE, NAVY),
        vd_path(circle(*SPARKLE_L) + circle(*SPARKLE_R), WHITE),
        vd_path(circle(*CHEEK_L) + circle(*CHEEK_R), "#F9A8D4"),
    ]
    return vector(paths, 0.62, comment=" (adaptive icon, colored layer)", loose=[vd_path("".join(NOTES), NOTE)])


def android_background():
    return f'''<?xml version="1.0" encoding="utf-8"?>
<!-- Generated by assets/brand/build_icon.py. Do not edit by hand. -->
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    xmlns:aapt="http://schemas.android.com/aapt"
    android:width="108dp"
    android:height="108dp"
    android:viewportWidth="108"
    android:viewportHeight="108">
    <path android:pathData="M0,0h108v108h-108z">
        <aapt:attr name="android:fillColor">
            <gradient android:type="linear" android:startX="0" android:startY="0" android:endX="108" android:endY="108">
                <item android:offset="0" android:color="#FF{BLUE[1:]}" />
                <item android:offset="0.55" android:color="#FF{TEAL[1:]}" />
                <item android:offset="1" android:color="#FF{GREEN[1:]}" />
            </gradient>
        </aapt:attr>
    </path>
</vector>
'''


def android_monochrome(scale=0.62, size=108, notes=True, comment=" (themed icon layer)"):
    paths = [
        vd_path(HOUSE + face_holes(), even_odd=True),
        vd_path(BAND, stroke=WHITE),
        vd_path(CUP_L + CUP_R),
    ]
    loose = [vd_path("".join(NOTES))] if notes else []
    return vector(paths, scale, size=size, comment=comment, loose=loose)


def mascot_ts():
    """Geometry for the animated mascot in the desktop app."""
    parts = {
        "house": HOUSE,
        "band": BAND,
        "cups": CUP_L + CUP_R,
        "pads": PAD_L + PAD_R,
        "eyes": circle(*EYE_L) + circle(*EYE_R),
        "sparkles": circle(*SPARKLE_L) + circle(*SPARKLE_R),
        "smile": SMILE,
        "cheeks": circle(*CHEEK_L) + circle(*CHEEK_R),
        "notes": "".join(NOTES),
    }
    body = "\n".join(f'  {k}: "{v}",' for k, v in parts.items())
    return f"""// Generated by assets/brand/build_icon.py. Do not edit by hand.
export const MASCOT = {{
{body}
}} as const;

export const MASCOT_EYES = {{ left: [{EYE_L[0]}, {EYE_L[1]}], right: [{EYE_R[0]}, {EYE_R[1]}], r: {EYE_L[2]} }} as const;
export const MASCOT_TILT = {{ deg: {TILT}, x: {TILT_PIVOT[0]}, y: {TILT_PIVOT[1]} }} as const;
export const MASCOT_BAND_WIDTH = {BAND_W};
export const MASCOT_COLORS = {{ navy: "{NAVY}", white: "{WHITE}", cushion: "{CUSHION}", cheek: "{CHEEK}", note: "{NOTE}" }} as const;
"""


def main():
    (BRAND / "logo.svg").write_text(svg_logo())
    # iPhone app icon: iOS rounds the corners itself
    (BRAND / "logo-square.svg").write_text(svg_logo(rounded=False))
    (BRAND / "mark.svg").write_text(svg_mark())
    (BRAND / "wordmark.svg").write_text(svg_wordmark())
    (BRAND / "mark.path").write_text(HOUSE + face_holes() + "\n")
    (DRAWABLE / "ic_launcher_foreground.xml").write_text(android_foreground())
    (DRAWABLE / "ic_launcher_background.xml").write_text(android_background())
    (DRAWABLE / "ic_launcher_monochrome.xml").write_text(android_monochrome())
    # notification icon: silhouette only, notes dropped so it reads at 24dp
    (DRAWABLE / "ic_stat_homefi.xml").write_text(
        android_monochrome(scale=1.25, size=24, notes=False, comment=" (notification icon)"))
    (ROOT / "apps/desktop/src/lib/mascot.ts").write_text(mascot_ts())
    (ROOT / "apps/desktop/public/logo.svg").write_text(svg_logo(64))
    print("wrote", ", ".join(p for p in ["logo.svg", "mark.svg", "wordmark.svg", "Android drawables"]))


if __name__ == "__main__":
    main()
