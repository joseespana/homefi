// Where the mascot lives: in the room's rack, or floating wherever the user
// dragged it. The drag outlives the component it started on (the rack copy
// disappears as soon as the mascot floats), so it is tracked here.

import type { RoomSettings } from "./api";
import { MOODS, moodFor, type Mood } from "./mood";
import { app } from "./state.svelte";
import { visual } from "./visual.svelte";

class MascotPlace {
  /** Centre of the floating mascot, as fractions of the window. */
  x = $state(0.85);
  y = $state(0.75);
  dragging = $state(false);
}

export const place = new MascotPlace();

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

function saveRoom(patch: Partial<RoomSettings>) {
  const room = app.settings?.room;
  if (room) app.saveSettings({ room: { ...room, ...patch } });
}

/** The dance style for what is playing now (the setting, or from the genre). */
export function roomMood(): Mood {
  const room = app.settings?.room;
  const track = app.current;
  if (room && room.mascot_style !== "auto") return room.mascot_style as Mood;
  return moodFor(track?.genre, `${track?.title ?? ""} ${track?.album ?? ""}`, { energy: visual.trackEnergy, bpm: visual.bpm, heard: visual.heard });
}

/** Right-click: Auto → each style in turn → Auto. Returns the new style's name. */
export function nextStyle(): string {
  const order = ["auto", ...MOODS.map((m) => m.id)];
  const room = app.settings?.room;
  const next = order[(order.indexOf(room?.mascot_style ?? "auto") + 1) % order.length];
  saveRoom({ mascot_style: next });
  return MOODS.find((m) => m.id === next)?.label ?? "Auto";
}

/**
 * Starts tracking a press on the mascot: a click calls `onclick`, a drag
 * moves it (and lifts it out of the room the first time).
 */
export function grab(e: PointerEvent, el: HTMLElement, onclick: () => void) {
  if (e.button !== 0) return;
  const r = el.getBoundingClientRect();
  const offX = e.clientX - (r.left + r.width / 2);
  const offY = e.clientY - (r.top + r.height / 2);
  const startX = e.clientX;
  const startY = e.clientY;
  let moved = false;

  const move = (ev: PointerEvent) => {
    if (!moved) {
      if (Math.hypot(ev.clientX - startX, ev.clientY - startY) < 5) return;
      moved = true;
      place.dragging = true;
      const room = app.settings?.room;
      if (room && !room.mascot_free) {
        // keep the size it had in the rack
        saveRoom({ mascot_free: true, mascot_size: room.mascot_size || Math.round(r.width) });
      }
    }
    place.x = clamp((ev.clientX - offX) / innerWidth, 0.02, 0.98);
    place.y = clamp((ev.clientY - offY) / innerHeight, 0.04, 0.98);
  };
  const up = () => {
    removeEventListener("pointermove", move);
    removeEventListener("pointerup", up);
    removeEventListener("pointercancel", up);
    if (moved) {
      place.dragging = false;
      saveRoom({ mascot_free: true, mascot_x: place.x, mascot_y: place.y });
    } else onclick();
  };
  addEventListener("pointermove", move);
  addEventListener("pointerup", up);
  addEventListener("pointercancel", up);
  e.preventDefault();
}

/** Puts the mascot back in the room's rack. */
export function dock() {
  saveRoom({ mascot_free: false });
}

let resizeTimer = 0;
/** Mouse wheel over the floating mascot: bigger / smaller. */
export function resize(e: WheelEvent, current: number) {
  e.preventDefault();
  const size = Math.round(clamp(current * (e.deltaY < 0 ? 1.08 : 1 / 1.08), 80, 520));
  const room = app.settings?.room;
  if (!room) return;
  // show it right away, store it once the wheel stops
  app.settings = { ...app.settings!, room: { ...room, mascot_size: size } };
  clearTimeout(resizeTimer);
  resizeTimer = window.setTimeout(() => saveRoom({ mascot_size: size }), 400);
}
