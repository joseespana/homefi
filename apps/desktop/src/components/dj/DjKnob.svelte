<script lang="ts">
  // A DJ knob: unipolar (0..max) or bipolar (an arc from the detent, e.g. EQ and filter).
  // Gestures from lib/djDrag (drag, Shift fine, wheel, double-click = detent, keys).
  import { dragValue, Held } from "../../lib/djDrag.svelte";

  let {
    value,
    min = 0,
    max = 1,
    detent = 0,
    label,
    tip,
    valueText,
    onchange,
    size = 44,
    color = "var(--primary)",
    dim = false,
    disabled = false,
  }: {
    value: number;
    min?: number;
    max?: number;
    /** Where double-click / Home puts it, and where the arc starts. */
    detent?: number;
    /** Accessible name ("High, deck A"). */
    label: string;
    /** Plain-words tooltip. */
    tip?: string;
    valueText: (v: number) => string;
    onchange: (v: number) => void;
    size?: number;
    color?: string;
    /** A killed band: the knob stays but looks off. */
    dim?: boolean;
    disabled?: boolean;
  } = $props();

  const SWEEP_DEG = 270;
  const START_DEG = -135;
  const R = 46;
  const held = new Held();
  const shown = $derived(held.value ?? value);
  const frac = (v: number) => (Math.min(max, Math.max(min, v)) - min) / (max - min || 1);
  const angle = $derived(START_DEG + SWEEP_DEG * frac(shown));
  const detentAngle = $derived(START_DEG + SWEEP_DEG * frac(detent));

  function point(deg: number) {
    const rad = ((deg - 90) * Math.PI) / 180;
    return `${50 + R * Math.cos(rad)} ${50 + R * Math.sin(rad)}`;
  }
  const arc = $derived.by(() => {
    const [a, b] = angle < detentAngle ? [angle, detentAngle] : [detentAngle, angle];
    if (b - a < 0.5) return "";
    return `M ${point(a)} A ${R} ${R} 0 ${b - a > 180 ? 1 : 0} 1 ${point(b)}`;
  });

  function change(v: number) {
    held.set(v);
    onchange(v);
  }
</script>

<div
  class="knob"
  class:dim
  class:disabled
  style:width="{size}px"
  style:height="{size}px"
  style:--kc={color}
  role="slider"
  tabindex={disabled ? -1 : 0}
  aria-label={label}
  aria-valuemin={min}
  aria-valuemax={max}
  aria-valuenow={Math.round(shown * 100) / 100}
  aria-valuetext={valueText(shown)}
  aria-disabled={disabled || undefined}
  title="{tip ?? label}: {valueText(shown)}"
  use:dragValue={{ value: shown, min, max, reset: detent, onchange: change, ondone: () => held.release(), disabled }}
>
  <svg viewBox="0 0 100 100" aria-hidden="true">
    <circle class="track" cx="50" cy="50" r={R} />
    {#if arc}<path class="arc" d={arc} />{/if}
  </svg>
  <span class="body"><span class="pointer" style:transform="rotate({angle}deg)"></span></span>
</div>

<style>
  .knob {
    position: relative;
    flex: none;
    border-radius: 50%;
    cursor: ns-resize;
    touch-action: none;
  }
  .knob.disabled {
    cursor: default;
    opacity: 0.45;
  }
  svg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    overflow: visible;
  }
  .track {
    fill: none;
    stroke: var(--border);
    stroke-width: 6;
    stroke-dasharray: 217 300;
    transform: rotate(135deg);
    transform-origin: 50% 50%;
  }
  .arc {
    fill: none;
    stroke: var(--kc);
    stroke-width: 6;
    stroke-linecap: round;
  }
  .body {
    position: absolute;
    inset: 14%;
    border-radius: 50%;
    background: radial-gradient(circle at 35% 30%, var(--hw-top), var(--hw-bottom) 70%);
    border: 1px solid color-mix(in srgb, var(--text) 18%, var(--border));
    box-shadow: 0 3px 8px rgb(0 0 0 / 0.35);
  }
  .knob:hover .body {
    border-color: var(--kc);
  }
  .pointer {
    position: absolute;
    left: 50%;
    top: 8%;
    bottom: 50%;
    width: 3px;
    margin-left: -1.5px;
    border-radius: 2px;
    background: var(--kc);
    transform-origin: 50% 100%;
  }
  .dim .pointer,
  .dim .arc {
    opacity: 0.35;
  }
  .dim .body {
    filter: grayscale(1) brightness(0.8);
  }
</style>
