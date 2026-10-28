// How the app moves (Settings → Look → Motion): the Svelte transitions every
// view, menu, dialog and list uses, and `data-motion` on <html> for the CSS ones.
//
//   full    views slide in, menus pop from their button, lists cascade in
//   subtle  short fades only, nothing travels; the looping sheens stop
//   off     everything appears at once (no transitions, no CSS animations)
//   auto    full, or subtle when the system asks for less motion
//           (macOS "Reduce motion", Windows "Animation effects" off...)
//
// The skin sets the character: the glass layouts glide with a little bounce,
// Aluminum is quick and smooth like Aqua, Luna and Web 2000 are short and
// plain like XP, Retro Amp moves in a few chunky frames like a 2000 PC.
import { backOut, cubicOut, quadOut, quintOut } from "svelte/easing";
import { flip } from "svelte/animate";
import type { TransitionConfig } from "svelte/transition";
import type { Motion, PlayerLayout } from "./api";

type Level = "full" | "subtle" | "off";
type Ease = (t: number) => number;

interface Flavor {
  /** Multiplies every duration. */
  speed: number;
  /** Things that slide or fade. */
  ease: Ease;
  /** Things that pop open (menus, dialogs). */
  pop: Ease;
  /** Multiplies every distance. */
  travel: number;
}

/** A few frames instead of a glide (Retro Amp). */
const steps =
  (n: number): Ease =>
  (t) =>
    Math.min(1, Math.ceil(t * n) / n);

const GLASS: Flavor = { speed: 1, ease: cubicOut, pop: backOut, travel: 1 };
const XP: Flavor = { speed: 0.6, ease: quadOut, pop: quadOut, travel: 0.6 };
const FLAVORS: Partial<Record<PlayerLayout, Flavor>> = {
  aluminum: { speed: 0.8, ease: cubicOut, pop: quintOut, travel: 0.8 },
  luna: XP,
  web2000: XP,
  retro_amp: { speed: 0.55, ease: steps(4), pop: steps(3), travel: 0.5 },
};

let level: Level = "full";
let flavor: Flavor = GLASS;
let chosen: Motion = "auto";
let skin: PlayerLayout = "classic";

const reduced = typeof matchMedia === "function" ? matchMedia("(prefers-reduced-motion: reduce)") : null;
// "auto" follows the system setting live
reduced?.addEventListener("change", () => applyMotion(chosen, skin));

/** Sets the motion of the whole app from the setting and the skin (App.svelte calls it on every change). */
export function applyMotion(setting: Motion | undefined, appSkin: PlayerLayout = "classic") {
  chosen = setting ?? "auto";
  skin = appSkin;
  level = chosen === "auto" ? (reduced?.matches ? "subtle" : "full") : chosen;
  flavor = FLAVORS[skin] ?? GLASS;
  document.documentElement.dataset.motion = level;
}

/** The motion in effect ("auto" resolved). */
export function motionLevel(): Level {
  return level;
}

const NONE: TransitionConfig = { duration: 0 };

/**
 * The common shape: fade, plus a move / scale in full motion. `out` plays a bit
 * faster than `in`, so closing never feels slower than opening.
 */
function moving(
  node: Element,
  o: { duration: number; delay?: number; x?: number; y?: number; scale?: number; origin?: string; pop?: boolean; plain?: boolean },
  direction: "in" | "out" | "both" = "in",
): TransitionConfig {
  if (level === "off") return NONE;
  const speed = (direction === "out" ? 0.7 : 1) * (level === "subtle" ? 0.7 : flavor.speed);
  const duration = Math.round(o.duration * speed);
  const delay = Math.round((o.delay ?? 0) * speed);
  // `plain`: a list row at rest (opaque, untransformed); reading its computed
  // style would force a style recalc of the whole page for every row
  const opacity = o.plain ? 1 : +getComputedStyle(node).opacity;
  if (level === "subtle") return { duration, delay, easing: cubicOut, css: (t) => `opacity: ${t * opacity}` };
  const x = (o.x ?? 0) * flavor.travel;
  const y = (o.y ?? 0) * flavor.travel;
  const s = 1 - (1 - (o.scale ?? 1)) * flavor.travel;
  const base = o.plain ? "none" : getComputedStyle(node).transform;
  const from = base === "none" ? "" : base;
  return {
    duration,
    delay,
    // the pop easing overshoots: only the transform gets it, opacity stays within 0..1
    easing: o.pop ? flavor.pop : flavor.ease,
    css: (t, u) =>
      `opacity: ${Math.min(1, t) * opacity};` +
      `transform: ${from} translate(${u * x}px, ${u * y}px) scale(${1 - u * (1 - s)});` +
      (o.origin ? `transform-origin: ${o.origin};` : ""),
  };
}

