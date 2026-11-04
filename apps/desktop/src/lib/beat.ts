// Follows the real beat of the music so the mascot can headbang on it. Pure
// math with no DOM and no imports (it runs under `node --test`, see
// `tests/beat.test.ts`), and the Android app keeps the same numbers
// (`ui/player/BeatFollower.kt`).
//
//   OnsetDetector  the spectrum, ~30 times a second → "something hit" and how hard
//   BeatFollower   the hits → a beat that keeps ticking between them (a phase
//                  lock), how sure we are about it, how loud this moment is
//                  compared with the last seconds, and the drops

export type Section = "quiet" | "groove" | "loud";

/** What the choreography reads; `BeatFollower` is one. Never written by the reader. */
export interface BeatView {
  /** Tempo the beat advances at (beats per minute). */
  readonly bpm: number;
  /** Beats since the start; the fraction is the phase, 0 is a beat. */
  readonly beat: number;
  /** 0..1: how many of the last hits landed on the beat grid. */
  readonly confidence: number;
  /** 0..1: the last ~300 ms against the last ~8 s. */
  readonly intensity: number;
  readonly section: Section;
  /** Which beat (0..3, counted like `Math.round(beat) & 3`) starts a bar. */
  readonly downbeat: number;
  /** How many drops have happened; changes once per drop. */
  readonly drops: number;
  /** How many strong hits (accents) have happened; changes once per accent. */
  readonly accents: number;
}

export const BEAT = {
  /** Folded tempo range of the estimate, so half/double-time hits agree. */
  minBpm: 80,
  maxBpm: 160,
  /** Hits further than this (in beats) from the grid are ignored. */
  window: 0.3,
  /** Hits this close count as "on the grid" for the confidence. */
  hit: 0.15,
  phaseGain: 0.2,
  tempoGain: 0.05,
  /** Below this the mascot free-runs, with no hard hits. */
  minConfidence: 0.35,
  /** How fast the confidence follows (per hit). */
  confidenceGain: 0.15,
  /** Seconds without a hit before the confidence starts to fade, and how fast (per second). */
  silence: 1,
  silenceFade: 0.25,
  /** Time constants of the loudness, seconds. */
  shortTau: 0.3,
  longTau: 8,
  /** Loudness under which the intensity fades to nothing. */
  silentLevel: 0.1,
  quiet: 0.25,
  loud: 0.7,
  /** Hysteresis of the section borders. */
  margin: 0.03,
  /** Intensity jump within a bar that makes a drop, after `quietBars` quieter bars. */
  dropJump: 0.35,
  quietBars: 2,
  quietLevel: 0.45,
  /** The intensity is sampled this often (seconds) to see the jump of a drop within the last bar. */
  sampleEvery: 0.1,
  /** At most one drop per this many bars. */
  dropEvery: 8,
  /** A hit this strong is an accent. */
  accent: 0.8,
  beatsPerBar: 4,
} as const;

const clamp01 = (v: number) => Math.min(1, Math.max(0, v));

/**
 * Finds hits in the spectrum: a rise of the low band (the kick) plus a little
 * of the mids, above an adaptive threshold, with a short rest between hits.
 */
export class OnsetDetector {
  /** Bands counted as "low" (the kick) in the spectrum. */
  static readonly LOW: readonly [number, number] = [1, 6];
  /** A hit needs a rise of at least this, and this many times the usual rise. */
  static readonly MIN_FLUX = 0.04;
  static readonly RATIO = 2.2;
  static readonly AVG_RATE = 0.1;
  static readonly REST = 0.25;
  /** The loudest rise seen decays by this each frame, never under `PEAK_FLOOR`. */
  static readonly PEAK_DECAY = 0.995;
  static readonly PEAK_FLOOR = 0.1;
  private lowPrev = 0;
  private fluxAvg = 0;
  private peak = OnsetDetector.PEAK_FLOOR;
  private sinceHit = 1;
  /** The first spectrum after a reset is only a starting point, not a hit. */
  private primed = false;

  reset() {
    this.lowPrev = this.fluxAvg = 0;
    this.peak = OnsetDetector.PEAK_FLOOR;
    this.sinceHit = 1;
    this.primed = false;
  }

  private static mean(spectrum: ArrayLike<number>, [from, to]: readonly [number, number]) {
    const end = Math.min(to, spectrum.length);
    let sum = 0;
    for (let i = from; i < end; i++) sum += spectrum[i];
    return end > from ? sum / (end - from) : 0;
  }

