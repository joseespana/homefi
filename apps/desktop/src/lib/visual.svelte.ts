// Visualizer data loop: polls the Rust engine (~30 fps) while playing and
// applies analog-meter ballistics.
import { api } from "./api";
import { BeatFollower, OnsetDetector } from "./beat";

export const BANDS = 48;

class VisualLoop {
  /** Raw frames for canvas drawing (not reactive, read every animation frame). */
  spectrum: number[] = new Array(BANDS).fill(0);
  wave: number[] = new Array(256).fill(0);
  peaks: number[] = new Array(BANDS).fill(0);

  /** Reactive values for Svelte components. */
  vuLeft = $state(0);
  vuRight = $state(0);
  /** Smoothed loudness 0..1 (mascot dance). */
  energy = $state(0);
  /** Bass hit, for little bounces. */
  kick = $state(false);
  /** The beat of the music, phase-locked to its hits (the mascot headbangs on it). Not reactive: read it every frame. */
  readonly follower = new BeatFollower();
  /** Estimated tempo (beats per minute); the mascot dances on it. */
  bpm = $state(120);
  /** Loudness averaged over the whole track so far. */
  trackEnergy = $state(0);
  /** Seconds of audio analysed for the current track. */
  heard = $state(0);

  private running = false;
  private busy = false;
  private last = 0;
  private bassAvg = 0;
  private lastTrack = 0;
  private lastFrame = 0;
  private readonly onsets = new OnsetDetector();

  /** Forgets the tempo and energy of the previous track. */
  reset() {
    this.follower.reset();
    this.onsets.reset();
    this.bpm = this.follower.estimate;
    this.lastTrack = 0;
    this.trackEnergy = 0;
    this.heard = 0;
  }

  start() {
    if (this.running) return;
    this.running = true;
    this.lastFrame = 0;
    const tick = (t: number) => {
      if (!this.running) return;
      // the beat keeps ticking between the hits the engine reports
      if (this.lastFrame) this.follower.advance(Math.min(0.1, (t - this.lastFrame) / 1000));
      this.lastFrame = t;
      if (!this.busy && t - this.last > 32) {
        this.last = t;
        this.busy = true;
        api
          .visual(BANDS)
          .then((v) => {
            this.update(v.spectrum, v.wave, v.left, v.right);
            this.track(t);
          })
          .catch(() => {})
          .finally(() => (this.busy = false));
      }
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  }

  /** Lets meters fall back to rest once playback stops. */
  stop() {
    this.running = false;
    const fall = () => {
      if (this.running) return;
      this.update(this.spectrum.map((v) => v * 0.8), this.wave.map((v) => v * 0.8), 0, 0);
      if (this.vuLeft > 0.005 || this.vuRight > 0.005 || this.energy > 0.005) requestAnimationFrame(fall);
    };
    requestAnimationFrame(fall);
  }

  private update(spectrum: number[], wave: number[], left: number, right: number) {
    this.spectrum = spectrum;
    this.wave = wave;
    // falling peak caps (Winamp style)
    this.peaks = spectrum.map((v, i) => Math.max(v, (this.peaks[i] ?? 0) - 0.012));

    // VU ballistics: fast-ish attack, slow release, scale −30 dB .. +3 dB
    const toVu = (peak: number) => Math.min(1, Math.max(0, (20 * Math.log10(Math.max(peak, 1e-5)) + 30) / 33));
    const ballistic = (current: number, target: number) => current + (target - current) * (target > current ? 0.45 : 0.12);
    this.vuLeft = ballistic(this.vuLeft, toVu(left));
    this.vuRight = ballistic(this.vuRight, toVu(right));

    const loud = spectrum.reduce((a, b) => a + b, 0) / Math.max(1, spectrum.length);
    this.energy = this.energy + (loud - this.energy) * 0.25;
    const bass = (spectrum[2] + spectrum[3] + spectrum[4] + spectrum[5]) / 4 || 0;
    this.kick = bass > this.bassAvg * 1.18 && bass > 0.45;
    this.bassAvg = this.bassAvg * 0.9 + bass * 0.1;
  }

  /** Hits, tempo and long-term energy, from what the engine reports while playing. */
  private track(t: number) {
    const dt = this.lastTrack ? Math.min(0.1, (t - this.lastTrack) / 1000) : 0.032;
    this.lastTrack = t;
    this.heard += dt;
    this.trackEnergy += (this.energy - this.trackEnergy) / Math.min(this.heard / 0.032, 600);
    this.follower.loudness(this.energy, dt);
    const hit = this.onsets.push(this.spectrum, dt);
    if (hit > 0) this.follower.onset(hit);
    // only move on real changes, so the animation doesn't stutter
    if (this.follower.estimate !== this.bpm) this.bpm = this.follower.estimate;
  }
}

export const visual = new VisualLoop();