/** A view arriving (Library, Settings, the room...): rises a little as it fades in. */
export function page(node: Element, { delay = 0 }: { delay?: number } = {}, opts?: { direction: "in" | "out" | "both" }) {
  return moving(node, { duration: 320, delay, y: 14, scale: 0.995 }, opts?.direction);
}

/** Content swapping in place (a settings section, a tab's list, lyrics / player). */
export function rise(node: Element, { y = 10, duration = 260, delay = 0 }: { y?: number; duration?: number; delay?: number } = {}, opts?: { direction: "in" | "out" | "both" }) {
  return moving(node, { duration, delay, y }, opts?.direction);
}

/** Plain fade (backdrops, scrims, small labels). */
export function fade(node: Element, { duration = 180, delay = 0 }: { duration?: number; delay?: number } = {}, opts?: { direction: "in" | "out" | "both" }) {
  if (level === "off") return NONE;
  const speed = (opts?.direction === "out" ? 0.7 : 1) * (level === "subtle" ? 0.7 : flavor.speed);
  const opacity = +getComputedStyle(node).opacity;
  return { duration: duration * speed, delay: delay * speed, easing: cubicOut, css: (t: number) => `opacity: ${t * opacity}` };
}

/**
 * A menu or popover opening from the button that opened it: grows from
 * `origin` (a CSS transform-origin, e.g. "bottom right" for a menu above it).
 */
export function pop(node: Element, { origin = "top right", y = -6 }: { origin?: string; y?: number } = {}, opts?: { direction: "in" | "out" | "both" }) {
  return moving(node, { duration: 200, y, scale: 0.94, origin, pop: true }, opts?.direction);
}

/** A dialog: scales up from the middle of the window. */
export function dialog(node: Element, _: object = {}, opts?: { direction: "in" | "out" | "both" }) {
  return moving(node, { duration: 260, y: 12, scale: 0.95, pop: true }, opts?.direction);
}

/** A side panel (the queue): slides in from the edge it lives on. */
export function drawer(node: Element, { x = 48 }: { x?: number } = {}, opts?: { direction: "in" | "out" | "both" }) {
  return moving(node, { duration: 300, x }, opts?.direction);
}

/** A floating note, a banner: slides in from the side it sits on. */
export function toast(node: Element, { x = 40, y = 0 }: { x?: number; y?: number } = {}, opts?: { direction: "in" | "out" | "both" }) {
  return moving(node, { duration: 280, x, y, scale: 0.97, pop: true }, opts?.direction);
}

/** The song changed: the new cover / title glides in from the side the music went. */
export function swap(node: Element, { x = 18, delay = 0 }: { x?: number; delay?: number } = {}) {
  return moving(node, { duration: 420, x, delay, scale: 0.985 });
}

// Lists cascade in: rows and cards that arrive together (a view opening, the
// next page of a lazy list, a search) start one after another, from the first
// of that batch. Only the first STAGGER of a batch wait for their turn, the rest
// come with the last of them, and past MOST nothing animates (off screen
// anyway), so a list of thousands costs nothing.
const STAGGER = 12;
const MOST = 48;
/** A batch that took longer than this to build skips the rest of its entrances (a slow machine is busy enough). */
const BUDGET_MS = 12;
let batch = 0;
let closing = false;
let batchStart = 0;

/**
 * A row or card of a list; use as `in:item|global` so the first load cascades too.
 * `skip`: appear at once (rows that a scroll brings in, a virtual list's recycling).
 */
export function item(node: Element, { step = 28, skip = false }: { step?: number; skip?: boolean } = {}) {
  if (level === "off" || skip) return NONE;
  const n = batch++;
  if (!closing) {
    // everything created in this update is one batch
    closing = true;
    batchStart = performance.now();
    setTimeout(() => {
      batch = 0;
      closing = false;
    });
  }
  if (n >= MOST || performance.now() - batchStart > BUDGET_MS) return NONE;
  return moving(node, { duration: 300, delay: Math.min(n, STAGGER) * step, y: 10, scale: 0.985, plain: true });
}

/** Siblings sliding into the gap a removed item left (`animate:reflow` in a keyed each). */
export function reflow(node: Element, rects: { from: DOMRect; to: DOMRect }) {
  if (level !== "full") return NONE;
  return flip(node, rects, { duration: 260 * flavor.speed, easing: flavor.ease });
}
