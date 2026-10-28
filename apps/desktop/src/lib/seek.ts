// Click or drag on any groove to seek (the retro layouts draw their own bars).
import { api } from "./api";

export interface SeekOptions {
  /** 0 turns seeking off. */
  duration: number;
  /** Where the seek goes (the app's player by default). */
  seek?: (ms: number) => void;
  /** 0..1 while dragging, `null` once released. */
  onpreview?: (fraction: number | null) => void;
}

export function seekable(node: HTMLElement, options: SeekOptions) {
  let opts = options;
  let id: number | null = null;
  const fraction = (e: PointerEvent) => {
    const r = node.getBoundingClientRect();
    return Math.min(1, Math.max(0, (e.clientX - r.left) / Math.max(1, r.width)));
  };
  const down = (e: PointerEvent) => {
    if (e.button !== 0 || !opts.duration) return;
    e.stopPropagation();
    id = e.pointerId;
    node.setPointerCapture(id);
    opts.onpreview?.(fraction(e));
  };
  const move = (e: PointerEvent) => {
    if (e.pointerId === id) opts.onpreview?.(fraction(e));
  };
  const up = (e: PointerEvent) => {
    if (e.pointerId !== id) return;
    id = null;
    (opts.seek ?? api.seek)(fraction(e) * opts.duration);
    opts.onpreview?.(null);
  };
  const cancel = () => {
    id = null;
    opts.onpreview?.(null);
  };
  node.addEventListener("pointerdown", down);
  node.addEventListener("pointermove", move);
  node.addEventListener("pointerup", up);
  node.addEventListener("pointercancel", cancel);
  node.style.cursor = "pointer";
  node.style.touchAction = "none";
  return {
    update(next: SeekOptions) {
      opts = next;
    },
    destroy() {
      node.removeEventListener("pointerdown", down);
      node.removeEventListener("pointermove", move);
      node.removeEventListener("pointerup", up);
      node.removeEventListener("pointercancel", cancel);
    },
  };
}
