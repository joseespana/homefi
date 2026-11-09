<script lang="ts">
  import { api } from "../lib/api";
  import { pop } from "../lib/motion";
  import { lyricsShown, shareTitle, toggleLyrics, toggleShare } from "../lib/playerButtons.svelte";
  import { share } from "../lib/share.svelte";
  import { shownSource as src } from "../lib/source.svelte";
  import { sidePlayer } from "../lib/sidePlayer.svelte";
  import Icon from "./Icon.svelte";

  // `ghost`: the small buttons of the top bar (sharing has the rail and its status pill there).
  // `hw`: the player bar's own buttons, sharing included; `narrow` folds the three into a "more" menu.
  let { look, narrow = false, queueOpen = $bindable(false) }: { look: "ghost" | "hw"; narrow?: boolean; queueOpen?: boolean } = $props();

  const hasTrack = $derived(!!src.current);
  const sharedNow = $derived(share.sharing || share.inRoom);
  const guestCount = $derived(share.sharing ? share.hub.guests.length : 0);
  const buttonClass = $derived(look === "hw" ? "hw small" : "tb-btn");

  let moreOpen = $state(false);
  $effect(() => {
    if (!narrow) moreOpen = false;
  });
  const miniPlayer = () => api.miniPlayer(true);
</script>

{#snippet lyricsButton()}
  <button class={buttonClass} class:on={lyricsShown()} title="Lyrics" aria-label="Lyrics" aria-pressed={lyricsShown()} disabled={!hasTrack} onclick={toggleLyrics}>
    <Icon name="lyrics" size={16} />
  </button>
{/snippet}

{#snippet miniButton()}
  <button class={buttonClass} title="Mini player (always on top)" aria-label="Mini player" onclick={miniPlayer}>
    <Icon name="pip" size={16} />
  </button>
{/snippet}

{#snippet queueButton()}
  <button class={buttonClass} class:on={queueOpen} title="Queue" aria-label="Queue" aria-pressed={queueOpen} onclick={() => (queueOpen = !queueOpen)}>
    <Icon name="queue" size={16} />
  </button>
{/snippet}

{#snippet sideButton()}
  {#if sidePlayer.wanted}
    {@const on = !sidePlayer.hidden && !queueOpen}
    <button
      class={buttonClass}
      class:on
      title={on ? "Hide the side player" : "Show the side player"}
      aria-label="Side player"
      aria-pressed={on}
      onclick={() => (queueOpen ? ((queueOpen = false), (sidePlayer.hidden = false)) : sidePlayer.toggle())}
    >
      <Icon name="sidePanel" size={16} />
    </button>
  {/if}
{/snippet}

{#if look === "ghost"}
  {@render lyricsButton()}
  {@render sideButton()}
  {@render queueButton()}
  {@render miniButton()}
{:else if narrow}
  <div class="more">
    <button class="hw small" class:on={moreOpen || sharedNow || lyricsShown()} title="More" aria-label="More" aria-expanded={moreOpen} onclick={() => (moreOpen = !moreOpen)}>
      <Icon name="more" size={16} />
      {#if guestCount}<span class="count">{guestCount}</span>{/if}
    </button>
    {#if moreOpen}
      <button class="deck-scrim" aria-label="Close" onclick={() => (moreOpen = false)}></button>
      <div class="deck-menu" role="menu" transition:pop={{ origin: "bottom right", y: 8 }}>
        <button role="menuitem" class:on={sharedNow} onclick={() => ((moreOpen = false), toggleShare())}>
          <Icon name="cast" size={16} /> {shareTitle()}
        </button>
        <button role="menuitem" class:on={lyricsShown()} disabled={!hasTrack} onclick={() => ((moreOpen = false), toggleLyrics())}>
          <Icon name="lyrics" size={16} /> Lyrics
        </button>
        <button role="menuitem" onclick={() => ((moreOpen = false), miniPlayer())}>
          <Icon name="pip" size={16} /> Mini player
        </button>
      </div>
    {/if}
  </div>
  {@render queueButton()}
{:else}
  <button class="hw small" class:on={sharedNow} title={shareTitle()} aria-label="Shared control and party" onclick={toggleShare}>
    <Icon name="cast" size={16} />
    {#if guestCount}<span class="count">{guestCount}</span>{/if}
  </button>
  {@render lyricsButton()}
  {@render miniButton()}
  {@render sideButton()}
  {@render queueButton()}
{/if}

<style>
  .more {
    position: relative;
  }
  .count {
    position: absolute;
    top: -4px;
    right: -4px;
    min-width: 16px;
    height: 16px;
    padding: 0 4px;
    border-radius: var(--r-sm);
    background: var(--primary-strong);
    color: var(--on-primary-strong);
    font-size: 10px;
    font-weight: 700;
    line-height: 16px;
  }
  /* the top bar's buttons: quiet until pointed at; skins give them their own raised look */
  .tb-btn {
    display: grid;
    place-items: center;
    width: var(--tb-size, 30px);
    height: var(--tb-size, 30px);
    /* concentric with the group around it (its corner less its 3px padding) */
    border-radius: var(--r-xs);
    color: var(--muted);
  }
  .tb-btn:hover:not(:disabled) {
    background-color: var(--surface-2);
    color: var(--text);
  }
  .tb-btn.on {
    color: var(--accent-text);
  }
  .tb-btn:focus-visible {
    outline: 2px solid var(--accent-text);
    outline-offset: -2px;
  }
  .tb-btn:disabled {
    opacity: 0.45;
  }
</style>
