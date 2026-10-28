<script lang="ts">
  // Blue glass in the spirit of the desktop players of the early 2000s: an
  // "ambience" of rays around the album, a green pill on the seek groove and
  // glossy orbs. Only the look is borrowed.
  import { formatTime, type Repeat } from "../../lib/api";
  import { seekable } from "../../lib/seek";
  import { useSource } from "../../lib/source.svelte";
  import { ownControls } from "../../lib/layouts";
  import { app } from "../../lib/state.svelte";
  import { visual } from "../../lib/visual.svelte";
  import Cover from "../Cover.svelte";
  import Icon from "../Icon.svelte";
  import SignalChain from "../SignalChain.svelte";
  import Customize from "./Customize.svelte";
  import { rise, swap } from "../../lib/motion";

  const src = useSource();
  // as the skin of the whole app, the player bar has the controls
  const own = $derived(ownControls(app.settings?.room));
  const p = $derived(src.player);
  const chainOn = $derived((app.settings?.room.signal_chain ?? true) && !!p?.stream);
  const track = $derived(src.current);
  const playing = $derived(p?.status === "playing");
  const duration = $derived(p?.duration_ms ?? track?.durationMs ?? 0);
  let scrub = $state<number | null>(null);
  const position = $derived(scrub != null ? scrub * duration : (p?.position_ms ?? 0));
  const f = $derived(duration > 0 ? Math.min(1, position / duration) : 0);
  const nextRepeat: Record<Repeat, Repeat> = { off: "all", all: "one", one: "off" };

  // 64 rays around the album, riding the spectrum
  let canvas: HTMLCanvasElement;
  let side = $state(0);
  $effect(() => {
    const ctx = canvas.getContext("2d")!;
    let raf = 0;
    const draw = () => {
      raf = requestAnimationFrame(draw);
      const dpr = window.devicePixelRatio || 1;
      const w = canvas.clientWidth;
      const h = canvas.clientHeight;
      if (canvas.width !== w * dpr || canvas.height !== h * dpr) {
        canvas.width = w * dpr;
        canvas.height = h * dpr;
      }
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      const cx = w / 2;
      const cy = h / 2;
      const min = Math.min(w, h);
      const bg = ctx.createRadialGradient(cx, cy, 0, cx, cy, Math.max(w, h) / 1.4);
      bg.addColorStop(0, "#1c3aa8");
      bg.addColorStop(1, "#000033");
      ctx.fillStyle = bg;
      ctx.fillRect(0, 0, w, h);
      const s = visual.spectrum;
      const r0 = min * 0.27;
      const n = 64;
      ctx.lineWidth = 4;
      for (let i = 0; i < n; i++) {
        const a = (2 * Math.PI * i) / n;
        const v = s[Math.floor(((i % (n / 2)) * s.length) / (n / 2))] ?? 0;
        const r1 = r0 + 8 + v * min * 0.2;
        ctx.strokeStyle = `rgb(127 212 255 / ${0.35 + v * 0.6})`;
        ctx.beginPath();
        ctx.moveTo(cx + r0 * Math.cos(a), cy + r0 * Math.sin(a));
        ctx.lineTo(cx + r1 * Math.cos(a), cy + r1 * Math.sin(a));
        ctx.stroke();
      }
    };
    raf = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(raf);
  });
</script>

