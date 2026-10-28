<script lang="ts">
  /** Classic analog VU meter. `value` 0..1 maps to −30 dB .. +3 dB. */
  let { value = 0, label = "L" }: { value?: number; label?: string } = $props();

  const W = 220;
  const H = 128;
  const cx = W / 2;
  const cy = 150;
  const R = 118;
  const SWEEP = 48; // degrees each side

  // real VU scales are not linear: 0 VU sits at ~3/4 of the arc
  const SCALE: [number, number][] = [
    [-20, 0], [-10, 0.3], [-7, 0.42], [-5, 0.52], [-3, 0.63], [-1, 0.75], [0, 0.81], [1, 0.87], [3, 1],
  ];
  const position = (db: number) => {
    if (db <= SCALE[0][0]) return -0.03;
    for (let i = 1; i < SCALE.length; i++) {
      const [d1, p1] = SCALE[i];
      const [d0, p0] = SCALE[i - 1];
      if (db <= d1) return p0 + ((db - d0) / (d1 - d0)) * (p1 - p0);
    }
    return 1.03;
  };
  /** `value` is 0..1 = −30 dB .. +3 dB (see visual.svelte.ts). */
  const dbToValue = (db: number) => position(db);
  const angle = (v: number) => -SWEEP + v * SWEEP * 2;
  const point = (deg: number, r: number) => {
    const a = ((deg - 90) * Math.PI) / 180;
    return [cx + r * Math.cos(a), cy + r * Math.sin(a)];
  };
  const arc = (from: number, to: number, r: number) => {
    const [x1, y1] = point(from, r);
    const [x2, y2] = point(to, r);
    return `M${x1} ${y1}A${r} ${r} 0 0 1 ${x2} ${y2}`;
  };

  const marks = [-20, -10, -5, -3, 0, 3];
  const minor = [-7, -1, 1, 2];
  const db = $derived(value * 33 - 30);
  const needle = $derived(angle(position(db)));
  const hot = $derived(db >= 0);
</script>

<div class="vu">
  <svg viewBox="0 0 {W} {H}" aria-label="{label} channel level">
    <defs>
      <radialGradient id="face-{label}" cx="50%" cy="90%" r="90%">
        <stop offset="0" stop-color="var(--vu-face-hi)" />
        <stop offset="1" stop-color="var(--vu-face)" />
      </radialGradient>
    </defs>
    <rect x="1" y="1" width={W - 2} height={H - 2} rx="12" fill="url(#face-{label})" stroke="var(--vu-rim)" stroke-width="2" />
    <path d={arc(-SWEEP, angle(dbToValue(0)), R - 10)} stroke="var(--vu-ink)" stroke-width="2" fill="none" />
    <path d={arc(angle(dbToValue(0)), SWEEP, R - 10)} stroke="#ef4444" stroke-width="6" fill="none" />
    {#each marks as db}
      {@const a = angle(dbToValue(db))}
      {@const [x1, y1] = point(a, R - 10)}
      {@const [x2, y2] = point(a, R - 20)}
      {@const [tx, ty] = point(a, R - 32)}
      <line {x1} {y1} {x2} {y2} stroke={db > 0 ? "#ef4444" : "var(--vu-ink)"} stroke-width="2" />
      <text x={tx} y={ty} text-anchor="middle" dominant-baseline="middle" fill={db > 0 ? "#ef4444" : "var(--vu-ink)"}>
        {db > 0 ? `+${db}` : db}
      </text>
    {/each}
    {#each minor as db}
      {@const a = angle(dbToValue(db))}
      {@const [x1, y1] = point(a, R - 10)}
      {@const [x2, y2] = point(a, R - 16)}
      <line {x1} {y1} {x2} {y2} stroke={db > 0 ? "#ef4444" : "var(--vu-ink)"} stroke-width="1.5" />
    {/each}
    <text x={cx} y={H - 30} text-anchor="middle" class="vu-label">VU</text>
    <text x="16" y={H - 14} class="ch">{label}</text>
    <circle cx={W - 18} cy={H - 18} r="5" fill={hot ? "#ef4444" : "var(--vu-led-off)"} class:glow={hot} />
    <g style:transform="rotate({needle}deg)" style:transform-origin="{cx}px {cy}px" class="needle">
      <line x1={cx} y1={cy} x2={cx} y2={cy - R + 6} stroke="var(--vu-needle)" stroke-width="2.5" stroke-linecap="round" />
    </g>
    <rect x="0" y={H - 6} width={W} height="6" fill="var(--vu-rim)" opacity="0.6" />
  </svg>
</div>

<style>
  .vu {
    --vu-face: #e9d6a8;
    --vu-face-hi: #fff4d6;
    --vu-ink: #2b2112;
    --vu-needle: #1a1206;
    --vu-rim: #3b3326;
    --vu-led-off: #5a4a32;
    width: 100%;
    max-width: 240px;
  }
  svg {
    width: 100%;
    display: block;
    border-radius: 12px;
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.06), 0 10px 26px rgb(0 0 0 / 0.35);
  }
  text {
    font: 600 10px ui-rounded, system-ui, sans-serif;
  }
  .vu-label {
    font: 800 16px ui-rounded, system-ui, sans-serif;
    fill: var(--vu-ink);
    letter-spacing: 0.08em;
  }
  .ch {
    font: 800 13px ui-rounded, system-ui, sans-serif;
    fill: var(--vu-ink);
  }
  .needle {
    transition: transform 0.06s linear;
  }
  .glow {
    filter: drop-shadow(0 0 5px #ef4444);
  }
</style>
