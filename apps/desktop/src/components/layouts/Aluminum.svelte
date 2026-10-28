<script lang="ts">
  // Brushed metal like the jukebox software of 2008: transport on the bar, a pale
  // LCD with the song, and the Cover Flow filling the rest.
  import { formatTime, type Repeat } from "../../lib/api";
  import { seekable } from "../../lib/seek";
  import { useSource } from "../../lib/source.svelte";
  import { ownControls } from "../../lib/layouts";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";
  import SignalChain from "../SignalChain.svelte";
  import CoverFlow from "./CoverFlow.svelte";
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
</script>

<div class="alu" in:rise>
  <div class="bar" class:bare={!own}>
    {#if own}
    <div class="transport">
      <button class="metal" style:--s="40px" onclick={() => src.previous()} disabled={!src.can.previous} aria-label="Previous" title="Previous"><Icon name="prev" size={20} /></button>
      <button class="metal" style:--s="52px" onclick={() => src.toggle()} disabled={!p?.queue.length || !src.can.toggle} aria-label={playing ? "Pause" : "Play"} title={playing ? "Pause" : "Play"}>
        <Icon name={playing ? "pause" : "play"} size={28} />
      </button>
      <button class="metal" style:--s="40px" onclick={() => src.next()} disabled={!src.can.next} aria-label="Next" title="Next"><Icon name="next" size={20} /></button>
    </div>
    {/if}

    <!-- the pale LCD -->
    <div class="lcd">
      {#if track}
        {#key track.path}
          <div class="lcd-title ellipsis" in:swap={{ x: 0 }}>{track.title}</div>
          <div class="lcd-sub ellipsis" in:swap={{ x: 0, delay: 60 }}>{[track.artist, track.album].filter(Boolean).join(" — ")}</div>
        {/key}
        <div class="lcd-time">
          <span>{formatTime(position)}</span>
          <div class="groove" use:seekable={{ duration: src.can.seek ? duration : 0, seek: src.seek, onpreview: (v) => (scrub = v) }} role="slider" aria-label="Seek" aria-valuemin={0} aria-valuemax={duration} aria-valuenow={position} tabindex="-1">
            <div class="fill" style:width="{f * 100}%"></div>
            <div class="diamond" style:left="calc({f} * (100% - 8px))"></div>
          </div>
          <span>-{formatTime(Math.max(0, duration - position))}</span>
        </div>
      {:else}
        <div class="lcd-title">HomeFi</div>
        <div class="lcd-sub">Pick an album below and click it for the songs</div>
      {/if}
    </div>

    <div class="modes">
      {#if own}
      <button class="metal" class:on={p?.shuffle} style:--s="34px" onclick={() => src.shuffle(!p?.shuffle)} disabled={!src.can.shuffle} aria-label="Shuffle" title="Shuffle"><Icon name="shuffle" size={17} /></button>
      <button class="metal" class:on={p && p.repeat !== "off"} style:--s="34px" onclick={() => p && src.repeat(nextRepeat[p.repeat])} disabled={!src.can.repeat} aria-label="Repeat: {p?.repeat ?? 'off'}" title="Repeat: {p?.repeat ?? 'off'}">
        <Icon name={p?.repeat === "one" ? "repeat1" : "repeat"} size={17} />
      </button>
      {/if}
      <Customize light />
    </div>
  </div>

  {#if chainOn}<div class="chain-row"><SignalChain look="alu" /></div>{/if}
  <div class="flow"><CoverFlow /></div>
</div>

<style>
  .alu {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 10px;
    border-radius: 12px;
    color: #333;
    background:
      repeating-linear-gradient(90deg, rgb(255 255 255 / 0.05) 0 1px, rgb(0 0 0 / 0.025) 1px 3px),
      linear-gradient(180deg, #e6e6e6, #bdbdbd);
    border: 1px solid #9a9a9a;
    box-shadow: inset 0 1px 0 rgb(255 255 255 / 0.8), 0 20px 50px rgb(0 0 0 / 0.35);
  }
  .bar {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    grid-template-areas: "transport lcd modes";
    align-items: center;
    gap: 14px;
  }
  /* no transport (the player bar has it): the LCD alone in the middle */
  .bar.bare {
    grid-template-columns: minmax(0, 1fr) minmax(0, 620px) minmax(0, 1fr);
    grid-template-areas: ". lcd modes";
  }
  .transport,
  .modes {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .transport {
    grid-area: transport;
  }
  .modes {
    grid-area: modes;
    justify-content: flex-end;
  }
  .metal {
    flex: none;
    display: grid;
    place-items: center;
    width: var(--s);
    height: var(--s);
    border-radius: 50%;
    color: #2e2e2e;
    background: linear-gradient(180deg, #fdfdfd, #c9c9c9 55%, #a0a0a0);
    border: 1px solid #6e6e6e;
    box-shadow: inset 0 1px 0 #fff, 0 1px 2px rgb(0 0 0 / 0.25);
  }
  .metal:hover {
    background: linear-gradient(180deg, #fff, #d6d6d6 55%, #ababab);
  }
  .metal:active {
    background: linear-gradient(180deg, #a8a8a8, #cfcfcf 55%, #efefef);
  }
  .metal.on {
    color: #2e6fd8;
  }
  .lcd {
    grid-area: lcd;
    justify-self: center;
    width: min(100%, 620px);
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 8px 12px;
    border-radius: 8px;
    background: linear-gradient(180deg, #f2f4e6, #d8dcc4);
    border: 1px solid #8c8c8c;
    box-shadow: inset 0 1px 3px rgb(0 0 0 / 0.18);
    text-align: center;
  }
  .lcd > div {
    max-width: 100%;
  }
  .lcd-title {
    color: #1e1e1e;
    font-weight: 600;
    font-size: 14px;
  }
  .lcd-sub {
    color: #4a4a4a;
    font-size: 12px;
  }
  .lcd-time {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 6px;
    font-size: 10px;
    color: #3a3a3a;
    font-variant-numeric: tabular-nums;
  }
  .groove {
    position: relative;
    flex: 1;
    height: 10px;
  }
  .groove::before {
    content: "";
    position: absolute;
    inset: 1.5px 0;
    border: 1px solid #555;
  }
  .fill {
    position: absolute;
    left: 0;
    top: 1.5px;
    bottom: 1.5px;
    background: #555;
  }
  .diamond {
    position: absolute;
    top: 1px;
    width: 8px;
    height: 8px;
    background: #222;
    transform: rotate(45deg);
  }
  .chain-row {
    min-width: 0;
  }
  .chain-row :global(.chain) {
    justify-content: center;
  }
  .flow {
    flex: 1;
    min-height: 0;
  }
  /* narrow: transport and modes on one row, the LCD under them (as on the phone) */
  @container (max-width: 720px) {
    .bar {
      grid-template-columns: auto 1fr;
      grid-template-areas: "transport modes" "lcd lcd";
      gap: 10px;
    }
    .bar.bare {
      grid-template-columns: 1fr;
      grid-template-areas: "modes" "lcd";
    }
    .lcd {
      width: 100%;
    }
  }
</style>