  /** Feeds one spectrum `dt` seconds after the last one; returns the hit's strength (0.05..1), or 0. */
  push(spectrum: ArrayLike<number>, dt: number): number {
    const low = OnsetDetector.mean(spectrum, OnsetDetector.LOW);
    const flux = this.primed ? Math.max(0, low - this.lowPrev) : 0;
    this.lowPrev = low;
    this.primed = true;
    this.sinceHit += dt;
    this.peak = Math.max(OnsetDetector.PEAK_FLOOR, this.peak * OnsetDetector.PEAK_DECAY, flux);
    const usual = this.fluxAvg;
    this.fluxAvg += (flux - this.fluxAvg) * OnsetDetector.AVG_RATE;
    if (flux < OnsetDetector.MIN_FLUX || flux < OnsetDetector.RATIO * usual || this.sinceHit < OnsetDetector.REST) return 0;
    this.sinceHit = 0;
    return Math.min(1, Math.max(0.05, flux / this.peak));
  }
}

export class BeatFollower implements BeatView {
  bpm = 120;
  beat = 0;
  confidence = 0;
  intensity = 0;
  section: Section = "groove";
  downbeat = 0;
  drops = 0;
  accents = 0;
  /** Tempo estimated from the gaps between hits (it only moves on real changes). */
  estimate = 120;

  private now = 0;
  private lastHit = -Infinity;
  private sinceHit = 0;
  private gaps: number[] = [];
  private sortScratch: number[] = [];
  /** How much of the hits landed on each beat of the bar, to find the downbeat. */
  private accentSum = [0, 0, 0, 0];
  private short = 0;
  private long = 0;
  private seeded = false;
  /** Intensity every `sampleEvery` seconds, to see the jump of a drop within the last bar. */
  private ring = new Array<number>(40).fill(0);
  private sampleClock = 0;
  private sampleAt = 0;
  private beatIndex = 0;
  private barIndex = 0;
  private barSum = 0;
  private barCount = 0;
  private quietRun = 0;
  private barsSinceDrop: number = BEAT.dropEvery;

  /** Forgets the previous track. */
  reset() {
    this.beat = 0;
    this.confidence = 0;
    this.intensity = 0;
    this.section = "groove";
    this.downbeat = 0;
    this.estimate = this.bpm = 120;
    this.now = this.sinceHit = 0;
    this.lastHit = -Infinity;
    this.gaps.length = 0;
    this.accentSum.fill(0);
    this.short = this.long = 0;
    this.seeded = false;
    this.ring.fill(0);
    this.beatIndex = 0;
    this.barIndex = 0;
    this.barSum = this.barCount = this.quietRun = 0;
    this.sampleClock = this.sampleAt = 0;
    this.barsSinceDrop = BEAT.dropEvery;
  }

  /** Moves time forward by `dt` seconds (call it every frame). */
  advance(dt: number) {
    this.now += dt;
    this.sinceHit += dt;
    this.beat += (dt * this.bpm) / 60;
    if (this.sinceHit > BEAT.silence) this.confidence = Math.max(0, this.confidence - BEAT.silenceFade * dt);
    const index = Math.floor(this.beat);
    if (index > this.beatIndex) {
      this.beatIndex = index;
      this.onBeat();
    }
  }

  /** A hit of `strength` (0..1) happened just now. */
  onset(strength: number) {
    const gap = this.now - this.lastHit;
    this.lastHit = this.now;
    this.sinceHit = 0;
    if (gap >= 0.25 && gap <= 1.5) this.fold(gap);

    const nearest = Math.round(this.beat);
    const error = this.beat - nearest; // > 0: the grid is ahead of the music
    if (Math.abs(error) <= BEAT.window) {
      this.beat -= BEAT.phaseGain * error;
      this.bpm = Math.min(200, Math.max(60, this.bpm * (1 - BEAT.tempoGain * error)));
      this.hit(Math.abs(error) <= BEAT.hit);
      const bar = ((nearest % 4) + 4) % 4;
      for (let i = 0; i < 4; i++) this.accentSum[i] *= 0.98;
      this.accentSum[bar] += strength;
      this.pickDownbeat();
      if (strength > BEAT.accent && this.confidence >= BEAT.minConfidence) this.accents++;
    } else {
      this.hit(false);
      this.resync();
    }
  }

