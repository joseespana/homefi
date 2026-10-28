<script lang="ts">
  // A skin in the spirit of the players of the late 90s: bevelled panels, a green
  // LCD, faders riding the spectrum and the playlist. Only the look is borrowed.
  import { formatTime, type Repeat } from "../../lib/api";
  import { liveBands } from "../../lib/bands.svelte";
  import { ownControls, qualityParts } from "../../lib/layouts";
  import { seekable } from "../../lib/seek";
  import { useSource } from "../../lib/source.svelte";
  import { app } from "../../lib/state.svelte";
  import { visual } from "../../lib/visual.svelte";
  import Icon from "../Icon.svelte";
  import SignalChain from "../SignalChain.svelte";
  import Customize from "./Customize.svelte";
  import { rise } from "../../lib/motion";

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
  const q = $derived(qualityParts(track?.quality));
  const index = $derived(p?.current ?? -1);
  const nextRepeat: Record<Repeat, Repeat> = { off: "all", all: "one", one: "off" };
  const EQ = ["60", "170", "310", "600", "1K", "3K", "6K", "12K", "14K", "16K"];
  const faders = liveBands(EQ.length);

  // the LCD's little spectrum: 19 columns of 8 LEDs
  let canvas: HTMLCanvasElement;
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
      ctx.clearRect(0, 0, w, h);
      const s = visual.spectrum;
      const n = 19;
      const segs = 8;
      const bw = w / n;
      const sh = h / segs;
      for (let i = 0; i < n; i++) {
        const a = Math.floor((i * s.length) / n);
        const v = ((s[a] ?? 0) + (s[Math.min(s.length - 1, a + 1)] ?? 0)) / 2;
        const lit = Math.floor(v * segs);
        for (let k = 0; k < lit; k++) {
          ctx.fillStyle = k >= 6 ? "#e03020" : k >= 4 ? "#e0d020" : "#20c020";
          ctx.fillRect(i * bw + 1, h - (k + 1) * sh + 1, bw - 2, sh - 2);
        }
      }
    };
    raf = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(raf);
  });

  // the playlist keeps the song playing in view
  let list: HTMLDivElement;
  $effect(() => {
    const i = index;
    if (i < 0 || !list) return;
    const row = list.children[Math.max(0, i - 2)] as HTMLElement | undefined;
    list.scrollTo({ top: row?.offsetTop ?? 0, behavior: "smooth" });
  });
</script>

