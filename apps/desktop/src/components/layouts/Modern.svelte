<script lang="ts">
  // The Android player's modern layouts over the same controls:
  // immersive (the album fills the room), vinyl (a record turning on the
  // platter) and studio (track strip, spectrum and VU meters).
  import { coverUrl, formatTime, type Repeat } from "../../lib/api";
  import { lowerLightness } from "../../lib/palette";
  import { useSource } from "../../lib/source.svelte";
  import { ownControls } from "../../lib/layouts";
  import { app } from "../../lib/state.svelte";
  import { visual } from "../../lib/visual.svelte";
  import Cover from "../Cover.svelte";
  import Icon from "../Icon.svelte";
  import NowActions from "../NowActions.svelte";
  import SeekBar from "../SeekBar.svelte";
  import Visualizer from "../Visualizer.svelte";
  import VuMeter from "../VuMeter.svelte";
  import Customize from "./Customize.svelte";
  import { rise, swap } from "../../lib/motion";

  let { kind }: { kind: "immersive" | "vinyl" | "studio" } = $props();

  const src = useSource();
  // as the skin of the whole app, the player bar has the controls
  const own = $derived(ownControls(app.settings?.room));
  const p = $derived(src.player);
  const track = $derived(src.current);
  const playing = $derived(p?.status === "playing");
  const duration = $derived(p?.duration_ms ?? track?.durationMs ?? 0);
  let scrub = $state<number | null>(null);

  // immersive: a shade under the title, darker the lighter the album is there
  let light = $state(0.5);
  $effect(() => {
    if (kind !== "immersive" || !track) return;
    let live = true;
    lowerLightness(coverUrl(track.path)).then((v) => live && (light = v));
    return () => (live = false);
  });
  const shade = $derived.by(() => {
    const mode = app.settings?.room.immersive_shade ?? "auto";
    if (mode === "off") return { mid: 0, end: 0 };
    if (mode === "strong") return { mid: 0.55, end: 0.88 };
    if (mode === "soft") return { mid: 0.18, end: 0.45 };
    return { mid: 0.12 + 0.45 * light, end: 0.35 + 0.55 * light };
  });
  const position = $derived(scrub ?? p?.position_ms ?? 0);
  const nextRepeat: Record<Repeat, Repeat> = { off: "all", all: "one", one: "off" };
  const hash = (text: string) => [...text].reduce((h, c) => (h * 31 + c.charCodeAt(0)) | 0, 7);
</script>

