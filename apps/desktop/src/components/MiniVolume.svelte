<script lang="ts">
  // The mini player's volume: the speaker button mutes (as before); pointing at it opens a
  // slim slider beside it, and the wheel or the arrow keys turn it. Always this computer's
  // volume, like the knob on the deck.
  import { api } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import Icon from "./Icon.svelte";

  /** One wheel notch or arrow key. */
  const STEP = 0.05;

  let { open = $bindable(false) }: { open?: boolean } = $props();

  // shown at once while dragging; the engine's state catches up 4 times a second
  let local = $state<number | null>(null);
  const volume = $derived(local ?? app.player?.volume ?? 0.8);
  let settle: ReturnType<typeof setTimeout> | undefined;

  export function set(v: number) {
    const next = Math.round(Math.max(0, Math.min(1, v)) * 100) / 100;
    local = next;
    clearTimeout(settle);
    settle = setTimeout(() => (local = null), 600);
    api.volume(next).catch(() => {});
    // turning it up means "I want to hear it"
    if (app.muted && next > 0) app.toggleMute();
  }

  export function nudge(steps: number) {
    set(volume + steps * STEP);
  }

  function onWheel(e: WheelEvent) {
    e.preventDefault();
    if (e.deltaY !== 0) nudge(e.deltaY < 0 ? 1 : -1);
  }

  const icon = $derived(app.muted || volume === 0 ? "mute" : "volume");
</script>

<div
  class="vol"
  class:open
  role="group"
  aria-label="Volume"
  onmouseenter={() => (open = true)}
  onmouseleave={() => (open = false)}
  onfocusin={() => (open = true)}
  onfocusout={() => (open = false)}
  onwheel={onWheel}
>
  <button
    class="icon-btn key mute"
    class:on={app.muted}
    aria-label={app.muted ? "Unmute" : "Mute"}
    aria-pressed={app.muted}
    title={app.muted ? "Unmute (M) · wheel or ↑↓: volume" : "Mute (M) · wheel or ↑↓: volume"}
    onclick={() => app.toggleMute()}
  >
    <Icon name={icon} size={15} />
  </button>
  <input
    type="range"
    min="0"
    max="1"
    step="0.01"
    value={app.muted ? 0 : volume}
    aria-label="Volume"
    aria-valuetext="{Math.round(volume * 100)}%"
    tabindex={open ? 0 : -1}
    style:--fill="{(app.muted ? 0 : volume) * 100}%"
    oninput={(e) => set(Number(e.currentTarget.value))}
  />
</div>

<style>
  .vol {
    display: flex;
    align-items: center;
  }
  /* silenced: the button says so at a glance */
  .mute.on {
    color: #f87171;
  }
  .key {
    color: var(--m-key);
  }
  input {
    appearance: none;
    width: 0;
    height: 4px;
    margin: 0;
    opacity: 0;
    border-radius: var(--r-pill);
    background: linear-gradient(to right, var(--m-fill) var(--fill), var(--m-groove) var(--fill));
    transition:
      width 0.18s ease,
      opacity 0.18s ease,
      margin 0.18s ease;
    cursor: pointer;
  }
  .open input {
    width: 64px;
    margin: 0 6px 0 2px;
    opacity: 1;
  }
  input::-webkit-slider-thumb {
    appearance: none;
    width: 11px;
    height: 11px;
    border-radius: 50%;
    background: var(--m-text);
    box-shadow: 0 0 6px color-mix(in srgb, var(--room-accent) 60%, transparent);
  }
  @media (prefers-reduced-motion: reduce) {
    input {
      transition: none;
    }
  }
</style>