<div class="amp" in:rise>
  <section class="panel">
    <div class="titlebar"><span>H O M E F I</span><Customize /></div>
    <div class="body">
      <div class="row">
        <div class="lcd sunken">
          <div class="clock">
            <Icon name={playing ? "play" : "pause"} size={14} />
            <span>{formatTime(position)}</span>
          </div>
          <canvas bind:this={canvas}></canvas>
        </div>
        <div class="right">
          <div class="marquee sunken">
            <!-- a new song starts its scroll over -->
            {#key track?.path}<span>{index + 1}. {track?.artist ?? ""} - {track?.title ?? ""} ({formatTime(duration)})</span>{/key}
          </div>
          <div class="specs">
            <span class="box sunken">{q.bits}</span><span class="unit">bit</span>
            <span class="box sunken">{q.khz}</span><span class="unit">kHz</span>
          </div>
          <div class="stereo">stereo</div>
        </div>
      </div>
      {#if chainOn}<div class="chain-row"><SignalChain look="lcd" /></div>{/if}

      {#if own}
      <div class="seek sunken" use:seekable={{ duration: src.can.seek ? duration : 0, seek: src.seek, onpreview: (v) => (scrub = v) }} role="slider" aria-label="Seek" aria-valuemin={0} aria-valuemax={duration} aria-valuenow={position} tabindex="-1">
        <div class="thumb raised-gold" style:left="calc({f} * (100% - 28px))"></div>
      </div>

      <div class="buttons">
        <button class="btn raised" onclick={() => src.previous()} disabled={!src.can.previous} aria-label="Previous" title="Previous"><Icon name="prev" size={18} /></button>
        <button class="btn raised" onclick={() => !playing && src.toggle()} disabled={!src.can.toggle} aria-label="Play" title="Play"><Icon name="play" size={18} /></button>
        <button class="btn raised" onclick={() => playing && src.toggle()} disabled={!src.can.toggle} aria-label="Pause" title="Pause"><Icon name="pause" size={18} /></button>
        <button class="btn raised" onclick={() => src.next()} disabled={!src.can.next} aria-label="Next" title="Next"><Icon name="next" size={18} /></button>
        <span class="grow"></span>
        <button class="toggle raised" onclick={() => src.shuffle(!p?.shuffle)} disabled={!src.can.shuffle}><i class:on={p?.shuffle}></i>SHUF</button>
        <button class="toggle raised" onclick={() => p && src.repeat(nextRepeat[p.repeat])} disabled={!src.can.repeat}><i class:on={p && p.repeat !== "off"}></i>{p?.repeat === "one" ? "REP1" : "REP"}</button>
      </div>
      {/if}
    </div>
  </section>

  <section class="panel">
    <div class="titlebar"><span>S P E C T R U M</span></div>
    <div class="body eq">
      {#each EQ as label, i}
        <div class="fader">
          <div class="slot">
            <div class="knob raised-green" style:top="calc({1 - (faders.values[i] ?? 0)} * (100% - 10px))"></div>
          </div>
          <span>{label}</span>
        </div>
      {/each}
    </div>
  </section>

  <section class="panel grow-panel">
    <div class="titlebar"><span>P L A Y L I S T</span></div>
    <div class="body list-body">
      <div class="playlist" bind:this={list}>
        {#each p?.queue ?? [] as path, i (i)}
          {@const t = src.trackAt(path)}
          <button class="entry" class:current={i === index} onclick={() => src.jump(i)} disabled={!src.can.jump && i !== index}>
            {i + 1}. {[t?.artist, t?.title ?? path.split(/[\\/]/).pop()].filter(Boolean).join(" - ")}
          </button>
        {/each}
        <!-- the rest of a Tidal stream, fetched just before each one plays -->
        {#if !src.remote}
          {#each app.upcoming as u, j (u.id + j)}
            <button class="entry later" onclick={() => app.playUpcoming(j)} title="Play now (from Tidal)">
              {(p?.queue.length ?? 0) + j + 1}. {[u.artist, u.title].filter(Boolean).join(" - ")}
            </button>
          {/each}
        {/if}
      </div>
    </div>
  </section>
</div>

<style>
  .amp {
    --face: #2e2e40;
    --light: #5c5c78;
    --dark: #12121a;
    --green: #00e000;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 8px;
    border-radius: 6px;
    background: linear-gradient(180deg, #34344a, #1e1e2c);
    color: #cfcfe0;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }
  .panel {
    padding: 3px;
    background: var(--face);
    border: 3px solid;
    border-color: var(--light) var(--dark) var(--dark) var(--light);
  }
  .grow-panel {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .titlebar {
    position: relative;
    height: 18px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: linear-gradient(90deg, #3a3a55, #8a7a3a, #3a3a55);
    color: #e8e0b0;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 2px;
  }
  .titlebar :global(.customize) {
    position: absolute;
    right: 2px;
    top: 0;
    font-family: var(--font);
    letter-spacing: 0;
  }
  .titlebar :global(.chip) {
    padding: 1px 8px;
    font-size: 10px;
  }
  .body {
    padding: 6px;
  }
  .sunken {
    background: #000;
    border: 2px solid;
    border-color: var(--dark) var(--light) var(--light) var(--dark);
  }
  .raised {
    background: #b8b8c8;
    color: #22222e;
    border: 2px solid;
    border-color: #f0f0fa #55556a #55556a #f0f0fa;
  }
  .raised:active {
    border-color: #55556a #f0f0fa #f0f0fa #55556a;
  }
  .row {
    display: flex;
    gap: 6px;
  }
  .lcd {
    width: 168px;
    flex: none;
    padding: 6px;
    color: var(--green);
  }
  .clock {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 34px;
    font-weight: 700;
    line-height: 1.1;
    text-shadow: 0 0 8px rgb(0 224 0 / 0.6);
    font-variant-numeric: tabular-nums;
  }
  canvas {
    display: block;
    width: 100%;
    height: 30px;
  }
  .right {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .marquee {
    overflow: hidden;
    white-space: nowrap;
    padding: 4px 6px;
    color: var(--green);
    font-size: 12px;
  }
  .marquee span {
    display: inline-block;
    padding-left: 100%;
    animation: marquee 14s linear infinite;
  }
  @keyframes marquee {
    to {
      transform: translateX(-100%);
    }
  }
  .chain-row {
    margin-top: 4px;
    min-width: 0;
  }
  .specs {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .box {
    padding: 1px 4px;
    color: var(--green);
    font-size: 11px;
    font-weight: 700;
  }
  .unit {
    margin-right: 6px;
    font-size: 10px;
    color: #b0b0c8;
  }
  .stereo {
    align-self: flex-end;
    color: var(--green);
    font-size: 10px;
  }
  .seek {
    position: relative;
    height: 16px;
    margin: 8px 0;
    background: #1a1a26;
  }
  .thumb {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 28px;
  }
  .raised-gold {
    background: #c8a848;
    border: 2px solid;
    border-color: #f0e0a0 #6a5418 #6a5418 #f0e0a0;
  }
  .buttons {
    display: flex;
    align-items: center;
    gap: 3px;
  }
  .btn {
    width: 40px;
    height: 30px;
    display: grid;
    place-items: center;
  }
  .grow {
    flex: 1;
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 5px 6px;
    font: 700 10px ui-monospace, SFMono-Regular, Menlo, monospace;
  }
  .toggle i {
    width: 6px;
    height: 6px;
    background: #2a4a2a;
  }
  .toggle i.on {
    background: #30f030;
  }
  .eq {
    display: flex;
    height: 92px;
  }
  .fader {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    font-size: 8px;
    color: #b0b0c8;
  }
  .slot {
    position: relative;
    flex: 1;
    width: 14px;
  }
  .slot::before {
    content: "";
    position: absolute;
    left: 5px;
    width: 4px;
    top: 0;
    bottom: 0;
    background: var(--dark);
  }
  .knob {
    position: absolute;
    left: 0;
    width: 14px;
    height: 10px;
  }
  .raised-green {
    background: #6a9a3a;
    border: 2px solid;
    border-color: #b0e080 #2a4a10 #2a4a10 #b0e080;
  }
  .list-body {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .playlist {
    position: relative;
    flex: 1;
    overflow-y: auto;
    padding: 4px;
    background: #000;
  }
  .entry.later {
    opacity: 0.6;
  }
  .entry {
    display: block;
    width: 100%;
    padding: 2px 0;
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--green);
    font: 13px ui-monospace, SFMono-Regular, Menlo, monospace;
  }
  .entry.current {
    color: #fff;
    background: #0000a0;
  }
  .entry:hover:not(.current) {
    background: rgb(0 224 0 / 0.12);
  }
  @container (max-width: 520px) {
    .lcd {
      width: 132px;
    }
    .clock {
      font-size: 26px;
    }
  }
</style>
