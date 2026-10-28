<script lang="ts">
  /** Hi-fi style rotary knob, 0..1. Drag up/down or use the wheel / arrow keys. */
  let { value = 0.8, onchange, size = 56, label = "Volume" }: { value?: number; onchange: (v: number) => void; size?: number; label?: string } = $props();

  const MIN = -135;
  const MAX = 135;
  const angle = $derived(MIN + (MAX - MIN) * value);

  let start: { y: number; v: number } | null = null;

  function set(v: number) {
    onchange(Math.round(Math.min(1, Math.max(0, v)) * 100) / 100);
  }

  function down(e: PointerEvent) {
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    start = { y: e.clientY, v: value };
  }
  function move(e: PointerEvent) {
    if (start) set(start.v + (start.y - e.clientY) / 160);
  }
  function up() {
    start = null;
  }

  const ticks = Array.from({ length: 11 }, (_, i) => MIN + ((MAX - MIN) * i) / 10);
</script>

<div
  class="knob"
  style:width="{size}px"
  style:height="{size}px"
  role="slider"
  tabindex="0"
  aria-label={label}
  aria-valuemin="0"
  aria-valuemax="100"
  aria-valuenow={Math.round(value * 100)}
  title="{label}: {Math.round(value * 100)}%"
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
  onwheel={(e) => {
    e.preventDefault();
    set(value - e.deltaY / 800);
  }}
  ondblclick={() => set(0.8)}
  onkeydown={(e) => {
    if (e.key === "ArrowUp" || e.key === "ArrowRight") set(value + 0.05);
    if (e.key === "ArrowDown" || e.key === "ArrowLeft") set(value - 0.05);
  }}
>
  <svg viewBox="0 0 100 100">
    {#each ticks as t, i}
      <line
        x1="50"
        y1="4"
        x2="50"
        y2="11"
        transform="rotate({t} 50 50)"
        stroke={i / 10 <= value ? "var(--room-accent)" : "var(--glass-border)"}
        stroke-width="3"
        stroke-linecap="round"
      />
    {/each}
    <circle cx="50" cy="50" r="34" fill="url(#knob-metal)" stroke="rgb(0 0 0 / 0.4)" stroke-width="1.5" />
    <defs>
      <radialGradient id="knob-metal" cx="40%" cy="30%" r="80%">
        <stop offset="0" stop-color="#5b5675" />
        <stop offset="0.6" stop-color="#2c2840" />
        <stop offset="1" stop-color="#16131f" />
      </radialGradient>
    </defs>
    <line x1="50" y1="22" x2="50" y2="34" transform="rotate({angle} 50 50)" stroke="var(--room-accent)" stroke-width="5" stroke-linecap="round" />
  </svg>
</div>

<style>
  .knob {
    flex: none;
    cursor: ns-resize;
    touch-action: none;
    border-radius: 50%;
  }
  svg {
    width: 100%;
    height: 100%;
    display: block;
    filter: drop-shadow(0 6px 10px rgb(0 0 0 / 0.4));
  }
</style>
