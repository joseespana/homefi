<script lang="ts">
  // The ride screen's TV button: its state (off / on) and a small menu for the window, the link and where
  // the music plays. The TV shows homefi-core's own page, so "open in a browser" is just the link.
  import { exercise } from "../../lib/exercise.svelte";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";
  import QrCode from "../share/QrCode.svelte";

  const QR_PX = 128;
  let open = $state(false);
  let qr = $state(false);
  const tv = $derived(exercise.tv);
  const on = $derived(tv.window || tv.server);

  async function copy() {
    if (!tv.url) await exercise.openTvServer();
    const url = exercise.tv.url;
    if (!url) return;
    try {
      await navigator.clipboard.writeText(url);
      app.note("Link copied. Open it on the TV's browser.", "success", "exercise-copy");
    } catch {
      app.note(url, "info", "exercise-copy");
    }
  }
  async function showQr() {
    if (!tv.url) await exercise.openTvServer();
    qr = !qr;
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && (open = false)} onclick={() => (open = false)} />

<span class="wrap" onclick={(e) => e.stopPropagation()} role="presentation">
  <button type="button" class="tv" class:on title="Show on a TV" aria-haspopup="menu" aria-expanded={open} aria-label="TV" onclick={() => (open = !open)}>
    <Icon name="monitor" size={18} />
    {#if on}<i class="dot" aria-hidden="true"></i>{/if}
  </button>
  {#if open}
    <div class="menu" role="menu">
      <button type="button" role="menuitemcheckbox" aria-checked={tv.window} onclick={() => exercise.toggleTvWindow()}>
        <Icon name="monitor" size={16} />TV window<span class="state">{tv.window ? "On" : "Off"}</span>
      </button>
      <button type="button" role="menuitem" onclick={copy}><Icon name="copy" size={16} />Open in browser (copy link)</button>
      <button type="button" role="menuitem" onclick={showQr}><Icon name="external" size={16} />Show QR</button>
      {#if qr && tv.url}<div class="qr"><QrCode text={tv.url} size={QR_PX} /></div>{/if}
      <div class="sep" role="separator"></div>
      <span class="cap">Music plays</span>
      <div class="two" role="radiogroup" aria-label="Music plays">
        <button type="button" role="radio" aria-checked={exercise.settings.tv_audio === "phone"} onclick={() => exercise.saveSettings({ tv_audio: "phone" })}>Here</button>
        <button type="button" role="radio" aria-checked={exercise.settings.tv_audio === "tv"} onclick={() => exercise.saveSettings({ tv_audio: "tv" })}>On the TV</button>
      </div>
    </div>
  {/if}
</span>

<style>
  .wrap {
    position: relative;
    display: inline-flex;
  }
  .tv {
    position: relative;
    width: var(--ride-hit);
    height: var(--ride-hit);
    border-radius: 50%;
    border: 1px solid var(--border);
    background: var(--surface-2);
    display: grid;
    place-items: center;
  }
  .tv:hover {
    background: var(--surface-3);
  }
  .dot {
    position: absolute;
    top: 6px;
    right: 6px;
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--ok);
    box-shadow: 0 0 0 2px var(--surface-2);
  }
  .menu {
    position: absolute;
    right: 0;
    top: calc(100% + 10px);
    z-index: 20;
    min-width: 270px;
    padding: 8px;
    border-radius: var(--r-md);
    background: var(--surface-3);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .menu > button {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 40px;
    padding: 0 10px;
    border-radius: var(--r-sm);
    text-align: left;
  }
  .menu > button:hover {
    background: var(--surface-2);
  }
  .state {
    margin-left: auto;
    color: var(--muted);
    font-size: 12px;
    font-weight: 700;
  }
  .qr {
    display: grid;
    place-items: center;
    padding: 6px;
  }
  .sep {
    height: 1px;
    background: var(--border);
    margin: 6px 4px;
  }
  .cap {
    padding: 0 10px;
    font-size: 12px;
    color: var(--muted);
    font-weight: 700;
  }
  .two {
    display: flex;
    gap: 6px;
    padding: 4px 6px 6px;
  }
  .two button {
    flex: 1;
    min-height: 36px;
    border-radius: var(--r-pill);
    border: 1px solid var(--border);
    font-weight: 600;
  }
  .two button[aria-checked="true"] {
    background: var(--primary);
    color: var(--on-primary);
    border-color: transparent;
  }
</style>
