<script lang="ts">
  // DJ mode's screen (docs/DJ_UI.md): header, deck A | mixer | deck B, sampler and crate. Turning
  // it on is explicit (the music that plays pauses); leaving the screen keeps it on, and songs
  // picked elsewhere go to a deck (Settings → DJ).
  import { dj } from "../../lib/dj.svelte";
  import { DJ_DRAWER_WIDTH_PX, DJ_MASTER_MAX, type DjSectionKind } from "../../lib/dj-api";
  import { rowsOf, type DjRow } from "../../lib/dj-layout";
  import { MASCOT } from "../../lib/mascot";
  import { homifitoDj } from "../../lib/homifito-dj.svelte";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";
  import DjCrate from "./DjCrate.svelte";
  import DjDeck from "./DjDeck.svelte";
  import DjFirstRun from "./DjFirstRun.svelte";
  import DjKeyboardSheet from "./DjKeyboardSheet.svelte";
  import DjHomifito from "./DjHomifito.svelte";
  import DjKnob from "./DjKnob.svelte";
  import DjLayoutEditor from "./DjLayoutEditor.svelte";
  import DjMixer from "./DjMixer.svelte";
  import DjSampler from "./DjSampler.svelte";
  import DjSetupsMenu from "./DjSetupsMenu.svelte";
  import DjVu from "./DjVu.svelte";

  const MASTER_RESET = 1;

  let width = $state(window.innerWidth);
  const drawer = $derived(width < DJ_DRAWER_WIDTH_PX);
  const simple = $derived(dj.settings.simple_view);
  const crateShown = $derived(drawer ? dj.drawerOpen : dj.crateOpen);
  const master = $derived(dj.snap?.master ?? MASTER_RESET);
  /** The rows to draw: the layout's visible sections in order (the crate waits in its drawer on a narrow window, or while it is closed). */
  const rows = $derived(rowsOf(dj.layout, drawer || !dj.crateOpen ? ["crate"] : []));
  /** A column per section: decks and sampler take the room, the mixer and the crate a fixed share. */
  const COLUMN: Record<DjSectionKind, string> = { deck_a: "minmax(0, 1fr)", deck_b: "minmax(0, 1fr)", mixer: "clamp(250px, 22vw, 320px)", sampler: "minmax(0, 1fr)", crate: "clamp(300px, 30vw, 440px)" };
  const columns = (row: DjRow) => row.kinds.map((k) => COLUMN[k]).join(" ");
  /** On a narrower board the decks share a row two by two and the mixer goes below them: how many decks sit side by side. */
  const deckColumns = (row: DjRow) => Math.max(1, Math.min(2, row.kinds.filter((k) => k === "deck_a" || k === "deck_b").length));
  /** The crate shows a few rows and scrolls: next to the sampler it is as tall as the pads, alone it gets more. */
  const rowHeight = (row: DjRow) => (row.band === "stage" || !row.kinds.includes("crate") ? undefined : row.kinds.length > 1 ? "150px" : "240px");
  const masterPeak = $derived(Math.max(...(dj.snap?.master_peak ?? [0, 0])));

  // dev screenshots: ?djsheet opens the shortcuts, ?djdrawer the crate drawer
  if (import.meta.env.DEV) {
    const flags = new URLSearchParams(location.search);
    if (flags.has("djsheet")) dj.sheetOpen = true;
    if (flags.has("djdrawer")) dj.drawerOpen = true;
    if (flags.has("djedit")) dj.editing = true;
  }

  async function exit() {
    await dj.stop();
    app.view = { name: "room" };
  }

  function key(e: KeyboardEvent, down: boolean) {
    if (down && e.key === "Escape" && drawer && dj.drawerOpen) {
      dj.drawerOpen = false;
      return;
    }
    if (dj.onKey(e, down)) e.preventDefault();
  }
</script>

<svelte:window bind:innerWidth={width} onkeydown={(e) => key(e, true)} onkeyup={(e) => key(e, false)} />

