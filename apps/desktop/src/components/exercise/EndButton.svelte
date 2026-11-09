<script lang="ts">
  // End the ride: hold for a second (a ring fills), or click for a small "End the ride?" question. A stray
  // click must never end a ride; the keyboard always gets the question.
  import Icon from "../Icon.svelte";

  let { onend }: { onend: () => void } = $props();

  const HOLD_MS = 1000;
  /** A press shorter than this is a click. */
  const CLICK_MS = 250;
  const RING = 2 * Math.PI * 20;

  let held = $state(0);
  let asking = $state(false);
  let from = 0;
  let raf = 0;

  function frame() {
    held = Math.min(1, (performance.now() - from) / HOLD_MS);
    if (held >= 1) {
      cancelAnimationFrame(raf);
      held = 0;
      onend();
      return;
    }
    raf = requestAnimationFrame(frame);
  }
  function down(e: PointerEvent) {
    if (e.button !== 0) return;
    from = performance.now();
    raf = requestAnimationFrame(frame);
  }
  function up() {
    cancelAnimationFrame(raf);
    const short = performance.now() - from < CLICK_MS;
    held = 0;
    if (short && from) asking = true;
    from = 0;
  }
  function leave() {
    cancelAnimationFrame(raf);
    held = 0;
    from = 0;
  }
  $effect(() => () => cancelAnimationFrame(raf));
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && (asking = false)} onclick={() => (asking = false)} />

<span class="wrap" onclick={(e) => e.stopPropagation()} role="presentation">
  <button
    type="button"
    class="end"
    title="End the ride (hold 1 second)"
    aria-haspopup="dialog"
    aria-expanded={asking}
    aria-label="End the ride"
    onpointerdown={down}
    onpointerup={up}
    onpointerleave={leave}
    onkeydown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), (asking = true))}
  >
    <svg class="fill" viewBox="0 0 44 44" aria-hidden="true"><circle cx="22" cy="22" r="20" stroke-dasharray={RING} stroke-dashoffset={RING * (1 - held)} /></svg>
    <Icon name="x" size={18} />
  </button>
  {#if asking}
    <div class="ask" role="alertdialog" aria-label="End the ride?">
      <b>End the ride?</b>
      <div class="row">
        <button type="button" class="btn tonal" onclick={() => (asking = false)}>Keep riding</button>
        <button type="button" class="btn danger" onclick={() => ((asking = false), onend())}>End ride</button>
      </div>
    </div>
  {/if}
</span>

<style>
  .wrap {
    position: relative;
    display: inline-flex;
  }
  .end {
    position: relative;
    width: var(--ride-hit);
    height: var(--ride-hit);
    border-radius: 50%;
    border: 2px solid var(--danger);
    color: var(--danger);
    display: grid;
    place-items: center;
    touch-action: none;
  }
  .end:hover {
    background: color-mix(in srgb, var(--danger) 14%, transparent);
  }
  .fill {
    position: absolute;
    inset: -2px;
    width: calc(100% + 4px);
    height: calc(100% + 4px);
    transform: rotate(-90deg);
    pointer-events: none;
  }
  .fill circle {
    fill: none;
    stroke: var(--danger);
    stroke-width: 4;
  }
  .ask {
    position: absolute;
    right: 0;
    top: calc(100% + 10px);
    z-index: 20;
    padding: 14px;
    border-radius: var(--r-md);
    background: var(--surface-3);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
    display: flex;
    flex-direction: column;
    gap: 10px;
    white-space: nowrap;
  }
  .row {
    display: flex;
    gap: 8px;
  }
  .btn {
    height: 40px;
    padding: 0 16px;
  }
  .btn.danger {
    background: var(--danger);
    color: #1a0606;
  }
</style>
