<script lang="ts">
  // Charcoal tabs, grey boxes and glossy round buttons, like the web players of
  // the 2000s. Only the look is borrowed.
  import { formatTime, type Repeat } from "../../lib/api";
  import { seekable } from "../../lib/seek";
  import { useSource } from "../../lib/source.svelte";
  import { ownControls } from "../../lib/layouts";
  import { app } from "../../lib/state.svelte";
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
  const index = $derived(p?.current ?? -1);
  const nextRepeat: Record<Repeat, Repeat> = { off: "all", all: "one", one: "off" };
  let tab = $state(0);
  const tabs = ["playing", "up next"];
</script>

<div class="web" in:rise>
  <div class="tabs">
    {#each tabs as label, i}
      <button class="tab" class:selected={tab === i} onclick={() => (tab = i)}>{label}</button>
    {/each}
    <span class="grow"></span>
    <Customize />
  </div>
  <div class="page">
    <div class="box">
      <div class="box-title">{tabs[tab]}</div>
      {#if tab === 0}
        {#key track?.path}
        <div class="now" in:swap={{ x: 0 }}>
          <div class="art"><Cover path={track?.path} size="100%" radius={4} icon="disc" /></div>
          <div class="title ellipsis">{track?.title ?? ""}</div>
          <div class="artist ellipsis">{track?.artist ?? ""}</div>
          {#if track?.quality}<div class="quality">{track.quality}</div>{/if}
        </div>
        {/key}
      {:else}
        <div class="list" in:rise>
          {#each p?.queue ?? [] as path, i (i)}
            {@const t = src.trackAt(path)}
            <button class="entry" class:current={i === index} onclick={() => src.jump(i)} disabled={!src.can.jump && i !== index}>
              <span class="num">{i + 1}</span>
              <span class="grow-text">
                <b class="ellipsis">{t?.title ?? path.split(/[\\/]/).pop()}</b>
                <span class="ellipsis">{t?.artist ?? ""}</span>
              </span>
              <span class="ring"><Icon name="play" size={12} /></span>
            </button>
          {/each}
          {#if !src.remote}
            {#each app.upcoming as u, j (u.id + j)}
              <button class="entry later" onclick={() => app.playUpcoming(j)} title="Play now (from Tidal)">
                <span class="num">{(p?.queue.length ?? 0) + j + 1}</span>
                <span class="grow-text">
                  <b class="ellipsis">{u.title}</b>
                  <span class="ellipsis">{u.artist}</span>
                </span>
                <span class="ring"><Icon name="play" size={12} /></span>
              </button>
            {/each}
          {/if}
        </div>
      {/if}
    </div>
    {#if chainOn}<div class="chain-row"><SignalChain look="web" /></div>{/if}

    {#if own}
    <!-- progress and controls -->
    <div class="progress">
      <span>{formatTime(position)}</span>
      <div class="track" use:seekable={{ duration: src.can.seek ? duration : 0, seek: src.seek, onpreview: (v) => (scrub = v) }} role="slider" aria-label="Seek" aria-valuemin={0} aria-valuemax={duration} aria-valuenow={position} tabindex="-1">
        <div class="fill" style:width="{f * 100}%"></div>
      </div>
      <span>{formatTime(duration)}</span>
    </div>
    <div class="controls">
      <button class="gloss" class:on={p?.shuffle} style:--s="36px" onclick={() => src.shuffle(!p?.shuffle)} disabled={!src.can.shuffle} aria-label="Shuffle" title="Shuffle"><Icon name="shuffle" size={18} /></button>
      <button class="gloss" style:--s="44px" onclick={() => src.previous()} disabled={!src.can.previous} aria-label="Previous" title="Previous"><Icon name="prev" size={22} /></button>
      <button class="gloss on" style:--s="60px" onclick={() => src.toggle()} disabled={!src.can.toggle} aria-label={playing ? "Pause" : "Play"} title={playing ? "Pause" : "Play"}><Icon name={playing ? "pause" : "play"} size={32} /></button>
      <button class="gloss" style:--s="44px" onclick={() => src.next()} disabled={!src.can.next} aria-label="Next" title="Next"><Icon name="next" size={22} /></button>
      <button class="gloss" class:on={p && p.repeat !== "off"} style:--s="36px" onclick={() => p && src.repeat(nextRepeat[p.repeat])} disabled={!src.can.repeat} aria-label="Repeat: {p?.repeat ?? 'off'}" title="Repeat: {p?.repeat ?? 'off'}">
        <Icon name={p?.repeat === "one" ? "repeat1" : "repeat"} size={18} />
      </button>
    </div>
    {/if}
  </div>
</div>

<style>
  .entry.later {
    opacity: 0.65;
  }
  .web {
    --green: #8cc63f;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    border-radius: 10px;
    background: #3c3c3f;
    color: #ededed;
  }
  .tabs {
    padding: 10px 10px 0;
    display: flex;
    align-items: flex-end;
    gap: 4px;
  }
  .tab {
    padding: 8px 18px;
    border-radius: 10px 10px 0 0;
    font-weight: 700;
    font-size: 14px;
    color: #dadada;
    background: linear-gradient(180deg, #6a6a6e, #525256);
  }
  .tab.selected {
    color: #3a3a3a;
    background: linear-gradient(180deg, #f4f4f4, #d9d9d9);
  }
  .tabs :global(.customize) {
    align-self: center;
    margin-bottom: 4px;
  }
  .grow {
    flex: 1;
  }
  .chain-row {
    min-width: 0;
  }
  .chain-row :global(.chain) {
    justify-content: center;
  }
  .page {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px;
    border-radius: 0 0 10px 10px;
    background: #d9d9d9;
  }
  .box {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    border-radius: 6px;
    background: #f2f2f2;
  }
  .box-title {
    padding: 5px 10px;
    color: #fff;
    font-weight: 700;
    font-size: 13px;
    background: linear-gradient(180deg, #a8a8a8, #8e8e8e);
  }
  .now {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 10px;
    text-align: center;
  }
  .now > * {
    max-width: 100%;
  }
  .art {
    flex: 1;
    min-height: 0;
    aspect-ratio: 1;
    margin-bottom: 8px;
  }
  .art :global(.cover) {
    height: 100%;
    width: auto !important;
    aspect-ratio: 1;
  }
  .title {
    color: #222;
    font-weight: 700;
    font-size: 17px;
  }
  .artist {
    color: #555;
    font-size: 13px;
  }
  .quality {
    color: #5e8f1e;
    font-size: 12px;
    font-weight: 700;
  }
  .list {
    flex: 1;
    overflow-y: auto;
    padding: 4px 0;
  }
  .entry {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    text-align: left;
    color: #222;
  }
  .entry.current {
    background: #e4f1d2;
  }
  .entry:hover:not(.current) {
    background: #e8e8e8;
  }
  .num {
    width: 26px;
    flex: none;
    color: #777;
    font-weight: 700;
    font-size: 13px;
  }
  .grow-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .grow-text b {
    font-size: 14px;
  }
  .grow-text span {
    font-size: 12px;
    color: #666;
  }
  .ring {
    flex: none;
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    border: 1.5px solid #8a8a8a;
    color: #6a6a6a;
  }
  .progress {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: #444;
    font-variant-numeric: tabular-nums;
  }
  .track {
    position: relative;
    flex: 1;
    height: 8px;
    overflow: hidden;
    border-radius: 4px;
    background: #b5b5b5;
  }
  .fill {
    height: 100%;
    background: var(--green);
  }
  .controls {
    display: flex;
    justify-content: center;
    align-items: center;
    gap: 8px;
    padding-bottom: 4px;
  }
  .gloss {
    display: grid;
    place-items: center;
    width: var(--s);
    height: var(--s);
    border-radius: 50%;
    color: #555;
    background: linear-gradient(180deg, #fafafa, #cfcfcf);
    border: 1.5px solid #8a8a8a;
  }
  .gloss.on {
    color: #5e8f1e;
    border-color: var(--green);
  }
  .gloss:hover {
    background: linear-gradient(180deg, #fff, #dadada);
  }
</style>
