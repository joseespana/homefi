<script lang="ts">
  import { ANGER_TIME, Choreographer, REST, TEMPER_CLICKS, TEMPER_WINDOW, angryPose, type DanceId, type Pose, type Specials, type Temper, type Turns } from "../lib/dance";
  import { MASCOT, MASCOT_BAND_WIDTH, MASCOT_COLORS as C, MASCOT_EYES, MASCOT_TILT } from "../lib/mascot";
  import type { Mood } from "../lib/mood";

  /**
   * Dances in `mood` while playing (harder with more `energy`), sleeps otherwise.
   * Click it to start a famous dance (or to say hello while paused); too
   * many clicks too fast and it throws a tantrum (see `temper`).
   */
  let {
    playing = false,
    energy = 0,
    kick = false,
    mood = "bop",
    bpm = 120,
    size = 180,
    dance = "auto",
    specials = "sometimes",
    limbs = "white",
    turns = "all",
    temper = "normal",
    singing = false,
    pose: fixed,
  }: {
    playing?: boolean;
    energy?: number;
    kick?: boolean;
    mood?: Mood;
    bpm?: number;
    size?: number;
    /** `auto` or a famous dance to keep doing. */
    dance?: string;
    specials?: Specials;
    limbs?: "white" | "navy" | "accent" | "off";
    /** When it may turn sideways to show its moves. */
    turns?: Turns;
    /** How many quick clicks it takes to make it angry. */
    temper?: Temper;
    /** Karaoke: sings along into a microphone. */
    singing?: boolean;
    /** Draw this pose instead of dancing (previews). */
    pose?: Pose;
  } = $props();

  const moves = new Choreographer();
  let p = $state<Pose>(REST);
  let current = $state<DanceId | null>(null);
  /** Seconds into a tantrum, or -1 when calm. */
  let anger = $state(-1);
  let angryFrom = 0;
  let angryUntil = 0;
  let clicks: number[] = [];
  const angry = $derived(anger >= 0);

  $effect(() => {
    if (fixed) {
      p = fixed;
      return;
    }
    let raf = 0;
    let last = performance.now();
    const loop = (t: number) => {
      const dt = Math.min(0.1, (t - last) / 1000);
      last = t;
      p = moves.step(dt, { playing, bpm, energy, mood, specials, pinned: dance === "auto" ? null : (dance as DanceId), turns });
      current = moves.dance;
      anger = t < angryUntil ? (t - angryFrom) / 1000 : -1;
      if (angry) p = angryPose(anger);
      // the right hand holds the microphone up to the mouth
      else if (singing && playing) p = { ...p, armR: -110, turn: 0, flip: false };
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  });

  /** Starts a famous dance now, or waves while paused; one click too many and it gets angry. */
  export function poke() {
    const now = performance.now();
    if (now < angryUntil) {
      // poking an angry mascot keeps it angry for longer
      angryUntil = Math.min(angryUntil + 600, now + 2 * ANGER_TIME * 1000);
      return;
    }
    const need = TEMPER_CLICKS[temper] ?? 0;
    if (need) {
      clicks = [...clicks.filter((c) => now - c < TEMPER_WINDOW * 1000), now];
      if (clicks.length >= need) {
        clicks = [];
        angryFrom = now;
        angryUntil = now + ANGER_TIME * 1000;
        return;
      }
    }
    if (playing) moves.trigger(mood);
    else moves.greet();
  }

  // where the limbs join the body (see dance.ts for the angles)
  const SHOULDER = { l: [268, 730], r: [756, 730] } as const;
  const HIP = { l: [420, 800], r: [604, 780] } as const;
  const GROUND = 896;
  const ARM = 130;
  const fill = $derived(limbs === "navy" ? C.navy : limbs === "accent" ? "var(--room-accent, #34d399)" : C.white);
  const outline = $derived(limbs === "white" ? C.navy : "none");
  const bodyTransform = $derived(
    `translate(${(512 + p.x).toFixed(1)} ${(GROUND + p.y).toFixed(1)}) rotate(${p.rot.toFixed(2)}) scale(${(p.flip ? -p.sx : p.sx).toFixed(3)} ${p.sy.toFixed(3)}) translate(-512 -${GROUND})`,
  );

  // Turning sideways, drawn as if the head were a cylinder: the face and the
  // headphone cups slide round it, the house gets narrower (its side is not as
  // wide as its front) and the limbs on the far side go behind the body.
  const HEAD_R = 270;
  const th = $derived((p.turn * Math.PI) / 2);
  const side = $derived(Math.sin(th));
  const across = $derived(Math.cos(th));
  const narrow = $derived(1 - 0.2 * Math.abs(side));
  // headphones sit a little behind the middle of the head when seen from the side
  const back = $derived(-46 * side);
  const squash = $derived(`translate(512 0) scale(${narrow.toFixed(3)} 1) translate(-512 0)`);
  // the face turns less than the body so a profile still shows one eye
  const faceTh = $derived((p.turn * 55 * Math.PI) / 180);
  const faceTransform = $derived(
    `translate(${(512 + 210 * Math.sin(faceTh)).toFixed(1)} 0) scale(${Math.cos(faceTh).toFixed(3)} 1) translate(-512 0)`,
  );
  const topTransform = $derived(
    `translate(${(512 + 110 * Math.sin(faceTh)).toFixed(1)} 0) scale(${(1 - 0.6 * (1 - Math.cos(faceTh))).toFixed(3)} 1) translate(-512 0)`,
  );
  const clipId = `mascot-face-${Math.random().toString(36).slice(2, 9)}`;
  const [cupL, cupR] = MASCOT.cups.split("ZM").map((d, i) => (i ? `M${d}` : `${d}Z`));
  const [padL, padR] = MASCOT.pads.split("ZM").map((d, i) => (i ? `M${d}` : `${d}Z`));
  /** Left / right cup: centre, how far it is towards you (-1 behind, 1 facing you) and its shapes. */
  const cups = $derived(
    [
      { home: 242, x: 512 + back - HEAD_R * narrow * across, depth: side, cup: cupL, pad: padL },
      { home: 782, x: 512 + back + HEAD_R * narrow * across, depth: -side, cup: cupR, pad: padR },
    ].map((c) => ({
      ...c,
      transform: `translate(${c.x.toFixed(1)} 0) scale(${(1 + 0.45 * Math.max(0, c.depth) * Math.abs(side)).toFixed(3)} 1) translate(-${c.home} 0)`,
    })),
  );
  // The head band, split at its top: each half goes with its cup, so the half
  // reaching the far cup passes behind the head instead of across the face.
  const bandHalves = $derived.by(() => {
    const l = cups[0].x - 16;
    const r = cups[1].x + 16;
    const m = (l + r) / 2;
    const f = (v: number) => v.toFixed(1);
    return [
      { d: `M${f(l)} 560C${f(l)} 344 ${f((3 * l + r) / 4)} 236 ${f(m)} 236`, depth: cups[0].depth },
      { d: `M${f(m)} 236C${f((l + 3 * r) / 4)} 236 ${f(r)} 344 ${f(r)} 560`, depth: cups[1].depth },
    ];
  });
  const bandWidth = $derived(MASCOT_BAND_WIDTH * (1 - 0.45 * Math.abs(side)));
  /**
   * Where a limb joins the body and how it is rotated. `front` is the
   * rotation direction when facing you (+1 left, -1 right); in profile both
   * swing towards where it faces.
   */
  function limb(x: number, angle: number, front: 1 | -1) {
    const towards = side === 0 ? front : -Math.sign(side);
    const s = Math.abs(side);
    return {
      x: 512 + (x - 512) * narrow * across + 18 * side,
      rot: angle * (front + (towards - front) * s),
      foot: front === 1 ? -1 + (Math.sign(side) + 1) * s : 1 + (Math.sign(side) - 1) * s,
      depth: front * side,
    };
  }
  const armList = $derived(
    [
      { ...limb(SHOULDER.l[0], p.armL, 1), y: SHOULDER.l[1], mic: false },
      { ...limb(SHOULDER.r[0], p.armR, -1), y: SHOULDER.r[1], mic: singing && playing && !angry },
    ].sort((a, b) => a.depth - b.depth),
  );
  const legList = $derived(
    [
      { ...limb(HIP.l[0], p.legL, 1), y: HIP.l[1], lift: p.liftL },
      { ...limb(HIP.r[0], p.legR, -1), y: HIP.r[1], lift: p.liftR },
    ].sort((a, b) => a.depth - b.depth),
  );
  /** Limbs this far round go behind the body; cups and band as soon as they face away. */
  const BEHIND = -0.3;

  // 0.5 (quiet) .. 1.3 (loud)
  const k = $derived(playing ? 0.5 + Math.min(1, energy * 1.6) * 0.8 : 0);
  const [lx, ly] = MASCOT_EYES.left;
  const [rx, ry] = MASCOT_EYES.right;
  const r = MASCOT_EYES.r;

  const closedEyes = `M${lx - r} ${ly}Q${lx} ${ly + r * 0.9} ${lx + r} ${ly}M${rx - r} ${ry}Q${rx} ${ry + r * 0.9} ${rx + r} ${ry}`;
  const happyEyes = `M${lx - r} ${ly + 8}Q${lx} ${ly - r * 0.9} ${lx + r} ${ly + 8}M${rx - r} ${ry + 8}Q${rx} ${ry - r * 0.9} ${rx + r} ${ry + 8}`;
  const coolEyes = `M${lx - r} ${ly}A${r} ${r} 0 0 0 ${lx + r} ${ly}ZM${rx - r} ${ry}A${r} ${r} 0 0 0 ${rx + r} ${ry}Z`;
  const heart = (x: number, y: number, s: number) =>
    `M${x} ${y + s * 0.3}C${x} ${y - s * 0.2} ${x - s} ${y - s * 0.2} ${x - s} ${y + s * 0.35}C${x - s} ${y + s * 0.8} ${x} ${y + s * 1.1} ${x} ${y + s * 1.3}C${x} ${y + s * 1.1} ${x + s} ${y + s * 0.8} ${x + s} ${y + s * 0.35}C${x + s} ${y - s * 0.2} ${x} ${y - s * 0.2} ${x} ${y + s * 0.3}Z`;
  const star = (x: number, y: number, r: number, inner = 0.45) =>
    "M" +
    Array.from({ length: 10 }, (_, i) => {
      const a = (Math.PI / 5) * i - Math.PI / 2;
      const d = i % 2 ? r * inner : r;
      return `${(x + d * Math.cos(a)).toFixed(1)} ${(y + d * Math.sin(a)).toFixed(1)}`;
    }).join("L") +
    "Z";
  // points along the collar / necklace line at the bottom of the house
  const spikes = Array.from({ length: 9 }, (_, i) => 300 + i * 53);
  // red in the face, pulsing; fades in at the start
  const flush = $derived(angry ? Math.min(1, anger / 0.3) * (0.22 + 0.1 * Math.sin(2 * Math.PI * 3 * anger)) : 0);
  const vein = $derived(angry ? 1 + 0.18 * Math.abs(Math.sin(2 * Math.PI * 2.5 * anger)) : 1);
  /** Puffs of steam out of each headphone cup: x, y, radius, opacity. */
  const steam = $derived(
    angry
      ? [0, 1, 2].flatMap((i) => {
          const f = (anger * 1.4 + i / 3) % 1;
          const fade = Math.min(1, anger / 0.4) * 0.85 * (1 - f);
          return [
            { x: 200 - 70 * f, y: 520 - 230 * f, r: 22 + 40 * f, o: fade },
            { x: 824 + 70 * f, y: 520 - 230 * f, r: 22 + 40 * f, o: fade },
          ];
        })
      : [],
  );
  const bolt = (x: number, y: number, s: number) =>
    `M${x} ${y}L${x - s * 0.5} ${y + s * 1.1}L${x - s * 0.05} ${y + s * 1.1}L${x - s * 0.35} ${y + s * 2}L${x + s * 0.55} ${y + s * 0.75}L${x + s * 0.08} ${y + s * 0.75}L${x + s * 0.35} ${y}Z`;
</script>

{#snippet armShape(arm: { x: number; y: number; rot: number; mic: boolean })}
  <g transform="translate({arm.x.toFixed(1)} {arm.y}) rotate({arm.rot.toFixed(1)})">
    {#if outline !== "none"}
      <line y2={ARM} stroke={outline} stroke-width="72" stroke-linecap="round" />
      <circle cy={ARM} r="46" fill={outline} />
    {/if}
    <line y2={ARM} stroke={fill} stroke-width="56" stroke-linecap="round" />
    <circle cy={ARM} r="38" fill={fill} />
    {#if arm.mic}
      <!-- microphone, pointing at the mouth -->
      <rect x="-15" y={ARM - 6} width="30" height="78" rx="12" fill="#374151" />
      <rect x="-19" y={ARM + 58} width="38" height="14" rx="5" fill="#9ca3af" />
      <circle cy={ARM + 100} r="36" fill="#d1d5db" stroke="#6b7280" stroke-width="6" />
      <path d="M-30 {ARM + 92}H30M-30 {ARM + 108}H30M-12 {ARM + 66}V{ARM + 134}M12 {ARM + 66}V{ARM + 134}" stroke="#6b7280" stroke-width="4" opacity="0.7" />
    {/if}
  </g>
{/snippet}

{#snippet bandShape(d: string)}
  <path {d} fill="none" stroke={C.navy} stroke-width={bandWidth} stroke-linecap="round" />
  {#if mood === "party"}
    <!-- LEDs on the head band -->
    <path {d} fill="none" stroke="#5eead4" stroke-width="10" stroke-linecap="round" stroke-dasharray="6 34" class="neon leds" />
  {/if}
{/snippet}

{#snippet cupShape(c: { home: number; transform: string; cup: string; pad: string })}
  <g transform={c.transform}>
    <path d={c.cup} fill={C.navy} />
    <path d={c.pad} fill={C.cushion} />
    {#if mood === "salsa"}
      <!-- gold hoop, hanging from the ear -->
      <circle cx={c.home} cy="738" r="26" fill="none" stroke="#fbbf24" stroke-width="9" />
    {/if}
  </g>
{/snippet}

<!-- worn round the body: stays put when it turns -->
{#snippet bodyDecor()}
  {#if mood === "mosh"}
    <!-- studded collar -->
    <rect x="262" y="744" width="500" height="40" rx="8" fill="#111827" />
    {#each spikes as x}<path d="M{x - 14} 744L{x} 712L{x + 14} 744Z" fill="#d1d5db" /><circle cx={x} cy="764" r="7" fill="#d1d5db" />{/each}
  {:else if mood === "pogo"}
    <!-- safety pin -->
    <g transform="rotate(-20 330 760)" fill="none" stroke="#cbd5e1" stroke-width="7" stroke-linecap="round">
      <path d="M300 760H372Q388 760 388 746Q388 732 372 732H312" />
      <circle cx="298" cy="760" r="8" />
    </g>
  {:else if mood === "party"}
    <!-- glow-stick necklace -->
    <path d="M290 770Q512 830 734 770" fill="none" stroke="#e879f9" stroke-width="14" stroke-linecap="round" class="neon" />
  {:else if mood === "groove"}
    <!-- gold chain + medallion -->
    <path d="M320 744Q512 820 704 744" fill="none" stroke="#fbbf24" stroke-width="10" stroke-dasharray="14 6" stroke-linecap="round" />
    <circle cx="512" cy="790" r="28" fill="#fbbf24" stroke="#b45309" stroke-width="5" />
    <path d="M504 774V800M504 774L524 768V794" fill="none" stroke="#b45309" stroke-width="5" stroke-linecap="round" />
  {/if}
{/snippet}

<!-- on top of the head: turns with the face, a little less -->
{#snippet topDecor()}
  {#if mood === "mosh"}
    <!-- devil horns -->
    <path d="M392 262Q340 200 356 128Q384 196 440 226Z" fill="#dc2626" stroke="#7f1d1d" stroke-width="6" stroke-linejoin="round" />
    <path d="M632 262Q684 200 668 128Q640 196 584 226Z" fill="#dc2626" stroke="#7f1d1d" stroke-width="6" stroke-linejoin="round" />
  {:else if mood === "pogo"}
    <!-- mohawk -->
    <path d="M432 224L436 60L478 206L496 10L524 204L552 26L568 208L600 76L594 228Z" fill="#22c55e" stroke="#15803d" stroke-width="6" stroke-linejoin="round" />
  {:else if mood === "cute"}
    <!-- bow -->
    <g transform="rotate(14 640 372)">
      <path d="M640 372L580 330Q560 372 580 414Z" fill="#f472b6" />
      <path d="M640 372L700 330Q720 372 700 414Z" fill="#f472b6" />
      <circle cx="640" cy="372" r="18" fill="#ec4899" />
    </g>
  {:else if mood === "salsa"}
    <!-- hibiscus -->
    <g transform="translate(372 396)">
      {#each [0, 72, 144, 216, 288] as a}<ellipse cx="0" cy="-34" rx="24" ry="38" fill="#ef4444" transform="rotate({a})" />{/each}
      <circle r="14" fill="#fde047" />
    </g>
  {:else if mood === "groove"}
    <!-- backwards cap -->
    <path d="M372 230Q380 120 512 112Q644 120 652 230Z" fill="#dc2626" />
    <path d="M372 230Q300 236 268 212Q300 196 380 204Z" fill="#991b1b" />
    <circle cx="512" cy="114" r="10" fill="#991b1b" />
  {:else if mood === "chill"}
    <!-- beret -->
    <ellipse cx="530" cy="214" rx="150" ry="44" fill="#1e3a8a" transform="rotate(-8 530 214)" />
    <rect x="522" y="156" width="14" height="22" rx="6" fill="#1e3a8a" />
  {/if}
{/snippet}

<!-- the face: slides round the head when it turns -->
{#snippet face()}
  <!-- makeup, under the eyes -->
  {#if mood === "mosh"}
    <!-- Aladdin Sane bolt across the left eye -->
    <path d="M392 500L470 500L438 590L494 590L388 726L416 620L366 620Z" fill="#ef4444" opacity="0.92" />
    <path d="M404 512L452 512L426 584" fill="none" stroke="#38bdf8" stroke-width="8" stroke-linejoin="round" />
  {:else if mood === "pogo"}
    <path d="M392 552Q440 530 488 552M536 552Q584 530 632 552" fill="none" stroke={C.navy} stroke-width="12" stroke-linecap="round" />
    <!-- X plaster -->
    <g transform="rotate(30 660 700)">
      <rect x="620" y="688" width="80" height="24" rx="8" fill="#fcd34d" />
      <rect x="648" y="660" width="24" height="80" rx="8" fill="#fcd34d" />
    </g>
  {:else if mood === "cute"}
    <path d={star(372, 628, 14)} fill="#f9a8d4" />
    <path d={star(652, 628, 14)} fill="#f9a8d4" />
    <path d={star(396, 720, 9)} fill="#fde047" />
    <path d={star(628, 720, 9)} fill="#fde047" />
  {:else if mood === "party"}
    <!-- UV face paint -->
    {#each [[380, 660], [400, 690], [360, 690], [644, 660], [624, 690], [664, 690]] as [x, y], i}
      <circle cx={x} cy={y} r="9" fill={i % 2 ? "#e879f9" : "#5eead4"} class="neon" />
    {/each}
    <path d="M470 760L512 744L554 760" fill="none" stroke="#5eead4" stroke-width="8" stroke-linecap="round" class="neon" />
  {:else if mood === "salsa"}
    <path d="M400 560Q440 536 480 560M544 560Q584 536 624 560" fill="none" stroke="#7c2d12" stroke-width="10" stroke-linecap="round" />
  {/if}

  {#if angry}
    <!-- frowning, eyes narrowed, teeth clenched -->
    <path d="M396 536L480 574M628 536L544 574" stroke={C.navy} stroke-width="20" stroke-linecap="round" />
    <path d="M{lx - r * 0.85} {ly - 2}L{lx + r * 0.85} {ly + 10}A{r * 0.85} {r * 0.85} 0 0 1 {lx - r * 0.85} {ly - 2}ZM{rx + r * 0.85} {ry - 2}L{rx - r * 0.85} {ry + 10}A{r * 0.85} {r * 0.85} 0 0 0 {rx + r * 0.85} {ry - 2}Z" fill={C.navy} />
    <rect x="458" y="682" width="108" height="44" rx="12" fill={C.white} stroke={C.navy} stroke-width="9" />
    <path d="M462 704H562M485 684V724M512 684V724M539 684V724" stroke={C.navy} stroke-width="6" />
  {:else if singing && playing}
    <!-- singing its heart out: eyes shut, mouth opening with the music -->
    <path d={happyEyes} fill="none" stroke={C.navy} stroke-width="12" stroke-linecap="round" />
    <ellipse cx="512" cy="704" rx={26 + 8 * Math.min(1, energy * 1.6)} ry={10 + 30 * Math.min(1, energy * 1.8)} fill={C.navy} />
    <ellipse cx="512" cy={712 + 18 * Math.min(1, energy * 1.8)} rx="16" ry={4 + 8 * Math.min(1, energy * 1.8)} fill="#f472b6" />
  {:else if !playing}
    <path d={closedEyes} fill="none" stroke={C.navy} stroke-width="12" stroke-linecap="round" />
    <ellipse cx="512" cy="690" rx="18" ry="12" fill={C.navy} />
  {:else if mood === "mosh"}
    <path d="M398 548L478 578M626 548L546 578" stroke={C.navy} stroke-width="18" stroke-linecap="round" />
    <circle cx={lx} cy={ly + 8} r={r * 0.7} fill={C.navy} />
    <circle cx={rx} cy={ry + 8} r={r * 0.7} fill={C.navy} />
    <ellipse cx="512" cy="704" rx="46" ry="40" fill={C.navy} />
    <ellipse cx="512" cy="724" rx="26" ry="14" fill="#f472b6" />
  {:else if mood === "party"}
    <!-- sunglasses -->
    <rect x={lx - r - 22} y={ly - 30} width={r * 2 + 44} height="62" rx="22" fill="#05070d" />
    <rect x={rx - r - 22} y={ry - 30} width={r * 2 + 44} height="62" rx="22" fill="#05070d" />
    <path d="M{lx + r + 22} {ly - 10}L{rx - r - 22} {ry - 10}" stroke="#05070d" stroke-width="14" />
    <path d="M{lx - r - 4} {ly - 14}L{lx - 6} {ly - 14}M{rx - r - 4} {ry - 14}L{rx - 6} {ry - 14}" stroke="#5eead4" stroke-width="8" stroke-linecap="round" opacity="0.8" />
    <path d={MASCOT.smile} fill={C.navy} />
  {:else if mood === "groove"}
    <path d={coolEyes} fill={C.navy} />
    <path d="M{lx - r} {ly}L{lx + r} {ly}M{rx - r} {ry}L{rx + r} {ry}" stroke={C.navy} stroke-width="10" stroke-linecap="round" />
    <path d="M470 690Q512 712 560 684" fill="none" stroke={C.navy} stroke-width="14" stroke-linecap="round" />
  {:else if mood === "chill"}
    <path d={happyEyes} fill="none" stroke={C.navy} stroke-width="12" stroke-linecap="round" />
    <path d="M478 688Q512 712 546 688" fill="none" stroke={C.navy} stroke-width="12" stroke-linecap="round" />
  {:else}
    <!-- cute, pogo, salsa: big shiny eyes -->
    <g class="eyes">
      <path d={MASCOT.eyes} fill={C.navy} />
      <path d={MASCOT.sparkles} fill={C.white} />
      {#if mood === "cute"}
        <circle cx={lx - 12} cy={ly + 14} r="6" fill={C.white} />
        <circle cx={rx - 12} cy={ry + 14} r="6" fill={C.white} />
      {/if}
    </g>
    {#if mood === "pogo"}
      <ellipse cx="512" cy="700" rx="30" ry="28" fill={C.navy} />
    {:else}
      <path d={MASCOT.smile} fill={C.navy} />
    {/if}
  {/if}
  <path d={MASCOT.cheeks} fill={C.cheek} opacity={playing && mood === "cute" ? 0.9 : 0.6} />

  {#if mood === "cute"}
    <!-- lashes -->
    <path d="M402 582L388 568M414 572L406 554M626 572L634 554M638 582L652 568" stroke={C.navy} stroke-width="7" stroke-linecap="round" />
  {:else if mood === "salsa"}
    <!-- red lips -->
    <path d={MASCOT.smile} fill="#dc2626" />
  {:else if mood === "chill"}
    <!-- round glasses and a bow tie -->
    <g fill="none" stroke={C.navy} stroke-width="8">
      <circle cx={lx} cy={ly} r="50" />
      <circle cx={rx} cy={ry} r="50" />
      <path d="M{lx + 50} {ly}Q512 {ly - 16} {rx - 50} {ry}" />
    </g>
    <path d="M512 784L456 756V812ZM512 784L568 756V812Z" fill="#b91c1c" />
    <circle cx="512" cy="784" r="12" fill="#7f1d1d" />
  {/if}
{/snippet}

<div
  class="mascot mood-{mood}"
  class:playing
  class:kick
  style:width="{size}px"
  style:--k={k}
  style:--beat="{(60 / bpm).toFixed(3)}s"
  role="img"
  aria-label={angry ? "HomeFi is angry" : singing && playing ? "HomeFi singing along" : playing ? `HomeFi dancing (${current ?? mood})` : "HomeFi sleeping"}
>
  <svg viewBox="40 40 1000 900">
    <!-- things floating around, depending on the mood -->
    {#if playing && !angry}
      <g class="fx">
        {#if mood === "mosh"}
          <path d={bolt(880, 150, 90)} fill="#fde047" />
          <path d={bolt(150, 220, 70)} fill="#fde047" />
          <path d="M120 520q-40 60 0 120M80 500q-60 90 0 180M904 520q40 60 0 120M944 500q60 90 0 180" fill="none" stroke="#fde047" stroke-width="16" stroke-linecap="round" opacity="0.8" />
        {:else if mood === "cute"}
          <path d={heart(870, 160, 60)} fill="#f472b6" />
          <path d={heart(160, 250, 42)} fill="#f9a8d4" />
          <path d={heart(950, 420, 30)} fill="#f472b6" />
        {:else}
          <path d={MASCOT.notes} fill={mood === "party" ? "#5eead4" : C.note} class:neon={mood === "party"} />
        {/if}
      </g>
    {/if}

    <g transform={bodyTransform}>
      {#if limbs !== "off"}
        <!-- legs and feet, behind the house; toes point where it faces -->
        {#each legList as leg}
          <g transform="translate({leg.x.toFixed(1)} {leg.y - leg.lift}) rotate({leg.rot.toFixed(1)})">
            {#if outline !== "none"}
              <line y2={GROUND - leg.y} stroke={outline} stroke-width="78" stroke-linecap="round" />
              <ellipse cx={leg.foot * 20} cy={GROUND - leg.y} rx="58" ry="36" fill={outline} />
            {/if}
            <line y2={GROUND - leg.y} stroke={fill} stroke-width="62" stroke-linecap="round" />
            <ellipse cx={leg.foot * 20} cy={GROUND - leg.y} rx="50" ry="28" fill={fill} />
          </g>
        {/each}
      {/if}
      <g transform="rotate({MASCOT_TILT.deg} {MASCOT_TILT.x} {MASCOT_TILT.y})">
        <defs>
          <clipPath id={clipId}><path d={MASCOT.house} /></clipPath>
        </defs>
        {#if limbs !== "off"}
          {#each armList.filter((a) => a.depth < BEHIND) as arm}
            {@render armShape(arm)}
          {/each}
        {/if}
        {#each bandHalves.filter((h) => h.depth < 0) as h}
          {@render bandShape(h.d)}
        {/each}
        {#each cups.filter((c) => c.depth < 0) as c}
          {@render cupShape(c)}
        {/each}

        <g transform={squash}>
          <path d={MASCOT.house} fill={C.white} />
          {#if angry}<path d={MASCOT.house} fill="#ef4444" opacity={flush} />{/if}
          {@render bodyDecor()}
          <g clip-path="url(#{clipId})">
            <g transform={faceTransform}>
              {@render face()}
            </g>
          </g>
        </g>

        {#each bandHalves.filter((h) => h.depth >= 0) as h}
          {@render bandShape(h.d)}
        {/each}
        {#each cups.filter((c) => c.depth >= 0) as c}
          {@render cupShape(c)}
        {/each}
        <g transform={squash}>
          <g transform={topTransform}>
            {@render topDecor()}
            {#if angry}
              <!-- the anime anger vein, throbbing -->
              <path
                d="M-34 -10Q-12 -12 -10 -34M10 -34Q12 -12 34 -10M34 10Q12 12 10 34M-10 34Q-12 12 -34 10"
                transform="translate(652 420) scale({vein.toFixed(3)})"
                fill="none"
                stroke="#dc2626"
                stroke-width="12"
                stroke-linecap="round"
              />
            {/if}
          </g>
        </g>
        {#if limbs !== "off"}
          <!-- arms with mitten hands, in front so raised arms stay visible -->
          {#each armList.filter((a) => a.depth >= BEHIND) as arm}
            {@render armShape(arm)}
          {/each}
        {/if}
      </g>
    </g>
    {#each steam as puff}
      <circle cx={puff.x.toFixed(1)} cy={puff.y.toFixed(1)} r={puff.r.toFixed(1)} fill="#e5e7eb" stroke={C.navy} stroke-opacity="0.18" stroke-width="5" opacity={puff.o.toFixed(2)} />
    {/each}
  </svg>
  {#if !playing && !angry}
    <span class="z z1">z</span><span class="z z2">z</span><span class="z z3">Z</span>
  {/if}
</div>

<style>
  .mascot {
    position: relative;
    aspect-ratio: 1000 / 900;
    flex: none;
    --k: 1;
    --beat: 0.5s;
  }
  svg {
    width: 100%;
    height: 100%;
    overflow: visible;
    filter: drop-shadow(0 14px 24px rgb(0 0 0 / 0.35));
  }

  .playing .leds {
    animation: chase calc(var(--beat) * 2) linear infinite;
  }
  @keyframes chase {
    to {
      stroke-dashoffset: -80;
    }
  }
  /* bass hit: a little squash */
  .kick svg {
    animation: hit 0.16s ease-out;
  }
  @keyframes hit {
    50% {
      transform: scale(1.04, 0.96);
    }
  }

  .fx {
    animation: float 2.2s ease-in-out infinite;
  }
  .mood-mosh .fx {
    animation: shake 0.21s steps(2) infinite;
  }
  @keyframes float {
    0%,
    100% {
      transform: translate(0, 0);
    }
    50% {
      transform: translate(10px, -26px);
    }
  }
  @keyframes shake {
    from {
      transform: translate(-6px, 4px);
    }
    to {
      transform: translate(6px, -4px);
    }
  }
  .neon {
    animation: neon 1.6s linear infinite;
  }
  @keyframes neon {
    to {
      filter: hue-rotate(360deg);
    }
  }
  .eyes {
    animation: blink 4.2s infinite;
    transform-box: fill-box;
    transform-origin: center;
  }
  @keyframes blink {
    0%,
    94%,
    100% {
      transform: scaleY(1);
    }
    97% {
      transform: scaleY(0.1);
    }
  }
  .z {
    position: absolute;
    right: 18%;
    top: 12%;
    font: 800 20px ui-rounded, system-ui, sans-serif;
    color: var(--muted);
    opacity: 0;
    animation: zz 3s ease-in infinite;
  }
  .z2 {
    animation-delay: 1s;
    font-size: 26px;
  }
  .z3 {
    animation-delay: 2s;
    font-size: 32px;
  }
  @keyframes zz {
    0% {
      opacity: 0;
      transform: translate(0, 10px);
    }
    30% {
      opacity: 0.9;
    }
    100% {
      opacity: 0;
      transform: translate(26px, -50px);
    }
  }
</style>
