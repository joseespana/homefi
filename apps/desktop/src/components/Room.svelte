<script lang="ts">
  import NowActions from "./NowActions.svelte";
  import LyricsView from "./LyricsView.svelte";
  import { lyrics } from "../lib/lyrics.svelte";
  import { api } from "../lib/api";
  import { app, groupAlbums } from "../lib/state.svelte";
  import RoomMenu from "./RoomMenu.svelte";
  import { visual } from "../lib/visual.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";
  import MascotHost from "./MascotHost.svelte";
  import SignalChain from "./SignalChain.svelte";
  import Visualizer, { type VisualMode } from "./Visualizer.svelte";
  import VuMeter from "./VuMeter.svelte";
  import { layoutOf } from "../lib/layouts";
  import { share } from "../lib/share.svelte";
  import { useSource } from "../lib/source.svelte";
  import Aluminum from "./layouts/Aluminum.svelte";
  import Luna from "./layouts/Luna.svelte";
  import Modern from "./layouts/Modern.svelte";
  import RetroAmp from "./layouts/RetroAmp.svelte";
  import Web2000 from "./layouts/Web2000.svelte";
  import { item as itemIn, rise, swap } from "../lib/motion";
  import { sound } from "../lib/sound.svelte";
  import HowSounds from "./sound/HowSounds.svelte";

  // the app's player, or someone else's room while this computer is a guest there
  const src = useSource();
  const p = $derived(src.player);
  const track = $derived(src.current);
  const playing = $derived(p?.status === "playing");
  let mode = $state<VisualMode>("bars");
  let customize = $state(false);
  const room = $derived(app.settings?.room);
  const show = $derived({
    signal: room?.signal_chain ?? true,
    vu: room?.vu_meters ?? true,
    viz: room?.visualizer ?? true,
    halo: room?.halo ?? true,
    // floating mascots are drawn by the app, over every view
    mascot: (room?.mascot ?? true) && !room?.mascot_free,
  });

  const upNext = $derived.by(() => {
    if (!p || p.current == null) return [];
    return p.queue.slice(p.current + 1, p.current + 6).map((path, i) => ({ index: p.current! + 1 + i, path, track: src.trackAt(path) }));
  });
  const recent = $derived(groupAlbums(app.tracks).slice(0, 8));
  const layout = $derived(layoutOf(room?.layout));
</script>