  /** The loudness (0..1) `dt` seconds after the last call: how loud now is against the recent past. */
  loudness(level: number, dt: number) {
    if (!this.seeded) {
      this.seeded = true;
      this.short = this.long = level;
    }
    this.short += (level - this.short) * (1 - Math.exp(-dt / BEAT.shortTau));
    this.long += (level - this.long) * (1 - Math.exp(-dt / BEAT.longTau));
    // above 0.6 times its recent self counts, and almost nothing at all is quiet whatever the ratio
    this.intensity = clamp01((this.short / Math.max(this.long, 0.02) - 0.6) / 1.0) * Math.min(1, this.short / BEAT.silentLevel);
    const m = BEAT.margin;
    const low = this.section === "quiet" ? BEAT.quiet + m : BEAT.quiet - m;
    const high = this.section === "loud" ? BEAT.loud - m : BEAT.loud + m;
    this.section = this.intensity < low ? "quiet" : this.intensity > high ? "loud" : "groove";
    this.sample(dt);
    this.checkDrop();
  }

  private hit(onGrid: boolean) {
    this.confidence += ((onGrid ? 1 : 0) - this.confidence) * BEAT.confidenceGain;
  }

  /** Folds a gap between hits into the tempo estimate (median of the last 16). */
  private fold(gap: number) {
    let bpm = 60 / gap;
    while (bpm < BEAT.minBpm) bpm *= 2;
    while (bpm > BEAT.maxBpm) bpm /= 2;
    this.gaps.push(bpm);
    if (this.gaps.length > 16) this.gaps.shift();
    if (this.gaps.length < 6 || this.confidence >= BEAT.minConfidence) return; // the seed only moves while unlocked
    this.sortScratch.length = 0;
    for (const g of this.gaps) this.sortScratch.push(g);
    this.sortScratch.sort((a, b) => a - b);
    const median = Math.round(this.sortScratch[this.sortScratch.length >> 1]);
    if (Math.abs(median - this.estimate) / this.estimate > 0.05) this.estimate = median;
  }

  /**
   * Lost the grid (tempo change, a new song): take the estimate and sit the
   * beat on this hit. Needs the estimate to differ, otherwise it is only noise.
   */
  private resync() {
    if (this.confidence >= BEAT.minConfidence || this.gaps.length < 6) return;
    if (Math.abs(this.estimate - this.bpm) / this.bpm <= 0.05 && this.confidence > 0.1) return;
    this.bpm = this.estimate;
    this.beat = Math.round(this.beat);
    this.confidence = BEAT.minConfidence + 0.02;
  }

  /** The bar starts on the beat that collects the strongest hits (only moved when clearly beaten). */
  private pickDownbeat() {
    let best = this.downbeat;
    for (let i = 0; i < 4; i++) if (this.accentSum[i] > this.accentSum[best] * 1.3) best = i;
    this.downbeat = best;
  }

  /** Once per beat: remember the intensity, count bars. */
  private onBeat() {
    this.barSum += this.intensity;
    this.barCount++;
    if (((this.beatIndex - this.downbeat) & 3) !== 0) return;
    // a new bar: was the last one a quiet one?
    const count = Math.max(1, this.barCount);
    const quiet = this.barSum / count < BEAT.quietLevel;
    this.quietRun = quiet ? this.quietRun + 1 : 0;
    this.barSum = this.barCount = 0;
    this.barIndex++;
    this.barsSinceDrop++;
  }

  private sample(dt: number) {
    this.sampleClock += dt;
    while (this.sampleClock >= BEAT.sampleEvery) {
      this.sampleClock -= BEAT.sampleEvery;
      this.ring[this.sampleAt % this.ring.length] = this.intensity;
      this.sampleAt++;
    }
  }

  /** A drop: the intensity leaps within one bar after a quiet stretch. At most one per eight bars. */
  private checkDrop() {
    if (this.quietRun < BEAT.quietBars || this.barsSinceDrop < BEAT.dropEvery) return;
    // the last bar's worth of samples (a bar is 240 / bpm seconds, at most the whole ring)
    const n = Math.min(this.ring.length, Math.max(4, Math.round(240 / this.bpm / BEAT.sampleEvery)));
    let before = 1;
    for (let i = 1; i <= n; i++) before = Math.min(before, this.ring[(this.sampleAt - i + 4 * this.ring.length) % this.ring.length]);
    if (this.intensity - before <= BEAT.dropJump) return;
    this.drops++;
    this.barsSinceDrop = 0;
    this.quietRun = 0;
  }
}
