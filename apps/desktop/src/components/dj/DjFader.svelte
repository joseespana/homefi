<script lang="ts">
  // A DJ fader, vertical (tempo, channel volume) or horizontal (crossfader). The fill grows from
  // `zero` (the centre for tempo, the bottom for volume). A press on the track jumps there.
  import type { Snippet } from "svelte";
  import { dragValue, Held } from "../../lib/djDrag.svelte";

  let {
    value,
    min = 0,
    max = 1,
    reset,
    zero = min,
    label,
    tip,
    valueText,
    onchange,
    ondone,
    horizontal = false,
    fill = true,
    centerLine = false,
    color = "var(--c, var(--primary))",
    disabled = false,
    children,
  }: {
    value: number;
    min?: number;
    max?: number;
    reset: number;
    /** Where the fill starts. */
    zero?: number;
    label: string;
    tip?: string;
    valueText: (v: number) => string;
    onchange: (v: number) => void;
    ondone?: () => void;
    horizontal?: boolean;
    fill?: boolean;
    centerLine?: boolean;
    color?: string;
    disabled?: boolean;
    children?: Snippet;
  } = $props();

  const held = new Held();
  const shown = $derived(held.value ?? value);
  const pct = (v: number) => ((Math.min(max, Math.max(min, v)) - min) / (max - min || 1)) * 100;
  const at = $derived(pct(shown));
  const from = $derived(pct(zero));
  const fillStart = $derived(Math.min(at, from));
  const fillSize = $derived(Math.abs(at - from));

  function change(v: number) {
    held.set(v);
    onchange(v);
  }
  function done() {
    held.release();
    ondone?.();
  }
</script>

<div
  class="fader"
  class:h={horizontal}
  class:disabled
  style:--fc={color}
  role="slider"
  tabindex={disabled ? -1 : 0}
  aria-label={label}
  aria-orientation={horizontal ? "horizontal" : "vertical"}
  aria-valuemin={min}
  aria-valuemax={max}
  aria-valuenow={Math.round(shown * 1000) / 1000}
  aria-valuetext={valueText(shown)}
  aria-disabled={disabled || undefined}
  title="{tip ?? label}: {valueText(shown)}"
  use:dragValue={{ value: shown, min, max, reset, onchange: change, ondone: done, horizontal, track: true, disabled }}
>
  <span class="travel" data-travel>
    <span class="rail" aria-hidden="true"></span>
    {#if centerLine}<span class="mid" aria-hidden="true"></span>{/if}
    {#if fill && fillSize > 0}
      <span class="fill" aria-hidden="true" style={horizontal ? `left:${fillStart}%;width:${fillSize}%` : `bottom:${fillStart}%;height:${fillSize}%`}></span>
    {/if}
    <span class="cap" data-cap aria-hidden="true" style={horizontal ? `left:${at}%` : `bottom:${at}%`}></span>
  </span>
  {@render children?.()}
</div>

<style>
  .fader {
    position: relative;
    width: 44px;
    min-height: 110px;
    flex: 1;
    cursor: ns-resize;
    touch-action: none;
    border-radius: var(--r-xs);
  }
  .fader.h {
    width: auto;
    min-height: 0;
    height: 48px;
    flex: none;
    cursor: ew-resize;
  }
  .fader.disabled {
    cursor: default;
    opacity: 0.45;
  }
  /* the cap's travel: percentages of the value live in here */
  .travel {
    position: absolute;
    inset: 14px 0;
    pointer-events: none;
  }
  .h .travel {
    inset: 0 15px;
  }
  .rail {
    position: absolute;
    left: 50%;
    top: 0;
    bottom: 0;
    width: 6px;
    margin-left: -3px;
    border-radius: var(--r-xs);
    background: var(--dj-well);
    box-shadow: inset 0 0 0 1px var(--border);
  }
  .h .rail {
    left: 0;
    right: 0;
    top: 50%;
    bottom: auto;
    width: auto;
    height: 6px;
    margin: -3px 0 0;
    background: linear-gradient(90deg, var(--dj-a), var(--dj-well) 50%, var(--dj-b));
  }
  .mid {
    position: absolute;
    left: 8px;
    right: 8px;
    top: 50%;
    height: 1px;
    background: var(--faint);
  }
  .fill {
    position: absolute;
    left: 50%;
    width: 6px;
    margin-left: -3px;
    border-radius: var(--r-xs);
    background: var(--fc);
    opacity: 0.9;
  }
  .h .fill {
    top: 50%;
    height: 6px;
    margin: -3px 0 0;
  }
  .cap {
    position: absolute;
    left: 50%;
    width: 40px;
    height: 28px;
    margin-left: -20px;
    border-radius: var(--r-xs);
    background: linear-gradient(var(--hw-top), var(--hw-bottom));
    border: 1px solid color-mix(in srgb, var(--text) 22%, var(--border));
    box-shadow: 0 3px 8px rgb(0 0 0 / 0.35);
    transform: translateY(50%);
    pointer-events: auto;
    cursor: grab;
  }
  .cap::after {
    content: "";
    position: absolute;
    left: 6px;
    right: 6px;
    top: 50%;
    height: 2px;
    margin-top: -1px;
    background: var(--fc);
  }
  .h .cap {
    top: 50%;
    width: 30px;
    height: 42px;
    margin: -21px 0 0 -15px;
    transform: none;
  }
  .h .cap::after {
    left: 50%;
    right: auto;
    top: 6px;
    bottom: 6px;
    width: 2px;
    height: auto;
    margin: 0 0 0 -1px;
    background: var(--primary);
  }
  .fader:hover:not(.disabled) .cap {
    border-color: var(--fc);
  }
</style>
