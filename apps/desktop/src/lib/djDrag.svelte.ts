// The one set of mouse gestures every DJ knob and fader shares (docs/DJ_UI.md §6):
// drag (160 px = full travel; vertical, or horizontal for horizontal faders), Shift = fine (x 0.1),
// wheel = 1/40 of the range (Shift 1/200), double-click = reset, click on a fader's track = jump
// there, and the keys of a slider (arrows, Shift+arrows = big step, PageUp/PageDown, Home = reset).

/** Pixels of drag for the whole range. */
const TRAVEL_PX = 160;
const FINE = 0.1;
const WHEEL_STEP = 1 / 40;
const WHEEL_FINE_STEP = 1 / 200;
const KEY_STEP = 1 / 40;
const KEY_BIG_STEP = 1 / 8;

export interface DragOptions {
  value: number;
  min: number;
  max: number;
  /** Double-click and Home put it back here. */
  reset: number;
  onchange: (value: number) => void;
  /** The gesture ended (pointer up, wheel pause): the screen may show the engine's value again. */
  ondone?: () => void;
  horizontal?: boolean;
  /** Faders: a press on the track (outside the cap) jumps to that spot first. */
  track?: boolean;
  disabled?: boolean;
}

/** Svelte action: `use:dragValue={{ value, min, max, reset, onchange }}`. */
export function dragValue(node: HTMLElement, initial: DragOptions) {
  let o = initial;
  let start: { at: number; value: number; fine: boolean } | null = null;
  let wheelTimer: ReturnType<typeof setTimeout> | undefined;
  const span = () => o.max - o.min;
  const clamp = (v: number) => Math.min(o.max, Math.max(o.min, v));
  const set = (v: number) => o.onchange(clamp(v));
  const coord = (e: PointerEvent) => (o.horizontal ? e.clientX : -e.clientY);

  /** The value under the pointer, along the track. */
  function valueAt(e: PointerEvent): number {
    // a fader measures along its cap's travel (inset from the ends), anything else its own box
    const r = (node.querySelector("[data-travel]") ?? node).getBoundingClientRect();
    const t = o.horizontal ? (e.clientX - r.left) / r.width : 1 - (e.clientY - r.top) / r.height;
    return clamp(o.min + Math.min(1, Math.max(0, t)) * span());
  }

  function down(e: PointerEvent) {
    if (o.disabled || e.button !== 0) return;
    node.setPointerCapture(e.pointerId);
    node.focus({ preventScroll: true });
    let value = o.value;
    const onCap = (e.target as HTMLElement).closest("[data-cap]");
    if (o.track && !onCap) {
      value = valueAt(e);
      set(value);
    }
    start = { at: coord(e), value, fine: e.shiftKey };
  }
  function move(e: PointerEvent) {
    if (!start) return;
    // Shift pressed or released mid-drag: restart from here so the value doesn't jump
    if (e.shiftKey !== start.fine) start = { at: coord(e), value: o.value, fine: e.shiftKey };
    const scale = (start.fine ? FINE : 1) * (span() / TRAVEL_PX);
    set(start.value + (coord(e) - start.at) * scale);
  }
  function up() {
    if (!start) return;
    start = null;
    o.ondone?.();
  }
  function wheel(e: WheelEvent) {
    if (o.disabled) return;
    e.preventDefault();
    const delta = Math.abs(e.deltaY) >= Math.abs(e.deltaX) ? -e.deltaY : e.deltaX;
    if (!delta) return;
    set(o.value + Math.sign(delta) * span() * (e.shiftKey ? WHEEL_FINE_STEP : WHEEL_STEP));
    clearTimeout(wheelTimer);
    wheelTimer = setTimeout(() => o.ondone?.(), 250);
  }
  function dbl() {
    if (o.disabled) return;
    set(o.reset);
    o.ondone?.();
  }
  function key(e: KeyboardEvent) {
    if (o.disabled) return;
    const step = span() * (e.shiftKey ? KEY_BIG_STEP : KEY_STEP);
    const moves: Record<string, number> = { ArrowUp: step, ArrowRight: step, ArrowDown: -step, ArrowLeft: -step, PageUp: span() * KEY_BIG_STEP, PageDown: -span() * KEY_BIG_STEP };
    if (e.key in moves) set(o.value + moves[e.key]);
    else if (e.key === "Home") set(o.reset);
    else return;
    e.preventDefault();
    e.stopPropagation();
    o.ondone?.();
  }

  node.addEventListener("pointerdown", down);
  node.addEventListener("pointermove", move);
  node.addEventListener("pointerup", up);
  node.addEventListener("pointercancel", up);
  node.addEventListener("wheel", wheel, { passive: false });
  node.addEventListener("dblclick", dbl);
  node.addEventListener("keydown", key);
  return {
    update(next: DragOptions) {
      o = next;
    },
    destroy() {
      clearTimeout(wheelTimer);
      node.removeEventListener("pointerdown", down);
      node.removeEventListener("pointermove", move);
      node.removeEventListener("pointerup", up);
      node.removeEventListener("pointercancel", up);
      node.removeEventListener("wheel", wheel);
      node.removeEventListener("dblclick", dbl);
      node.removeEventListener("keydown", key);
    },
  };
}

/** A value the user is moving: shown at once, while the engine's echo catches up. */
export class Held {
  value = $state<number | null>(null);
  private timer: ReturnType<typeof setTimeout> | undefined;
  set(v: number) {
    clearTimeout(this.timer);
    this.value = v;
  }
  /** Back to the engine's value a moment after the gesture (its next snapshots carry it). */
  release() {
    clearTimeout(this.timer);
    this.timer = setTimeout(() => (this.value = null), 300);
  }
}
