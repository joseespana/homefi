<script lang="ts">
  // The ride screen's TV button: its state (off / on) and a small menu for the second screen, a smart TV (HomeFi
  // on a Google TV finds the ride by itself; any other TV types what it shows in its browser) and where the music plays.
  import { exercise } from "../../lib/exercise.svelte";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";
  import Spinner from "../Spinner.svelte";

  let open = $state(false);
  let smart = $state(false);
  const tv = $derived(exercise.tv);
  const on = $derived(tv.window || tv.server);
  /** What a person types in a TV's browser: no "http://", no trailing slash. */
  const typed = $derived(tv.url ? tv.url.replace(/^https?:\/\//, "").replace(/\/$/, "") : "");

  /** "Smart TV": the TV page starts (a Google TV with HomeFi then finds the ride) and the two ways show. */
  async function showSmart() {
    smart = !smart;
    if (smart && !tv.url) await exercise.openTvServer();
  }
  async function copy() {
    if (!typed) return;
    try {
      await navigator.clipboard.writeText(typed);
      app.note("Copied", "success", "exercise-copy");
    } catch {
      app.note(typed, "info", "exercise-copy");
    }
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
      <button type="button" role="menuitemcheckbox" aria-checked={tv.window} aria-busy={exercise.working === "tv-window"} disabled={!!exercise.working} onclick={() => exercise.toggleTvWindow()}>
        <Icon name="monitor" size={16} />Second screen{#if exercise.working === "tv-window"}<span class="state"><Spinner size={14} /></span>{:else}<span class="state">{tv.window ? "On" : "Off"}</span>{/if}
      </button>
      <button type="button" role="menuitem" aria-expanded={smart} aria-busy={exercise.working === "tv-smart"} onclick={showSmart}>{#if exercise.working === "tv-smart"}<Spinner size={16} />{:else}<Icon name="external" size={16} />{/if}Smart TV</button>
      {#if smart}
        <div class="smart">
          <p><b>Google TV or Android TV:</b> open HomeFi on the TV and go to Train. The ride shows up there by itself.{#if exercise.settings.tv_audio === "tv"} For the music on the TV too, choose "Type what the phone shows" there and type the address below, all of it.{/if}</p>
          <p><b>Another smart TV:</b> in its web browser, type</p>
          {#if typed}<button type="button" class="addr" title="Copy" onclick={copy}>{typed}</button>{:else}<p>Getting it ready…</p>{/if}
        </div>
      {/if}
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
  .smart {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-width: 280px;
    padding: 4px 10px 6px;
    font-size: 13px;
    line-height: 1.4;
  }
  .smart p {
    margin: 0;
  }
  .smart .addr {
    font-family: var(--font-mono);
    font-size: 18px;
    font-weight: 800;
    color: var(--primary);
    text-align: left;
    user-select: text;
    overflow-wrap: anywhere;
  }
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