<div class="luna" in:rise>
  <div class="corner"><Customize /></div>
  <div class="screen">
    <div class="ambience" bind:clientHeight={side}>
      <canvas bind:this={canvas}></canvas>
      {#key track?.path}<div class="album" style:width="{side * 0.48}px" in:swap={{ x: 0 }}><Cover path={track?.path} size="100%" radius={999} icon="disc" /></div>{/key}
    </div>
    <div class="status">
      {#key track?.path}<span class="ellipsis" in:swap>{[track?.artist, track?.title].filter(Boolean).join(" : ")}</span>{/key}
      <span class="time">{formatTime(position)}</span>
    </div>
  </div>
  {#if chainOn}<div class="chain-row"><SignalChain look="luna" /></div>{/if}

  {#if own}
  <!-- seek: a groove with the green pill -->
  <div class="seek" use:seekable={{ duration: src.can.seek ? duration : 0, seek: src.seek, onpreview: (v) => (scrub = v) }} role="slider" aria-label="Seek" aria-valuemin={0} aria-valuemax={duration} aria-valuenow={position} tabindex="-1">
    <div class="groove"></div>
    <div class="groove lit" style:width="{f * 100}%"></div>
    <div class="pill" style:left="calc({f} * (100% - 26px))"></div>
  </div>

  <div class="dock">
    <button class="orb" style:--s="64px" onclick={() => src.toggle()} disabled={!src.can.toggle} aria-label={playing ? "Pause" : "Play"} title={playing ? "Pause" : "Play"}><Icon name={playing ? "pause" : "play"} size={30} /></button>
    <button class="orb" style:--s="40px" onclick={() => src.previous()} disabled={!src.can.previous} aria-label="Previous" title="Previous"><Icon name="prev" size={20} /></button>
    <button class="orb" style:--s="40px" onclick={() => src.next()} disabled={!src.can.next} aria-label="Next" title="Next"><Icon name="next" size={20} /></button>
    <span class="grow"></span>
    <button class="orb" class:on={p?.shuffle} style:--s="34px" onclick={() => src.shuffle(!p?.shuffle)} disabled={!src.can.shuffle} aria-label="Shuffle" title="Shuffle"><Icon name="shuffle" size={16} /></button>
    <button class="orb" class:on={p && p.repeat !== "off"} style:--s="34px" onclick={() => p && src.repeat(nextRepeat[p.repeat])} disabled={!src.can.repeat} aria-label="Repeat: {p?.repeat ?? 'off'}" title="Repeat: {p?.repeat ?? 'off'}">
      <Icon name={p?.repeat === "one" ? "repeat1" : "repeat"} size={16} />
    </button>
  </div>
  {/if}
</div>

<style>
  .luna {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    border-radius: 14px;
    background: linear-gradient(180deg, #7fa6ee, #2e5bd0 50%, #1b3a93);
    color: #fff;
  }
  .screen {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    border-radius: 12px;
    border: 2px solid #0b2a80;
    background: #0a1450;
  }
  .ambience {
    position: relative;
    flex: 1;
    min-height: 0;
    display: grid;
    place-items: center;
  }
  canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }
  .album {
    position: relative;
    max-width: 48%;
    aspect-ratio: 1;
    border-radius: 50%;
    overflow: hidden;
    box-shadow: 0 0 40px rgb(127 212 255 / 0.35);
  }
  .corner {
    position: absolute;
    top: 22px;
    right: 22px;
    z-index: 2;
  }
  .chain-row {
    padding: 0 2px;
    min-width: 0;
  }
  .status {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 10px;
    background: #1d2d5c;
    font-size: 13px;
  }
  .status .ellipsis {
    flex: 1;
    min-width: 0;
  }
  .time {
    color: #9ec5ff;
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
  .seek {
    position: relative;
    height: 16px;
  }
  .groove {
    position: absolute;
    left: 0;
    right: 0;
    top: 5px;
    height: 6px;
    border-radius: 3px;
    background: #0e2366;
  }
  .groove.lit {
    right: auto;
    background: #7fd4ff;
  }
  .pill {
    position: absolute;
    top: 1px;
    width: 26px;
    height: 14px;
    border-radius: 7px;
    background: linear-gradient(180deg, #b8f0a8, #2e9a36);
    border: 1px solid #1c5a20;
  }
  .dock {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    border-radius: 40px;
    background: linear-gradient(180deg, #e8eef8, #a9bde0);
    border: 1px solid #5c7cc0;
  }
  .grow {
    flex: 1;
  }
  .orb {
    position: relative;
    flex: none;
    display: grid;
    place-items: center;
    width: var(--s);
    height: var(--s);
    border-radius: 50%;
    color: #14326e;
    background: radial-gradient(circle at 50% 35%, #fff, #9db8e6 55%, #3d6cc8);
    border: 1px solid #2e4e9a;
    overflow: hidden;
  }
  .orb.on {
    background: radial-gradient(circle at 50% 35%, #d6ffd0, #3cb043);
  }
  /* the glossy highlight on the upper half */
  .orb::before {
    content: "";
    position: absolute;
    inset: 10%;
    border-radius: 50%;
    background: linear-gradient(180deg, rgb(255 255 255 / 0.55), transparent 50%);
  }
  .orb :global(svg) {
    position: relative;
  }
  .orb:hover {
    filter: brightness(1.08);
  }
</style>
