// The ride's sounds: the cues the core lists in every view (3-2-1, "stand up", GO...) and the optional
// click on each pedal stroke, played with WebAudio from the WAVs the backend synthesizes. A cue is
// played once however many views list it (ids are stable); a paused ride cancels what has not sounded.
import { exerciseApi, strokePhase, type CueKind, type RideView } from "./exercise-api";

/** Sound rate asked of the backend (the context resamples). */
const CUE_RATE_HZ = 44_100;
/** Clicks are scheduled this far ahead of the newest view. */
const CLICK_HORIZON_S = 0.6;
const MS_PER_S = 1000;

export interface CueOptions {
  /** The 3-2-1 and change sounds. */
  audible: boolean;
  /** 0 to 1. */
  volume: number;
  /** A click on every pedal stroke. */
  click: boolean;
}

interface Pending {
  /** Missing while the sound is still being decoded. */
  source: AudioBufferSourceNode | null;
  startAt: number;
}

export class CuePlayer {
  #ctx: AudioContext | null = null;
  #gain: GainNode | null = null;
  #buffers = new Map<CueKind, Promise<AudioBuffer | null>>();
  /** Ids already scheduled (and whether they still wait to sound). */
  #played = new Set<number>();
  #pending = new Map<number, Pending>();
  #clickSources = new Set<AudioBufferSourceNode>();
  #lastClickAt = 0;
  /** Bumped by every stop: a sound still decoding then is dropped. */
  #epoch = 0;

  /** Created on a user gesture (Start ride), or the browser keeps it suspended. */
  unlock() {
    this.#context()?.resume().catch(() => {});
  }

  #context(): AudioContext | null {
    if (this.#ctx) return this.#ctx;
    if (typeof AudioContext === "undefined") return null;
    this.#ctx = new AudioContext();
    this.#gain = this.#ctx.createGain();
    this.#gain.connect(this.#ctx.destination);
    return this.#ctx;
  }

  #buffer(kind: CueKind): Promise<AudioBuffer | null> {
    let b = this.#buffers.get(kind);
    if (!b) {
      b = (async () => {
        const ctx = this.#context();
        if (!ctx) return null;
        try {
          return await ctx.decodeAudioData(await exerciseApi.cueWav(kind, CUE_RATE_HZ));
        } catch {
          // a missing sound is not worth stopping the ride; try again next time
          this.#buffers.delete(kind);
          return null;
        }
      })();
      this.#buffers.set(kind, b);
    }
    return b;
  }

  /** Schedules what `view` lists; call with every view. */
  onView(view: RideView, playing: boolean, o: CueOptions) {
    const ctx = this.#context();
    if (!ctx || !this.#gain) return;
    if (!playing) return this.stop();
    this.#gain.gain.value = Math.min(1, Math.max(0, o.volume));
    const arrival = ctx.currentTime;
    if (o.audible) {
      for (const cue of view.cues) {
        if (this.#played.has(cue.id)) continue;
        this.#played.add(cue.id);
        const startAt = arrival + cue.in_ms / MS_PER_S;
        this.#pending.set(cue.id, { source: null, startAt });
        this.#play(cue.kind, startAt, cue.id);
      }
    }
    if (o.click && view.pulse.stroke_ms > 0) this.#clicks(view, arrival);
  }

  #clicks(view: RideView, arrival: number) {
    const stroke = view.pulse.stroke_ms;
    const phase = strokePhase(view.pulse, view.pulse.song_position_ms);
    let at = arrival + (((1 - phase) % 1) * stroke) / MS_PER_S;
    // the stroke that is on now has sounded (or is too late to matter)
    while (at <= arrival + CLICK_HORIZON_S) {
      if (at > this.#lastClickAt + (stroke / MS_PER_S) * 0.5) {
        this.#lastClickAt = at;
        this.#play("click", at, null);
      }
      at += stroke / MS_PER_S;
    }
  }

  #play(kind: CueKind, startAt: number, id: number | null) {
    const epoch = this.#epoch;
    this.#buffer(kind).then((buffer) => {
      const ctx = this.#ctx;
      if (epoch !== this.#epoch) return;
      if (!buffer || !ctx || !this.#gain) {
        if (id !== null) {
          this.#played.delete(id);
          this.#pending.delete(id);
        }
        return;
      }
      const source = ctx.createBufferSource();
      source.buffer = buffer;
      source.connect(this.#gain);
      const when = Math.max(ctx.currentTime, startAt);
      source.onended = () => {
        source.disconnect();
        if (id !== null) this.#pending.delete(id);
        else this.#clickSources.delete(source);
      };
      source.start(when);
      const pending = id !== null ? this.#pending.get(id) : undefined;
      if (pending) pending.source = source;
      else if (id === null) this.#clickSources.add(source);
    });
  }

  /** The ride paused: what has not sounded yet is cancelled, and will be listed again on resume. */
  stop() {
    this.#epoch++;
    const now = this.#ctx?.currentTime ?? 0;
    for (const [id, p] of this.#pending) {
      if (p.startAt > now) {
        try {
          p.source?.stop();
        } catch {}
        this.#played.delete(id);
        this.#pending.delete(id);
      }
    }
    for (const s of this.#clickSources) {
      try {
        s.stop();
      } catch {}
    }
    this.#clickSources.clear();
    this.#lastClickAt = 0;
  }

  /** A new ride: every id is new again. */
  reset() {
    this.stop();
    this.#played.clear();
    this.#pending.clear();
  }

  dispose() {
    this.reset();
    this.#ctx?.close().catch(() => {});
    this.#ctx = null;
    this.#gain = null;
    this.#buffers.clear();
  }
}