{#if !dj.on}
  <section class="intro" aria-labelledby="dj-intro-title">
    <span class="mark" aria-hidden="true"><Icon name="dj" size={36} /></span>
    <h1 id="dj-intro-title">DJ mode</h1>
    <p>Mix two songs like a DJ: line up their beats, fade from one to the other, add effects. Everything works with the mouse.</p>
    <p class="muted small">The music that is playing pauses while DJ mode is on, and the sound is mixed (not bit-perfect). Turn it off to get your player back where it was.</p>
    {#if dj.startError}<p class="error" role="alert">{dj.startError}</p>{/if}
    <button class="btn primary start" type="button" disabled={dj.starting} onclick={() => dj.start()}>{dj.starting ? "Starting…" : "Start DJ mode"}</button>
  </section>
{:else}
  <main class="dj" class:hlook={homifitoDj.look} aria-label="DJ mode">
    <header class="bar">
      <h1>DJ</h1>
      <span class="badge-np" title="DJ mode mixes two songs, so audio is not bit-perfect. Turn it off to listen bit-perfect."><b></b><span class="long">DJ mode on: not bit-perfect</span><span class="short">Not bit-perfect</span></span>
      {#if dj.snap?.device_lost}<span class="lost" role="alert">{dj.snap.device_lost}</span>{/if}
      <span class="sp"></span>
      <div class="master" role="group" aria-label="Master">
        <span class="cap-t">Master</span>
        <DjKnob value={master} min={0} max={DJ_MASTER_MAX} detent={MASTER_RESET} size={44} label="Master volume" tip="Overall volume" valueText={(v) => `${Math.round(v * 100)} %`} onchange={(value) => dj.send({ type: "master", value })} />
        <DjVu horizontal level={masterPeak} label="Master level" />
      </div>
      <DjSetupsMenu />
      <button class="btn hbtn hdj-btn" class:on={homifitoDj.on} type="button" aria-pressed={homifitoDj.on} aria-label="Homifito DJ" title={homifitoDj.on ? "Send Homifito home" : "Homifito DJ: he mixes with you behind his own booth"} onclick={() => homifitoDj.toggle()}>
        <svg viewBox="0 0 1024 1024" width="20" height="20" aria-hidden="true"><path d={MASCOT.house} fill="currentColor" opacity="0.28" /><path d={MASCOT.band} fill="none" stroke="currentColor" stroke-width="70" stroke-linecap="round" /><path d={MASCOT.cups} fill="currentColor" /><path d={MASCOT.eyes} fill="currentColor" /></svg>
      </button>
      <button class="btn hbtn" class:on={dj.editing} type="button" aria-pressed={dj.editing} aria-label="Edit layout" title="Rearrange the sections, hide some and choose the sound of each pad" onclick={() => (dj.editing = !dj.editing)}><Icon name="edit" size={16} /><span class="long">Edit layout</span></button>
      <button class="btn hbtn" type="button" aria-pressed={!simple} title={simple ? "Show hot cues, loops, trim and the tempo faders" : "Hide the extra controls: Sync does the matching"} onclick={() => dj.saveSettings({ simple_view: !simple })}>
        {simple ? "Show all controls" : "Simple view"}
      </button>
      <button class="btn hbtn" type="button" aria-label="Keyboard shortcuts" title="Keyboard shortcuts (?)" onclick={() => (dj.sheetOpen = true)}><Icon name="key" size={16} /><span class="long">Shortcuts</span></button>
      {#if dj.crateVisible}<button class="btn hbtn" class:on={crateShown} type="button" aria-pressed={crateShown} title="Hide or show the crate" onclick={() => (drawer ? (dj.drawerOpen = !dj.drawerOpen) : (dj.crateOpen = !dj.crateOpen))}>Crate</button>{/if}
      <button class="btn hbtn" type="button" title="Back to the normal player" onclick={exit}>Exit DJ</button>
    </header>

    {#if dj.settings.show_tips}<DjFirstRun />{/if}

    {#if dj.restartNeeded}
      <div class="notice" role="status">
        <Icon name="info" size={16} />
        <span>The new pad sounds play after DJ mode restarts. The songs on the decks are unloaded.</span>
        <button class="btn hbtn" type="button" disabled={dj.starting} onclick={() => dj.restart()}>Restart DJ mode</button>
      </div>
    {/if}

    {#if dj.editing}<DjLayoutEditor />{/if}

    {#if homifitoDj.on && homifitoDj.place === "top"}<DjHomifito size={homifitoDj.mode === "big" ? "big" : "booth"} />{/if}

    {#each rows as row (row.kinds.join())}
      <section class="row {row.band}" style:--cols={columns(row)} style:--deck-cols={deckColumns(row)} style:height={rowHeight(row)}>
        {#each row.kinds as kind (kind)}
          {#if kind === "deck_a"}<DjDeck deck="a" />
          {:else if kind === "deck_b"}<DjDeck deck="b" />
          {:else if kind === "mixer"}<DjMixer />
          {:else if kind === "sampler"}<DjSampler />
          {:else}<DjCrate />{/if}
        {/each}
      </section>
    {:else}
      <p class="bare">Every section is hidden. <button type="button" class="link" onclick={() => dj.resetLayout()}>Show them all again</button></p>
    {/each}

    {#if homifitoDj.on && homifitoDj.place === "bottom"}<DjHomifito size={homifitoDj.mode === "big" ? "big" : "booth"} />{/if}
  </main>
  {#if dj.drawerOpen && drawer && dj.crateVisible}<DjCrate drawer onclose={() => (dj.drawerOpen = false)} />{/if}
  {#if dj.sheetOpen}<DjKeyboardSheet />{/if}
{/if}
<div class="sr" aria-live="polite">{dj.live}</div>

<style>
  /* the rows (the layout's sections, in order): the decks and mixer take the height; when it doesn't
     fit (short windows) the screen scrolls instead of shrinking the controls */
  .dj {
    --dj-gap: 12px;
    display: flex;
    flex-direction: column;
    gap: var(--dj-gap);
    flex: 1;
    min-height: 0;
    min-width: 0;
    overflow-y: auto;
    padding-bottom: 4px;
    /* the board lays itself out by its own width, not the window's: the sidebar and the padding take their share */
    container: dj / inline-size;
  }
  .dj > * {
    flex: none;
  }
  @media (max-width: 1099px) {
    .dj {
      --dj-gap: 8px;
    }
  }
  @media (min-width: 1500px) {
    .dj {
      --dj-deck-h: 112px;
    }
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  /* on the room's album colours, which can be dark in a light theme: a frosted plate keeps it legible */
  .bar > h1,
  .bar > .badge-np {
    background: linear-gradient(var(--glass-strong), var(--glass-strong)), var(--bar-glass);
    backdrop-filter: blur(24px) saturate(1.5);
    -webkit-backdrop-filter: blur(24px) saturate(1.5);
  }
  .bar > h1 {
    padding: 2px 10px;
    border-radius: var(--r-sm);
    color: var(--text);
  }
  h1 {
    margin: 0 6px 0 0;
    font-size: 18px;
  }
  .badge-np {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 32px;
    padding: 0 12px;
    border-radius: var(--r-pill);
    background: var(--warn-bg);
    border: 1px solid var(--warn-border);
    color: var(--warn);
    font-size: 12px;
    font-weight: 600;
  }
  .badge-np b {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--warn);
  }
  .short {
    display: none;
  }
  @media (max-width: 1179px) {
    .long {
      display: none;
    }
    .short {
      display: inline;
    }
    .bar {
      gap: 8px;
    }
    .bar :global(.hbtn) {
      padding: 0 10px;
    }
  }
  .lost {
    color: var(--danger);
    font-size: 12px;
  }
  .sp {
    flex: 1;
  }
  .master {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .cap-t {
    font-size: 10px;
    color: var(--faint);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .bar :global(.hbtn),
  .notice .hbtn {
    height: 40px;
    padding: 0 14px;
    border-radius: var(--r-sm);
    border: 1px solid var(--border);
    background: var(--surface-2);
    font-weight: 500;
  }
  .bar :global(.hdj-btn) {
    display: inline-flex;
    align-items: center;
    padding: 0 12px;
  }
  .bar :global(.hbtn:hover),
  .notice .hbtn:hover {
    background: var(--surface-3);
  }
  .bar :global(.hbtn.on) {
    background: var(--primary);
    color: var(--on-primary);
    border-color: transparent;
  }
  /* Homifito's look: his own colours on the whole board (mint deck A, pink deck B, navy waveforms) */
  .dj.hlook {
    --dj-a: #10b981;
    --dj-a-ink: #032b1d;
    --dj-b: #f472b6;
    --dj-b-ink: #3b0620;
    --dj-wave-bg: #0b1e3a;
    --dj-beat: rgb(255 255 255 / 0.16);
    --dj-bar: rgb(253 230 138 / 0.4);
    --dj-playhead: #fde68a;
    --dj-band-low: #fbbf24;
    --dj-band-high: #ffffff;
    --dj-well: #081529;
    --dj-mix-gradient: linear-gradient(90deg, #10b981, #fde68a 50%, #f472b6);
    --dj-mix-ink: #0b1e3a;
  }
  .dj.hlook :global(:is(.deck, .mixer, .sampler, .hdj)) {
    border-radius: 22px;
  }
  .dj.hlook :global(.deck) {
    box-shadow: inset 0 5px 0 var(--c);
  }
  .dj.hlook :global(.sp8 button) {
    border-radius: 16px;
  }
  :global(:root[data-mode="light"]) .dj.hlook {
    --dj-a: #059669;
    --dj-a-ink: #ffffff;
    --dj-b: #db2777;
    --dj-b-ink: #ffffff;
    --dj-wave-bg: #e3f6ef;
    --dj-beat: rgb(11 30 58 / 0.14);
    --dj-bar: rgb(11 30 58 / 0.34);
    --dj-playhead: #0b1e3a;
    --dj-band-low: #d97706;
    --dj-band-high: #0b1e3a;
    --dj-well: #cdebe0;
    --dj-mix-gradient: linear-gradient(90deg, #059669, #fbbf24 50%, #db2777);
    --dj-mix-ink: #0b1e3a;
  }
  .row {
    display: grid;
    grid-template-columns: var(--cols);
    gap: var(--dj-gap);
    min-width: 0;
    min-height: 0;
  }
  /* Narrower than a wide board: the decks side by side, the mixer in a band under them (the layout's order still decides who is first) */
  @container dj (max-width: 1029px) {
    .row.stage {
      grid-template-columns: repeat(var(--deck-cols), minmax(0, 1fr));
    }
    .row.stage > :global(.mixer) {
      grid-column: 1 / -1;
      order: 1;
    }
  }
  /* A narrow window: the decks one above the other, the mixer between them as the layout says */
  @container dj (max-width: 679px) {
    .row.stage {
      grid-template-columns: minmax(0, 1fr);
    }
    .row.stage {
      align-content: start;
    }
    .row.stage > :global(.mixer) {
      order: 0;
    }
  }
  .dj > .row.stage {
    flex: 1 0 auto;
  }
  .notice {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    border-radius: var(--r-md);
    background: var(--warn-bg);
    border: 1px solid var(--warn-border);
    color: var(--warn);
    font-size: 13px;
  }
  .notice span {
    flex: 1;
  }
  .bare {
    margin: 24px auto;
    color: var(--muted);
  }
  .link {
    color: var(--primary);
    font-weight: 600;
  }
  .intro {
    max-width: 560px;
    margin: 8vh auto 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    text-align: center;
  }
  .intro p {
    margin: 0;
    line-height: 1.5;
  }
  .small {
    font-size: 13px;
  }
  .mark {
    display: grid;
    place-items: center;
    width: 72px;
    height: 72px;
    border-radius: 50%;
    background: var(--dj-mix-gradient, var(--primary));
    color: var(--dj-mix-ink);
  }
  .error {
    color: var(--danger);
  }
  .start {
    height: 48px;
    padding: 0 28px;
    font-size: 15px;
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
