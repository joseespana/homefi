<script lang="ts">
  // Homifito DJ: the little house with headphones behind his own booth (two tiny turntables, a mixer,
  // speakers and lights). He works with what DJ mode does: bobs on the master beat, keeps a hand on
  // each deck, scratches when a deck is dragged or the Scratch pad fires, reaches for the crossfader
  // during "Mix to A/B", cheers on air horns and jumps on drops, and lifts one headphone cup while the
  // other deck is cued. "subtle" is a calm sway, "off" a still picture. The maths is in
  // lib/homifito-dj.ts; this draws it.
  import { dj } from "../../lib/dj.svelte";
  import { DECKS, deckIndex } from "../../lib/dj-api";
  import { onFrame } from "../../lib/djColors.svelte";
  import { MASCOT, MASCOT_COLORS as C } from "../../lib/mascot";
  import { motionWatch } from "../../lib/motionLevel.svelte";
  import { REACTION_MS, REST_POSE, aim, approach, armPath, beatNumber, beatPhase, cueingDeck, crossfaderKnob, freePhase, FREE_BPM, HAND_R, HOUSE_SCALE, HOUSE_TOP_Y, isScratching, masterDeck, PIVOT, PLATTER, PLATTER_RX, PLATTER_SQUASH, platterDot, reactionOf, recordDegrees, shoulderAt, SHOULDER_L, SHOULDER_R, STAGE, XF_SLOT, type Pose, type Reaction } from "../../lib/homifito-dj";

  let { size }: { size: "booth" | "big" } = $props();

  /** Bars on each side wall, their lowest height, and how fast / how far apart in time they dance. */
  const BARS = 14;
  const BAR_FLOOR = 0.07;
  const BAR_HZ = 5;
  const BAR_SPREAD = 0.8;
  const BAR_SIDE_SHIFT = 1.7;

  /** What one frame draws. */
  interface Frame {
    pose: Pose;
    deg: [number, number];
    glow: [number, number];
    vu: [number, number];
    xf: number;
    beat: number;
    /** The two walls of equalizer bars (0..1 each), deck A's side then B's. */
    bars: [number[], number[]];
    reaction: Reaction | null;
    playing: [boolean, boolean];
    loaded: [boolean, boolean];
  }
  const STILL: Frame = { pose: REST_POSE, deg: [0, 0], glow: [0, 0], vu: [0, 0], xf: 0.5, beat: 0, bars: [Array(BARS).fill(BAR_FLOOR), Array(BARS).fill(BAR_FLOOR)], reaction: null, playing: [false, false], loaded: [false, false] };
  let frame = $state.raw<Frame>(STILL);

  // ---- what the loop remembers between frames (plain values: they are not drawn)
  let pose: Pose = REST_POSE;
  let lastAt = 0;
  let reaction: { kind: Reaction; startedAt: number } | null = null;
  let padsWere: boolean[] = [];
  let positionWas: [number, number] = [0, 0];
  let scratchUntil: [number, number] = [0, 0];
  let xfWas = 0.5;
  let xfTouchUntil = 0;
  let xfHand: 0 | 1 | null = null;

  /** A deck dragged this recently still counts as being scratched (snapshots arrive at 20 Hz). */
  const SCRATCH_HOLD_MS = 260;
  /** The crossfader's hand stays on it this long after the knob last moved. */
  const XF_HOLD_MS = 1100;
  const XF_MOVED = 0.002;
  const CENTER = 0.5;
  const MAX_DT_S = 0.1;
  const VU_LEDS = 5;
  const BEAT_GLOW = 0.5;
  const wall = (d: 0 | 1, playing: boolean, level: number, seconds: number, full: boolean, pulse: number): number[] =>
    Array.from({ length: BARS }, (_, n) => {
      if (!playing) return BAR_FLOOR;
      const reach = 0.25 + 0.75 * level;
      if (!full) return Math.max(BAR_FLOOR, reach * (0.3 + 0.35 * (n % 3) / 2));
      const wave = Math.abs(Math.sin(seconds * BAR_HZ + n * BAR_SPREAD + d * BAR_SIDE_SHIFT));
      return Math.max(BAR_FLOOR, reach * (0.3 + 0.7 * wave) * (0.65 + 0.35 * pulse));
    });

  function step(now: number, dt: number) {
    const snap = dj.snap;
    const level = motionWatch.level;
    const seconds = now / 1000;
    const master = masterDeck(snap);

    // beat: the master deck's, or free-running
    let phase = freePhase(seconds);
    let beatIndex = Math.floor((seconds * FREE_BPM) / 60);
    if (snap && master) {
      const i = deckIndex(master);
      const at = dj.positionMs(master);
      const p = beatPhase(snap.decks[i], at);
      if (p !== null) {
        phase = p;
        beatIndex = beatNumber(snap.decks[i], at);
      }
    }

    // a sampler pad just fired: react to its sound
    snap?.pads_active.forEach((on, n) => {
      if (on && !padsWere[n]) reaction = { kind: reactionOf(dj.layout.pads[n] ?? "hit"), startedAt: now };
    });
    padsWere = snap?.pads_active ?? [];
    if (reaction && now - reaction.startedAt >= REACTION_MS[reaction.kind]) reaction = null;

    // decks being dragged (a paused one whose playhead moves) or nudged
    const scratching: [boolean, boolean] = [false, false];
    for (const d of DECKS) {
      const i = deckIndex(d);
      const s = snap?.decks[i];
      const at = dj.positionMs(d);
      if (s && isScratching(s, at - positionWas[i])) scratchUntil[i] = now + SCRATCH_HOLD_MS;
      positionWas[i] = at;
      scratching[i] = now < scratchUntil[i];
    }

    // a hand on the crossfader while it moves or a mix runs
    const xf = snap?.crossfader ?? CENTER;
    if (Math.abs(xf - xfWas) > XF_MOVED || snap?.auto_mix) {
      if (xfHand === null || now >= xfTouchUntil) xfHand = xf < CENTER ? 0 : 1;
      xfTouchUntil = now + XF_HOLD_MS;
    }
    xfWas = xf;
    if (now >= xfTouchUntil) xfHand = null;

    const playing: [boolean, boolean] = [!!snap?.decks[0].playing, !!snap?.decks[1].playing];
    const target = aim({
      level,
      phase,
      beatIndex,
      playing,
      scratching,
      cueing: cueingDeck(snap),
      crossfader: xf,
      xfHand,
      reaction: reaction ? { kind: reaction.kind, progress: (now - reaction.startedAt) / REACTION_MS[reaction.kind] } : null,
      seconds,
    });
    pose = approach(pose, target, dt);

    const full = level === "full";
    const spin = (d: 0 | 1) => (full && snap?.decks[d].track ? recordDegrees(dj.positionMs(DECKS[d])) : 0);
    const weight = [1 - xf, xf];
    const pulse = playing[0] || playing[1] ? 1 - phase : 0;
    const glow = (d: 0 | 1): number => (playing[d] ? 0.3 + 0.5 * Math.min(1, weight[d] * 1.6) + (full ? BEAT_GLOW * 0.4 * pulse * pulse : 0) : 0);
    frame = {
      pose,
      deg: [spin(0), spin(1)],
      glow: [glow(0), glow(1)],
      vu: [Math.max(...(snap?.decks[0].peak ?? [0, 0])), Math.max(...(snap?.decks[1].peak ?? [0, 0]))],
      xf,
      bars: [wall(0, playing[0], Math.max(...(snap?.decks[0].peak ?? [0, 0])), seconds, full, pulse), wall(1, playing[1], Math.max(...(snap?.decks[1].peak ?? [0, 0])), seconds, full, pulse)],
      beat: full && (playing[0] || playing[1]) ? 1 - Math.min(1, phase * 4) : 0,
      reaction: reaction?.kind ?? null,
      playing,
      loaded: [!!snap?.decks[0].track, !!snap?.decks[1].track],
    };
  }

  $effect(() => {
    if (motionWatch.level === "off") {
      // a still picture of the state, redrawn when a snapshot changes it
      step(performance.now(), Infinity);
      return;
    }
    lastAt = performance.now();
    return onFrame(() => {
      const now = performance.now();
      const dt = Math.min(MAX_DT_S, (now - lastAt) / 1000);
      lastAt = now;
      step(now, dt);
    });
  });

  // ---- the drawing
  const HS = HOUSE_SCALE;
  const houseTransform = `translate(${STAGE.w / 2 - 512 * HS} ${HOUSE_TOP_Y - 288 * HS}) scale(${HS})`;
  const [cupL, cupR] = MASCOT.cups.split("ZM").map((d, i) => (i ? `M${d}` : `${d}Z`));
  const [padL, padR] = MASCOT.pads.split("ZM").map((d, i) => (i ? `M${d}` : `${d}Z`));
  const CUP_Y = 606;
  const CUP_X = { l: 242, r: 782 } as const;
  const CUP_LIFT = { x: 34, y: 78, deg: 26 } as const;
  const cupTransform = (side: "l" | "r") => {
    const lift = frame.pose.cupSide === side ? frame.pose.cupLift : 0;
    const s = side === "l" ? -1 : 1;
    return `translate(${(s * CUP_LIFT.x * lift).toFixed(1)} ${(-CUP_LIFT.y * lift).toFixed(1)}) rotate(${(s * CUP_LIFT.deg * lift).toFixed(1)} ${CUP_X[side]} ${CUP_Y})`;
  };
  const [lx, ly] = [440, 600];
  const [rx, ry] = [584, 600];
  const happyEyes = `M${lx - 34} ${ly + 10}Q${lx} ${ly - 34} ${lx + 34} ${ly + 10}M${rx - 34} ${ry + 10}Q${rx} ${ry - 34} ${rx + 34} ${ry + 10}`;
  const openSmile = "M436 668Q512 790 588 668Q512 700 436 668Z";

  const shoulderL = $derived(shoulderAt(SHOULDER_L, frame.pose));
  const shoulderR = $derived(shoulderAt(SHOULDER_R, frame.pose));
  const arms = $derived([
    { d: armPath(shoulderL, frame.pose.handL), hand: frame.pose.handL },
    { d: armPath(shoulderR, frame.pose.handR), hand: frame.pose.handR },
  ]);
  const knob = $derived(crossfaderKnob(frame.xf));
  const stripes = $derived(([0, 1] as const).map((d) => [0, 180].map((off) => [platterDot(d, frame.deg[d] + off, 34), platterDot(d, frame.deg[d] + off, 90)] as const)));
  const ledOn = (level: number, n: number) => level * VU_LEDS > n + 0.3;
  const sparkle = (a: number, r: number, s: number) => `M${(PIVOT.x + Math.cos(a) * r).toFixed(0)} ${(HOUSE_TOP_Y + 40 + Math.sin(a) * r * 0.55).toFixed(0)}m0 ${-s}l${s * 0.3} ${s * 0.7}l${s * 0.7} ${s * 0.3}l${-s * 0.7} ${s * 0.3}l${-s * 0.3} ${s * 0.7}l${-s * 0.3} ${-s * 0.7}l${-s * 0.7} ${-s * 0.3}l${s * 0.7} ${-s * 0.3}z`;
