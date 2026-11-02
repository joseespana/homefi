<script lang="ts">
  // "Sound ▸": which sound holds this song, album, artist or playlist. A check
  // marks the one that covers it now; "Everything else" takes it out of its
  // sound; "New sound from AI…" opens Ask the AI for it. Used as a flyout of
  // the queue menu (`flyout`) and on its own from the album / playlist buttons.
  import type { TasteScope } from "../../lib/api";
  import { pop } from "../../lib/motion";
  import { holds, playsOn, sound } from "../../lib/sound.svelte";
  import Icon from "../Icon.svelte";

  let {
    scope,
    label,
    x = 0,
    y = 0,
    flyout = false,
    onclose,
  }: { scope: TasteScope; label: string; x?: number; y?: number; flyout?: boolean; onclose: () => void } = $props();

  /** Menu width and the space it keeps from the window's edge. */
  const WIDTH_PX = 230;
  const EDGE_PX = 12;
  const MAX_H_PX = 360;

  const output = $derived(sound.output);
  const holder = $derived(sound.rules.find((r) => holds(r, scope) && playsOn(r, output)) ?? sound.rules.find((r) => holds(r, scope)) ?? null);
  const left = $derived(Math.max(EDGE_PX, Math.min(x, window.innerWidth - WIDTH_PX - EDGE_PX)));
  const top = $derived(Math.max(EDGE_PX, Math.min(y, window.innerHeight - MAX_H_PX)));

  function pick(id: string) {
    onclose();
    sound.assign(id, scope);
  }
  function elsewhere() {
    onclose();
    if (holder) sound.unassign(holder.id, scope);
  }
  function ask() {
    onclose();
    sound.openAsk(`a sound for ${scope.name || label}`);
  }
</script>

<svelte:window onkeydown={(e) => !flyout && e.key === "Escape" && onclose()} />
{#if !flyout}
  <button class="scrim" aria-label="Close" onclick={onclose} oncontextmenu={(e) => (e.preventDefault(), onclose())}></button>
{/if}
<div class="menu" class:flyout style:left={flyout ? undefined : `${left}px`} style:top={flyout ? undefined : `${top}px`} role="menu" aria-label="Sound for {label}" transition:pop={{ origin: "top left" }}>
  <div class="head ellipsis">For “{label}”</div>
  {#each sound.rules as r (r.id)}
    <button class="row" role="menuitemradio" aria-checked={holder?.id === r.id} onclick={() => pick(r.id)}>
      <span class="tick">{#if holder?.id === r.id}<Icon name="check" size={14} />{/if}</span>
      <span class="ellipsis">{r.label}</span>
    </button>
  {/each}
  <button class="row" role="menuitemradio" aria-checked={!holder} onclick={elsewhere}>
    <span class="tick">{#if !holder}<Icon name="check" size={14} />{/if}</span>
    <span>Everything else</span>
  </button>
  <div class="sep"></div>
  <button class="row" role="menuitem" onclick={ask}><span class="tick"><Icon name="sparkles" size={14} /></span><span>New sound from AI…</span></button>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 44;
    cursor: default;
  }
  .menu {
    position: fixed;
    z-index: 45;
    width: 230px;
    max-height: 360px;
    overflow-y: auto;
    padding: 6px;
    border-radius: min(12px, var(--r-max));
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
  }
  .menu.flyout {
    position: absolute;
    left: calc(100% + 4px);
    top: -6px;
  }
  .head {
    padding: 4px 8px 6px;
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--faint);
  }
  .row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 8px;
    border-radius: min(8px, var(--r-max));
    text-align: left;
    font-size: 13px;
  }
  .row:hover {
    background: var(--surface-2);
  }
  .tick {
    flex: none;
    width: 16px;
    display: grid;
    place-items: center;
    color: var(--accent-text);
  }
  .sep {
    height: 1px;
    margin: 4px 6px;
    background: var(--border);
  }
</style>
