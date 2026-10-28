<script lang="ts">
  // "Customize" for the layouts that draw their own chrome: opens the room menu (layout and options).
  import RoomMenu from "../RoomMenu.svelte";
  import Icon from "../Icon.svelte";
  import { lyrics } from "../../lib/lyrics.svelte";
  import { useSource } from "../../lib/source.svelte";

  let { light = false }: { light?: boolean } = $props();
  let open = $state(false);
  const src = useSource();
</script>

<div class="customize">
  {#if src.current}
    <button class="chip" class:light onclick={() => lyrics.setOpen(true)} aria-label="Show the lyrics" title="Lyrics">
      <Icon name="lyrics" size={14} /> <span>Lyrics</span>
    </button>
  {/if}
  <button class="chip" class:light onclick={() => (open = !open)} aria-label="Customize the room" title="Customize the room">
    <Icon name="settings" size={14} /> <span>Customize</span>
  </button>
  {#if open}<RoomMenu onclose={() => (open = false)} />{/if}
</div>

<style>
  .customize {
    position: relative;
    flex: none;
    display: flex;
    gap: 6px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 11px;
    border-radius: min(999px, var(--r-max));
    font-size: 12px;
    color: rgb(255 255 255 / 0.85);
    background: rgb(0 0 0 / 0.3);
    border: 1px solid rgb(255 255 255 / 0.18);
  }
  .chip:hover {
    color: #fff;
    background: rgb(0 0 0 / 0.45);
  }
  .chip.light {
    color: #333;
    background: rgb(255 255 255 / 0.55);
    border-color: rgb(0 0 0 / 0.25);
  }
  .chip.light:hover {
    color: #000;
    background: rgb(255 255 255 / 0.8);
  }
  /* the menu keeps the app's own colors, whatever the skin around it */
  .customize :global(.menu) {
    color: var(--text);
    text-align: left;
  }
  @container (max-width: 520px) {
    .chip span {
      display: none;
    }
  }
</style>
