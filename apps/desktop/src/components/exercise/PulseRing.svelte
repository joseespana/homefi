<script lang="ts">
  // The RPM ring: the big number, and the beat pulse inside it ("pedal when the ring closes"). The pulse is
  // one Web Animation per element, its period the stroke, lined up with the song's own clock (the view's
  // position plus the pulse offset) and nudged back whenever it drifts: no state per frame. ring = the ring
  // swells and glows; bar = a bar sweeps under the digits; flash = the disc flashes (capped at .22). Motion
  // "subtle" keeps only the glow, "off" a still dot that steps once per stroke (at most twice a second).
  import { untrack } from "svelte";
  import { shownValue, unitLabel, type PulseStyle, type RideView } from "../../lib/exercise-api";
  import { motionWatch } from "../../lib/motionLevel.svelte";

  let {
    view,
    pulseStyle,
    offsetMs,
    playing,
    countdown,
    color,
    label,
  }: {
    view: RideView;
    pulseStyle: PulseStyle;
    offsetMs: number;
    playing: boolean;
    /** The 3-2-1 digit over the number, 0 when none. */
    countdown: number;
    /** The zone colour (a CSS value). */
    color: string;
    label: string;
  } = $props();

  /** No flash faster than 3 a second (docs/EXERCISE_UI.md §8): a faster stroke pulses every other stroke. */
  const MIN_PERIOD_MS = 334;
  /** The still dot steps at most twice a second. */
  const STEP_MIN_MS = 500;
  /** A phase error beyond this share of a period is put right. */
  const DRIFT = 0.1;
  const GLOW_PEAK = 0.34;
  const FLASH_PEAK = 0.22;
  const SWELL = 1.07;

  const level = $derived(motionWatch.level);
  const stroke = $derived(view.pulse.stroke_ms > 0 ? view.pulse.stroke_ms : (60_000 / Math.max(1, view.target_rpm)));
  const period = $derived(stroke * Math.max(1, Math.ceil(MIN_PERIOD_MS / stroke)));
  /** The target in the console's unit: a speed with one decimal, its cadence underneath. */
  const speed = $derived(!!view.shown && view.shown.unit !== "rpm");
  const digits = $derived(shownValue(view.shown, view.target_rpm));
  const small = $derived(digits.length > 2);

  let ring = $state<HTMLElement>();
  let glow = $state<HTMLElement>();
  let sweep = $state<HTMLElement>();
  let disc = $state<HTMLElement>();
  let anims: Animation[] = [];

  function build(): Animation[] {
    const o = { duration: period, iterations: Infinity, easing: "ease-out" } as const;
    const out: Animation[] = [];
    const swell = level === "full";
    if (pulseStyle === "ring") {
      if (swell && ring) out.push(ring.animate([{ transform: `scale(${SWELL})` }, { transform: "scale(1)", offset: 0.6 }, { transform: "scale(1)" }], o));
      if (glow) out.push(glow.animate([{ opacity: GLOW_PEAK }, { opacity: 0, offset: 0.7 }, { opacity: 0 }], o));
    } else if (pulseStyle === "bar") {
      if (sweep) out.push(sweep.animate([{ transform: "scaleX(0)", opacity: 0.7 }, { transform: "scaleX(1)", opacity: 1, offset: 0.92 }, { transform: "scaleX(1)", opacity: 0.3 }], { ...o, easing: "linear" }));
    } else if (disc) {
      out.push(disc.animate([{ opacity: FLASH_PEAK }, { opacity: 0, offset: 0.5 }, { opacity: 0 }], o));
    }
    return out;
  }

  function phaseNow(): number {
    const p = view.pulse;
    const at = p.song_position_ms + offsetMs;
    return (((at - p.first_beat_ms) / period) % 1 + 1) % 1;
  }
  function align() {
    const want = phaseNow();
    for (const a of anims) {
      const have = (((Number(a.currentTime ?? 0) / period) % 1) + 1) % 1;
      const err = Math.abs(have - want);
      if (Math.min(err, 1 - err) > DRIFT) a.currentTime = want * period;
    }
  }

  // (re)create the animations when what shapes them changes
  $effect(() => {
    level;
    pulseStyle;
    period;
    ring;
    glow;
    sweep;
    disc;
    untrack(() => {
      for (const a of anims) a.cancel();
      anims = level === "off" ? [] : build();
      const want = phaseNow();
      for (const a of anims) {
        a.currentTime = want * period;
        if (!playing) a.pause();
      }
    });
    return () => {
      for (const a of anims) a.cancel();
      anims = [];
    };
  });
  // play / pause with the music, and keep the phase with the song
  $effect(() => {
    view;
    playing;
    for (const a of anims) {
      if (playing && a.playState === "paused") a.play();
      else if (!playing && a.playState === "running") a.pause();
    }
    if (playing) align();
  });

  // motion off: a still dot that steps once per stroke
  let lit = $state(false);
  $effect(() => {
    if (level !== "off" || !playing) return;
    lit = false;
    const id = setInterval(() => (lit = !lit), Math.max(stroke, STEP_MIN_MS));
    return () => clearInterval(id);
  });
  const rpmShown = $derived(countdown > 0 ? 0.16 : 1);