<div class="modern {kind}" in:rise={{ y: 16, duration: 360 }}>
  {#if kind === "immersive" && track}
    <div class="backdrop" aria-hidden="true">
      {#key track.path}<img src={coverUrl(track.path)} alt="" />{/key}
      <div class="fade" style:--shade-mid={shade.mid.toFixed(2)} style:--shade-end={shade.end.toFixed(2)}></div>
    </div>
  {/if}

  <div class="top"><Customize /></div>

  {#if kind === "vinyl"}
    <div class="platter">
      <div class="record" class:spin={playing}>
        {#key track?.path}<div class="label" in:swap={{ x: 0 }}><Cover path={track?.path} size="100%" radius={999} icon="disc" /></div>{/key}
        <div class="hole"></div>
      </div>
    </div>
  {:else if kind === "studio"}
    <div class="strip">
      <Cover path={track?.path} size={64} radius={10} />
      {@render info(false)}
    </div>
    <div class="panel spectrum"><Visualizer mode="bars" height={200} /></div>
    <div class="meters">
      <VuMeter value={visual.vuLeft} label="L" />
      <VuMeter value={visual.vuRight} label="R" />
    </div>
  {:else}
    <div class="spacer"></div>
  {/if}

  {#if kind !== "studio"}{@render info(kind === "vinyl")}{/if}

  {#if own}
  <div class="seek">
    <span class="time">{formatTime(position)}</span>
    <SeekBar
      fraction={duration ? position / duration : 0}
      style={app.settings?.progress_style}
      thickness={app.settings?.progress_thickness}
      animated={app.settings?.progress_animated ?? true}
      {playing}
      seed={hash(track?.path ?? "")}
      disabled={!duration || !src.can.seek}
      onscrub={(f) => (scrub = f === null ? null : f * duration)}
      onseek={(f) => {
        src.seek(Math.round(f * duration));
        scrub = null;
      }}
    />
    <span class="time">{formatTime(duration)}</span>
  </div>

  <!-- shuffle · previous · play · next · repeat, one even gap between them -->
  <div class="transport">
    <button class="ctl" class:on={p?.shuffle} style:--s="44px" onclick={() => src.shuffle(!p?.shuffle)} disabled={!src.can.shuffle} aria-label="Shuffle" title="Shuffle"><Icon name="shuffle" size={22} /></button>
    <button class="ctl" style:--s="56px" onclick={() => src.previous()} disabled={!src.can.previous} aria-label="Previous" title="Previous"><Icon name="prev" size={30} /></button>
    <button class="ctl play" style:--s="72px" onclick={() => src.toggle()} disabled={!p?.queue.length || !src.can.toggle} aria-label={playing ? "Pause" : "Play"} title={playing ? "Pause" : "Play"}>
      <Icon name={playing ? "pause" : "play"} size={36} />
    </button>
    <button class="ctl" style:--s="56px" onclick={() => src.next()} disabled={!src.can.next} aria-label="Next" title="Next"><Icon name="next" size={30} /></button>
    <button class="ctl" class:on={p && p.repeat !== "off"} style:--s="44px" onclick={() => p && src.repeat(nextRepeat[p.repeat])} disabled={!src.can.repeat} aria-label="Repeat: {p?.repeat ?? 'off'}" title="Repeat: {p?.repeat ?? 'off'}">
      <Icon name={p?.repeat === "one" ? "repeat1" : "repeat"} size={22} />
    </button>
  </div>
  {/if}
</div>

{#snippet info(centered: boolean)}
  <div class="info" class:centered class:large={kind === "immersive"}>
    {#key track?.path}
      <h1 class="title ellipsis" in:swap>{track?.title ?? ""}</h1>
      <div class="sub ellipsis" in:swap={{ delay: 70 }}>{[track?.artist, track?.album].filter(Boolean).join(" · ")}</div>
    {/key}
    <div class="facts">
      {#if src.remote}
        <span class="chip"><Icon name="users" size={13} /> {src.label}</span>
      {:else}
        <span class="chip"><Icon name={app.currentIsStream ? "download" : "disc"} size={13} /> {app.currentIsStream ? "Tidal" : "This computer"}</span>
      {/if}
      {#if track?.quality}<span class="chip quality">{track.quality}</span>{/if}
      <NowActions compact />
    </div>
  </div>
{/snippet}

<style>
  .modern {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 4px 0 8px;
  }
  .modern > :not(.backdrop) {
    position: relative;
  }
  .top {
    display: flex;
    justify-content: flex-end;
    z-index: 3;
  }
  .backdrop {
    position: absolute;
    inset: 0 0 0 0;
    border-radius: min(18px, var(--r-max));
    overflow: hidden;
  }
  .backdrop img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    animation: fadein 0.8s ease;
  }
  @keyframes fadein {
    from {
      opacity: 0;
    }
  }
  .backdrop .fade {
    position: absolute;
    inset: 0;
    background: linear-gradient(180deg, rgb(0 0 0 / 0.3), transparent 35%, color-mix(in srgb, var(--room-bg) 85%, transparent) 72%, var(--room-bg));
  }
  /* the shade that keeps the title readable on any cover (see `shade`) */
  .backdrop .fade::after {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(180deg, transparent 45%, rgb(0 0 0 / var(--shade-mid, 0.3)) 72%, rgb(0 0 0 / var(--shade-end, 0.6)));
    transition: background 0.6s;
  }
  .immersive {
    padding: 12px 24px 18px;
  }
  .immersive .info,
  .immersive .seek,
  .immersive .time {
    color: #fff;
  }
  .immersive .sub {
    color: rgb(255 255 255 / 0.82);
  }
  .immersive .info .title,
  .immersive .info .sub {
    text-shadow: 0 1px 2px rgb(0 0 0 / 0.45), 0 2px 18px rgb(0 0 0 / 0.35);
  }
  .spacer {
    flex: 1;
  }
  .info {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .info.centered {
    align-items: center;
    text-align: center;
  }
  .info > * {
    max-width: 100%;
  }
  .title {
    margin: 0;
    font: 800 24px/1.15 var(--font-display);
    letter-spacing: -0.02em;
  }
  .info.large .title {
    font-size: clamp(28px, 4vw, 48px);
  }
  .sub {
    font-size: 15px;
    color: var(--muted);
  }
  .facts {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 6px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    border-radius: min(999px, var(--r-max));
    font-size: 12px;
    font-weight: 600;
    background: var(--glass-strong);
    border: 1px solid var(--glass-border);
  }
  .chip.quality {
    color: var(--accent-text);
  }
  .seek {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .seek > :global(canvas) {
    flex: 1;
    min-width: 0;
  }
  .time {
    font: 600 12px var(--font-mono);
    color: var(--muted);
    min-width: 40px;
    text-align: center;
  }
  .transport {
    display: flex;
    justify-content: center;
    align-items: center;
    gap: 6px;
  }
  .ctl {
    display: grid;
    place-items: center;
    width: var(--s);
    height: var(--s);
    border-radius: 50%;
    color: var(--text);
  }
  .ctl:hover {
    background: var(--glass-strong);
  }
  .ctl.on {
    color: var(--accent-text);
  }
  .ctl:not(.on):not(.play) {
    color: color-mix(in srgb, var(--text) 80%, transparent);
  }
  .ctl.play {
    color: var(--on-primary);
    background: var(--accent-text);
    box-shadow: 0 10px 30px color-mix(in srgb, var(--room-glow) 45%, transparent);
  }
  .immersive .ctl:not(.play) {
    color: #fff;
  }
  /* vinyl: a black record with grooves, the cover as its label, 33⅓ rpm */
  .platter {
    flex: 1;
    min-height: 0;
    display: grid;
    place-items: center;
    container-type: size;
  }
  .record {
    position: relative;
    width: min(100cqw, 100cqh);
    aspect-ratio: 1;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background:
      conic-gradient(from -60deg, transparent 0deg, rgb(255 255 255 / 0.1) 25deg, transparent 50deg, transparent 180deg, rgb(255 255 255 / 0.06) 205deg, transparent 230deg),
      repeating-radial-gradient(circle, transparent 0 5px, rgb(255 255 255 / 0.06) 5px 6px),
      radial-gradient(circle, #2a2a2e, #0b0b0d 70%);
    box-shadow: 0 30px 80px rgb(0 0 0 / 0.55), 0 0 0 1px rgb(255 255 255 / 0.06);
    animation: spin 1.8s linear infinite;
    animation-play-state: paused;
  }
  .record.spin {
    animation-play-state: running;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .label {
    position: absolute;
    width: 38%;
    aspect-ratio: 1;
    border-radius: 50%;
    overflow: hidden;
  }
  .hole {
    position: absolute;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--room-bg);
  }
  /* studio: hi-fi deck */
  .strip {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .panel {
    flex: 1;
    min-height: 0;
    padding: 8px;
    border-radius: min(16px, var(--r-max));
    background: var(--glass);
    border: 1px solid var(--glass-border);
  }
  .panel :global(.visualizer) {
    height: 100% !important;
  }
  .meters {
    display: flex;
    justify-content: center;
    gap: 16px;
  }
  .meters :global(.vu) {
    flex: 1;
    /* short windows: the spectrum keeps most of the height */
    max-width: min(240px, 28vh);
  }
</style>