{#if track && lyrics.open}
  <LyricsView />
{:else if layout === "aluminum"}
  <Aluminum />
{:else if track && layout === "retro_amp"}
  <RetroAmp />
{:else if track && layout === "luna"}
  <Luna />
{:else if track && layout === "web2000"}
  <Web2000 />
{:else if track && (layout === "immersive" || layout === "vinyl" || layout === "studio")}
  <Modern kind={layout} />
{:else if track}
  <section class="stage" in:rise={{ y: 16, duration: 360 }}>
    <div class="art">
      {#if show.halo}<div class="halo"><Visualizer mode="halo" height={560} /></div>{/if}
      {#key track.path}
        <div class="sleeve" class:spin={playing} in:swap={{ x: -24 }}>
          <Cover path={track.path} size="100%" radius={18} icon="disc" />
        </div>
      {/key}
    </div>

    <div class="info">
      <div class="topline">
        <div class="eyebrow">{playing ? "Now playing" : p?.status === "paused" ? "Paused" : "Ready"}{track.genre ? ` · ${track.genre}` : ""}{src.remote ? ` · ${src.label}` : ""}</div>
        <div class="customize">
          {#if !src.remote && sound.ui.react_room}<HowSounds place="room" />{/if}
          <button class="chip-btn" onclick={() => lyrics.setOpen(true)} aria-label="Show the lyrics" title="Lyrics"><Icon name="lyrics" size={13} /> Lyrics</button>
          <button class="chip-btn" onclick={() => (customize = !customize)} aria-label="Customize the room">Customize</button>
          {#if customize}<RoomMenu onclose={() => (customize = false)} />{/if}
        </div>
      </div>
      {#key track.path}
      <div class="titles" in:swap={{ x: 24, delay: 60 }}>
      <h1 class="title">{track.title}</h1>
      <div class="sub">
        {#if src.remote}
          <span class="link">{track.artist || "Unknown artist"}</span>
        {:else}
          <button class="link" onclick={() => (app.view = { name: "artist", name_: track.albumArtist || track.artist })}>{track.artist || "Unknown artist"}</button>
        {/if}
        {#if track.album}<span>·</span><span>{track.album}</span>{/if}
        {#if track.year}<span>·</span><span>{track.year}</span>{/if}
      </div>
      </div>
      {/key}
      <NowActions />

      {#if show.signal && p?.stream}
        <div class="panel" class:slim={app.settings?.room.signal_style !== "detailed"}>
          <SignalChain />
        </div>
      {/if}

      {#if show.vu || show.mascot}
        <div class="rack">
          {#if show.vu}
            <div class="meters">
              <VuMeter value={visual.vuLeft} label="L" />
              <VuMeter value={visual.vuRight} label="R" />
            </div>
          {/if}
          {#if show.mascot}
            <MascotHost size={show.vu ? 150 : 220} />
          {/if}
        </div>
      {/if}

      {#if show.viz}
        <div class="panel scope">
          <Visualizer bind:mode cycle height={120} />
        </div>
      {/if}
    </div>
  </section>

  {#if upNext.length}
    <h2>Up next</h2>
    <div class="next">
      {#each upNext as item (item.index)}
        <button class="next-item" in:itemIn onclick={() => src.jump(item.index)} disabled={!src.can.jump}>
          <Cover path={item.path} size={44} radius={8} />
          <div class="grow">
            <div class="ellipsis">{item.track?.title ?? item.path.split(/[\\/]/).pop()}</div>
            <div class="muted ellipsis small">{item.track?.artist ?? ""}</div>
          </div>
        </button>
      {/each}
    </div>
  {/if}
{:else if src.remote}
  <section class="idle" in:rise>
    {#if show.mascot}<MascotHost size={220} asleep />{/if}
    <h1>Nothing playing in {share.guest.session_name}</h1>
    <p class="muted">Search the hub’s music and add a song.</p>
    <div class="actions">
      <button class="btn primary" onclick={() => share.open("search")}><Icon name="search" size={16} /> Search</button>
    </div>
  </section>
{:else}
  <section class="idle" in:rise>
    {#if show.mascot}<MascotHost size={220} asleep />{/if}
    <h1>Nothing playing</h1>
    <p class="muted">Pick a record and HomeFi will wake up.</p>
    {#if recent.length}
      <div class="crate">
        {#each recent as a, i (a.key)}
          <button
            class="record"
            style:--i={i}
            in:itemIn|global={{ step: 45 }}
            onclick={() => api.play(a.tracks.map((t) => t.path))}
            title="Play {a.title}"
            aria-label="Play {a.title}"
          >
            <Cover path={a.tracks[0].path} size="100%" radius={10} icon="disc" />
            <span class="record-title ellipsis">{a.title}</span>
          </button>
        {/each}
      </div>
    {:else}
      <div class="actions">
        <button class="btn primary" onclick={() => (app.view = { name: "search" })}><Icon name="search" size={16} /> Find music on Tidal</button>
        <button class="btn tonal" onclick={() => (app.view = { name: "settings" })}><Icon name="folder" size={16} /> Add a music folder</button>
      </div>
    {/if}
  </section>
{/if}

<style>
  .stage {
    display: grid;
    grid-template-columns: minmax(260px, 0.9fr) minmax(360px, 1.1fr);
    gap: 48px;
    align-items: center;
    min-height: calc(100vh - 240px);
  }
  .art {
    position: relative;
    aspect-ratio: 1;
    display: grid;
    place-items: center;
  }
  .halo {
    position: absolute;
    inset: -11%;
    pointer-events: none;
  }
  .halo :global(.visualizer) {
    height: 100% !important;
  }
  .sleeve {
    position: relative;
    width: 82%;
    border-radius: min(18px, var(--r-max));
    box-shadow: 0 30px 80px rgb(0 0 0 / 0.55), 0 0 0 1px rgb(255 255 255 / 0.08);
    transition: transform 0.6s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .sleeve.spin {
    transform: rotate(-2deg) scale(1.01);
  }
  .info {
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-width: 0;
  }
  /* the title and the artist line: they change together with the song */
  .titles {
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-width: 0;
  }
  .topline {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
  }
  .customize {
    position: relative;
    display: flex;
    gap: 6px;
  }
  .chip-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 5px 12px;
    border-radius: min(999px, var(--r-max));
    font-size: 12px;
    color: var(--muted);
    background: var(--glass-strong);
    border: 1px solid var(--glass-border);
  }
  .chip-btn:hover {
    color: var(--text);
  }
  .eyebrow {
    font: 700 11px var(--font-mono);
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--accent-text);
  }
  .title {
    font: 800 clamp(30px, 4vw, 54px) / 1.02 var(--font-display);
    letter-spacing: -0.03em;
    margin: 0;
    overflow-wrap: anywhere;
  }
  .sub {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    font-size: 16px;
    color: var(--muted);
  }
  .link {
    color: var(--text);
    font-weight: 600;
  }
  .link:hover {
    text-decoration: underline;
  }
  .panel {
    padding: 14px;
    border-radius: min(18px, var(--r-max));
    background: var(--glass);
    border: 1px solid var(--glass-border);
    backdrop-filter: blur(20px) saturate(1.3);
  }
  /* the compact signal chain: a thin strip */
  .panel.slim {
    padding: 8px 10px;
  }
  .rack {
    display: flex;
    align-items: flex-end;
    gap: 16px;
  }
  .meters {
    flex: 1;
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
    padding: 14px;
    border-radius: min(18px, var(--r-max));
    background: linear-gradient(180deg, #25313e, #141c26);
    box-shadow: inset 0 1px 0 rgb(255 255 255 / 0.08), 0 16px 40px rgb(0 0 0 / 0.35);
  }
  .scope {
    padding: 8px 10px;
  }
  .next {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 8px;
  }
  .next-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px;
    border-radius: min(12px, var(--r-max));
    background: var(--glass);
    border: 1px solid var(--glass-border);
    text-align: left;
    min-width: 0;
  }
  .next-item:hover {
    background: var(--glass-strong);
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .small {
    font-size: 12px;
  }
  .idle {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 6px;
    padding-top: 40px;
  }
  .idle h1 {
    font: 800 34px var(--font-display);
  }
  .actions {
    display: flex;
    gap: 10px;
    margin-top: 16px;
  }
  .crate {
    display: flex;
    justify-content: center;
    margin-top: 36px;
    perspective: 900px;
    padding: 0 40px;
  }
  .record {
    position: relative;
    width: 150px;
    flex: none;
    margin-left: -46px;
    transform: rotateY(28deg) translateZ(calc(var(--i) * -4px));
    transform-origin: left center;
    transition: transform 0.35s cubic-bezier(0.2, 0.8, 0.2, 1), margin 0.35s;
    box-shadow: -12px 10px 30px rgb(0 0 0 / 0.45);
    border-radius: min(10px, var(--r-max));
  }
  .record:first-child {
    margin-left: 0;
  }
  .record:hover {
    transform: rotateY(0deg) translateY(-18px) scale(1.06);
    margin: 0 30px 0 -16px;
    z-index: 2;
  }
  .record-title {
    position: absolute;
    left: 8px;
    right: 8px;
    bottom: -26px;
    font-size: 12px;
    color: var(--muted);
    opacity: 0;
    transition: opacity 0.2s;
  }
  .record:hover .record-title {
    opacity: 1;
  }
  .rack {
    flex-wrap: wrap;
  }
  .meters {
    min-width: min(100%, 300px);
  }
  /* medium: smaller sleeve next to the info */
  @container (max-width: 980px) {
    .stage {
      grid-template-columns: minmax(200px, 0.75fr) minmax(300px, 1.25fr);
      gap: 28px;
    }
  }
  /* narrow: one column, compact sleeve on top */
  @container (max-width: 720px) {
    .stage {
      grid-template-columns: 1fr;
      gap: 20px;
      min-height: 0;
      padding-top: 8px;
    }
    .art {
      width: min(100%, 300px);
      margin: 0 auto;
    }
    .rack {
      justify-content: center;
    }
  }
</style>