</script>

<div class="hdj {size}" style:--ga={frame.glow[0].toFixed(2)} style:--gb={frame.glow[1].toFixed(2)} aria-hidden="true">
  <!-- the walls of equalizer bars on both sides of the stage -->
  {#each [0, 1] as d}
    <div class="wall {d ? 'b' : 'a'}">
      {#each frame.bars[d] as h}<i style:height="{(h * 100).toFixed(0)}%"></i>{/each}
    </div>
  {/each}
  <svg viewBox="0 {STAGE.top} {STAGE.w} {STAGE.h - STAGE.top}" preserveAspectRatio="xMidYMax meet" focusable="false">
    <defs>
      <linearGradient id="hdj-beam-a" x1="0" y1="0" x2="0" y2="1"><stop offset="0" style="stop-color: var(--dj-a)" stop-opacity="0.55" /><stop offset="1" style="stop-color: var(--dj-a)" stop-opacity="0" /></linearGradient>
      <linearGradient id="hdj-beam-b" x1="0" y1="0" x2="0" y2="1"><stop offset="0" style="stop-color: var(--dj-b)" stop-opacity="0.55" /><stop offset="1" style="stop-color: var(--dj-b)" stop-opacity="0" /></linearGradient>
      <linearGradient id="hdj-house" x1="0" y1="288" x2="0" y2="806" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="#ffffff" /><stop offset="1" stop-color="#ddefea" /></linearGradient>
      <linearGradient id="hdj-top" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#2c4260" /><stop offset="1" stop-color="#1d2f49" /></linearGradient>
      <filter id="hdj-soft" x="-20%" y="-20%" width="140%" height="150%"><feDropShadow dx="0" dy="6" stdDeviation="7" flood-color="#021b2e" flood-opacity="0.35" /></filter>
    </defs>

    <!-- spotlights: one in each deck's colour, brighter on the beat while that deck plays -->
    <polygon points="150,-84 210,-84 400,300 20,300" fill="url(#hdj-beam-a)" opacity={frame.glow[0]} />
    <polygon points="790,-84 850,-84 980,300 600,300" fill="url(#hdj-beam-b)" opacity={frame.glow[1]} />

    <!-- speakers: the woofers push out on the beat -->
    {#each [{ x: 22 }, { x: 912 }] as sp}
      <g>
        <rect x={sp.x} y="176" width="66" height="230" rx="10" fill="#14223a" stroke="#ffffff" stroke-opacity="0.1" />
        {#each [236, 330] as cy, n}
          <circle cx={sp.x + 33} cy={cy} r={n ? 24 : 18} fill="#0b1524" stroke="#34495f" stroke-width="2" />
          <circle cx={sp.x + 33} cy={cy} r={(n ? 11 : 8) * (1 + 0.28 * frame.beat)} fill="#26384f" />
        {/each}
        <circle cx={sp.x + 33} cy="194" r="3.5" fill={C.cushion} opacity={frame.beat > 0 ? 1 : 0.35} />
      </g>
    {/each}

    <!-- Homifito: the logo's house, swaying and dropping on the beat -->
    <g transform="translate(0 {frame.pose.bodyY.toFixed(2)}) rotate({frame.pose.tilt.toFixed(2)} {PIVOT.x} {PIVOT.y})">
      <g transform={houseTransform}>
        <g transform="rotate(-6 512 560)" filter="url(#hdj-soft)">
          <path d={MASCOT.house} fill="url(#hdj-house)" stroke={C.navy} stroke-opacity="0.18" stroke-width="10" />
          <path d={MASCOT.band} fill="none" style="stroke: var(--hdj-rim)" stroke-width="66" stroke-linecap="round" />
          <path d={MASCOT.band} fill="none" stroke={C.navy} stroke-width="50" stroke-linecap="round" />
          <path d={MASCOT.band} fill="none" stroke="#5b86b8" stroke-opacity="0.5" stroke-width="10" stroke-linecap="round" transform="translate(0 -10) scale(1 0.97)" />
          <g transform={cupTransform("l")}><path d={cupL} fill={C.navy} style="stroke: var(--hdj-rim)" stroke-width="10" /><path d={padL} fill={C.cushion} /></g>
          <g transform={cupTransform("r")}><path d={cupR} fill={C.navy} style="stroke: var(--hdj-rim)" stroke-width="10" /><path d={padR} fill={C.cushion} /></g>
          {#if frame.pose.happy}
            <path d={happyEyes} fill="none" stroke={C.navy} stroke-width="18" stroke-linecap="round" />
            <path d={openSmile} fill={C.navy} />
            <path d="M470 740Q512 760 554 740Q512 722 470 740Z" fill="#f472b6" />
          {:else}
            <path d={MASCOT.eyes} fill={C.navy} />
            <path d={MASCOT.sparkles} fill={C.white} />
            <path d={MASCOT.smile} fill={C.navy} />
          {/if}
          <path d={MASCOT.cheeks} fill="#f472b6" fill-opacity="0.6" />
        </g>
      </g>
    </g>

    <!-- the booth -->
    <g>
      <rect x="70" y="268" width="860" height="132" rx="22" fill="#17263f" stroke="#ffffff" stroke-opacity="0.14" />
      <rect x="84" y="274" width="832" height="104" rx="16" fill="url(#hdj-top)" />
      <rect x="84" y="378" width="832" height="22" rx="8" fill="#101c30" />
      <text x="500" y="394" text-anchor="middle" font-size="13" font-weight="800" letter-spacing="5" fill="#5b86b8">HOMIFITO DJ</text>

      {#each [0, 1] as d}
        {@const c = PLATTER[d]}
        {@const dir = d === 0 ? 1 : -1}
        <rect x={c.x - 128} y="282" width="256" height="88" rx="14" fill="#101c30" />
        <ellipse cx={c.x} cy={c.y} rx={PLATTER_RX + 8} ry={(PLATTER_RX + 8) * PLATTER_SQUASH} fill="#0a1220" stroke={d ? "var(--dj-b)" : "var(--dj-a)"} stroke-width="3" stroke-opacity={frame.playing[d] ? 0.95 : 0.3} />
        <ellipse cx={c.x} cy={c.y} rx={PLATTER_RX} ry={PLATTER_RX * PLATTER_SQUASH} fill="#05090f" />
        <ellipse cx={c.x} cy={c.y} rx="78" ry={78 * PLATTER_SQUASH} fill="none" stroke="#ffffff" stroke-opacity="0.07" />
        <ellipse cx={c.x} cy={c.y} rx="58" ry={58 * PLATTER_SQUASH} fill="none" stroke="#ffffff" stroke-opacity="0.07" />
        {#each stripes[d] as [a, b]}<path d="M{a.x.toFixed(1)} {a.y.toFixed(1)}L{b.x.toFixed(1)} {b.y.toFixed(1)}" stroke="#ffffff" stroke-opacity="0.55" stroke-width="3" stroke-linecap="round" />{/each}
        <ellipse cx={c.x} cy={c.y} rx="30" ry={30 * PLATTER_SQUASH} fill={d ? "var(--dj-b)" : "var(--dj-a)"} opacity={frame.loaded[d] ? 1 : 0.35} />
        <circle cx={c.x} cy={c.y} r="3" fill="#05090f" />
        <!-- tonearm -->
        <circle cx={c.x + dir * 112} cy="298" r="7" fill="#33475f" />
        <path d="M{c.x + dir * 112} 298L{c.x + dir * 60} {c.y + 4}" stroke="#9fb3c8" stroke-width="4" stroke-linecap="round" />
      {/each}

      <rect x="398" y="284" width="204" height="86" rx="10" fill="#101c30" />
      {#each [424, 452, 548, 576] as kx, n}
        <circle cx={kx} cy="304" r="8" fill="#33475f" />
        <path d="M{kx} 304L{kx + (n % 2 ? 4 : -4)} 298" stroke="#e8eef5" stroke-width="2.4" stroke-linecap="round" />
      {/each}
      {#each [0, 1] as d}
        {#each Array(VU_LEDS) as _, n}
          <rect x={d ? 516 : 472} y={338 - n * 8 - 4} width="12" height="5" rx="1.5" fill={n >= 4 ? "#f87171" : d ? "var(--dj-b)" : "var(--dj-a)"} opacity={ledOn(frame.vu[d], n) ? 1 : 0.18} />
        {/each}
      {/each}
      <rect x={XF_SLOT.x1} y={XF_SLOT.y - 3} width={XF_SLOT.x2 - XF_SLOT.x1} height="6" rx="3" fill="#05101a" />
      <rect x={knob.x - 12} y={XF_SLOT.y - 11} width="24" height="16" rx="4" fill="#e8eef5" stroke="#0b1e3a" stroke-opacity="0.4" />
      <path d="M{knob.x} {XF_SLOT.y - 9}V{XF_SLOT.y + 3}" stroke="#0b1e3a" stroke-width="2.4" />
    </g>

    <!-- his arms, in front of the booth -->
    {#each arms as a}
      <path d={a.d} fill="none" stroke={C.navy} stroke-width="25" stroke-linecap="round" />
    {/each}
    {#each arms as a}
      <path d={a.d} fill="none" stroke={C.white} stroke-width="16" stroke-linecap="round" />
    {/each}
    {#each arms as a}
      <circle cx={a.hand.x} cy={a.hand.y} r={HAND_R + 4} fill={C.navy} />
      <circle cx={a.hand.x} cy={a.hand.y} r={HAND_R} fill={C.white} />
    {/each}

    {#if frame.reaction === "hands_up" || frame.reaction === "jump"}
      <g fill={C.note} stroke={C.navy} stroke-opacity="0.35" stroke-width="2" class="stars">
        <path d={sparkle(-2.6, 190, 18)} />
        <path d={sparkle(-0.55, 200, 22)} />
        <path d={sparkle(-1.6, 150, 14)} />
      </g>
    {/if}
  </svg>
</div>

<style>
  .hdj {
    position: relative;
    flex: none;
    height: var(--hdj-h);
    min-width: 0;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
    /* the deck colours spill across the stage while that deck plays */
    background:
      radial-gradient(ellipse 34% 120% at 14% 105%, color-mix(in srgb, var(--dj-a) calc(var(--ga, 0) * 42%), transparent), transparent),
      radial-gradient(ellipse 34% 120% at 86% 105%, color-mix(in srgb, var(--dj-b) calc(var(--gb, 0) * 42%), transparent), transparent),
      var(--surface);
  }
  /* a lighter rim on his navy parts so the band and cups read on a dark board; the light one doesn't need it */
  .hdj {
    --hdj-rim: #4f7bb0;
  }
  :global(:root[data-mode="light"]) .hdj {
    --hdj-rim: transparent;
  }
  .booth {
    --hdj-h: clamp(104px, 16vh, 150px);
  }
  .big {
    --hdj-h: clamp(170px, 33vh, 300px);
  }
  @container dj (max-width: 699px) {
    .big {
      --hdj-h: clamp(140px, 26vh, 200px);
    }
  }
  .wall {
    position: absolute;
    bottom: 0;
    height: 78%;
    width: calc(50% - var(--hdj-h) * 1.3);
    min-width: 0;
    display: flex;
    align-items: flex-end;
    gap: 5px;
    padding: 0 12px;
    overflow: hidden;
    pointer-events: none;
  }
  .wall.a {
    left: 0;
    --bar: var(--dj-a);
    mask-image: linear-gradient(90deg, #000 40%, transparent);
  }
  .wall.b {
    right: 0;
    --bar: var(--dj-b);
    flex-direction: row-reverse;
    mask-image: linear-gradient(270deg, #000 40%, transparent);
  }
  .wall i {
    flex: 1 1 0;
    max-width: 16px;
    min-width: 4px;
    border-radius: 4px 4px 0 0;
    background: var(--bar);
    opacity: 0.8;
  }
  svg {
    position: relative;
    display: block;
    width: 100%;
    height: 100%;
  }
  .stars {
    animation: twinkle calc(0.7s * var(--motion-speed, 1)) ease-in-out infinite alternate;
    transform-origin: 500px 120px;
  }
  @keyframes twinkle {
    from {
      opacity: 0.55;
    }
    to {
      opacity: 1;
    }
  }
  :global(:root[data-motion="subtle"]) .stars,
  :global(:root[data-motion="off"]) .stars {
    animation: none;
  }
</style>