</script>

<div class="pulse" style:--c={color} role="img" aria-label={label}>
  <div class="disc" bind:this={disc}></div>
  <div class="glow" bind:this={glow}></div>
  <div class="ring" bind:this={ring}></div>
  {#if level === "off"}<span class="dot" class:lit aria-hidden="true"></span>{/if}
  <div class="inner">
    <div class="num" class:small style:opacity={rpmShown}>{digits}</div>
    <div class="unit" style:opacity={rpmShown}>{view.shown ? unitLabel(view.shown.unit) : "RPM"}</div>
    <div class="range" style:opacity={rpmShown}>{speed ? `≈ ${view.target_rpm} RPM` : `${view.range.min_rpm} to ${view.range.max_rpm}`}</div>
    {#if pulseStyle === "bar"}<div class="barwrap" aria-hidden="true"><i bind:this={sweep}></i></div>{/if}
  </div>
  {#if countdown > 0}<div class="count" aria-hidden="true">{#key countdown}<span>{countdown}</span>{/key}</div>{/if}
</div>

<style>
  .pulse {
    --ring: var(--ride-ring);
    position: relative;
    width: var(--ring);
    height: var(--ring);
    margin: 0 auto;
    display: grid;
    place-items: center;
    flex: none;
  }
  .ring,
  .glow,
  .disc {
    position: absolute;
    inset: 0;
    border-radius: 50%;
  }
  .ring {
    border: calc(var(--ring) * 0.032) solid var(--c);
  }
  .glow {
    background: var(--c);
    opacity: 0;
  }
  .disc {
    background: var(--c);
    opacity: 0;
  }
  .inner {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
  }
  .num {
    font-size: calc(var(--ring) * 0.395);
    font-weight: 900;
    line-height: 1;
    color: var(--c);
    font-variant-numeric: tabular-nums;
    letter-spacing: -0.02em;
    transition: opacity 0.2s;
  }
  .num.small {
    font-size: calc(var(--ring) * 0.316);
  }
  .unit {
    font-size: calc(var(--ring) * 0.047);
    font-weight: 700;
    letter-spacing: 0.14em;
    color: var(--muted);
    transition: opacity 0.2s;
  }
  .range {
    font-size: calc(var(--ring) * 0.04);
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    transition: opacity 0.2s;
  }
  .barwrap {
    margin-top: calc(var(--ring) * 0.03);
    width: calc(var(--ring) * 0.5);
    height: 12px;
    border-radius: var(--r-pill);
    background: var(--track);
    overflow: hidden;
  }
  .barwrap i {
    display: block;
    height: 100%;
    background: var(--c);
    transform-origin: left;
    transform: scaleX(0);
  }
  .dot {
    position: absolute;
    top: calc(var(--ring) * 0.07);
    width: calc(var(--ring) * 0.06);
    height: calc(var(--ring) * 0.06);
    border-radius: 50%;
    border: 3px solid var(--c);
  }
  .dot.lit {
    background: var(--c);
  }
  .count {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
  }
  .count span {
    font-size: calc(var(--ring) * 0.5);
    font-weight: 900;
    line-height: 1;
    color: var(--text);
    text-shadow: 0 0 28px var(--bg);
    font-variant-numeric: tabular-nums;
    animation: ex-count 0.5s ease-out;
  }
  @keyframes ex-count {
    from {
      transform: scale(1.25);
      opacity: 0.4;
    }
  }
  :global(:root[data-motion="subtle"]) .count span {
    animation: none;
  }
</style>
