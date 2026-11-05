<script lang="ts">
  import { busy } from "../lib/busy.svelte";
  import { buttonsOnTop, toggleShare } from "../lib/playerButtons.svelte";
  import { share } from "../lib/share.svelte";
  import { sound } from "../lib/sound.svelte";
  import { shownSource as src } from "../lib/source.svelte";
  import { app } from "../lib/state.svelte";
  import Icon from "./Icon.svelte";
  import PlayerButtons from "./PlayerButtons.svelte";
  import HowSounds from "./sound/HowSounds.svelte";

  let { queueOpen = $bindable(false) }: { queueOpen?: boolean } = $props();

  const onTop = $derived(buttonsOnTop());
  // sharing, or inside someone's room: the rail's Share button opens the panel, this says it is live
  const statusOn = $derived(share.sharing || share.inRoom);
  const guests = $derived(share.hub.guests.length);
  const statusText = $derived(share.sharing ? `Sharing: ${guests} ${guests === 1 ? "guest" : "guests"}` : `In ${share.guest.session_name}`);
</script>

<!-- the strip above the content: drags the window, shows the loading bar and (top mode) the player's side buttons -->
<div class="topbar drag" data-tauri-drag-region>
  {#if busy.count > 0 || app.scanning}<div class="loadbar" role="progressbar" aria-label="Loading"></div>{/if}
  {#if onTop}
    <div class="grow" data-tauri-drag-region></div>
    {#if statusOn}
      <button class="status" title={statusText} aria-label="{share.sharing ? `Sharing, ${guests} ${guests === 1 ? 'guest' : 'guests'}` : statusText}. Open" onclick={toggleShare}>
        <span class="dot" aria-hidden="true"></span>
        <Icon name="cast" size={14} />
        <span class="text ellipsis">{statusText}</span>
        {#if share.sharing && guests}<span class="badge">{guests}</span>{/if}
      </button>
    {/if}
    <div class="cluster" role="toolbar" aria-label="Player tools">
      {#if sound.ui.react_deck && !src.remote}<HowSounds place="top" look="tb" />{/if}
      <PlayerButtons look="ghost" bind:queueOpen />
    </div>
  {/if}
</div>

<style>
  .topbar {
    position: sticky;
    top: 0;
    z-index: 7;
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    height: var(--topbar-h, 34px);
    padding-right: clamp(18px, 3.2vw, 40px);
  }
  .grow {
    flex: 1;
    align-self: stretch;
  }
  /* indeterminate bar along the top while anything slow is loading */
  .loadbar {
    position: absolute;
    inset: 0 0 auto;
    height: 2px;
    overflow: hidden;
    background: color-mix(in srgb, var(--accent-text) 15%, transparent);
  }
  .loadbar::after {
    content: "";
    position: absolute;
    inset: 0 auto 0 0;
    width: 40%;
    background: linear-gradient(90deg, transparent, #1d4ed8, #0891b2, #10b981, transparent);
    animation: loadbar 1.2s ease-in-out infinite;
  }
  @keyframes loadbar {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(250%);
    }
  }
  .cluster,
  .status {
    display: flex;
    align-items: center;
    height: 34px;
    border-radius: var(--r-sm);
    background: var(--glass-strong);
    border: 1px solid var(--glass-border);
  }
  .cluster {
    gap: 2px;
    padding: 3px;
    box-sizing: border-box;
  }
  .status {
    gap: 6px;
    max-width: 240px;
    padding: 0 12px 0 10px;
    color: var(--accent-text);
    font-size: 12.5px;
    font-weight: 600;
    box-sizing: border-box;
  }
  .status:hover {
    color: var(--text);
  }
  .status:focus-visible {
    outline: 2px solid var(--accent-text);
    outline-offset: -2px;
  }
  .dot {
    width: 6px;
    height: 6px;
    flex: none;
    border-radius: 50%;
    background: var(--ok);
    box-shadow: 0 0 6px var(--ok);
  }
  .badge {
    display: none;
    min-width: 16px;
    height: 16px;
    padding: 0 4px;
    border-radius: var(--r-sm);
    background: var(--primary-strong);
    color: var(--on-primary-strong);
    font-size: 10px;
    font-weight: 700;
    line-height: 16px;
    text-align: center;
  }
  /* narrow window: the status is the icon with a count */
  @media (max-width: 920px) {
    .status {
      padding: 0 8px;
    }
    .status .text,
    .status .dot {
      display: none;
    }
    .status .badge {
      display: block;
    }
  }
</style>
