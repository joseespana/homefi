<script lang="ts">
  // A panel that slides in from the right over the Sound screen (Edit sound,
  // the output's own curve): the screen behind is dimmed, a click on it or Esc
  // closes it. Esc first closes whatever the panel has open inside (`onescape`
  // returns true when it did).
  import type { Snippet } from "svelte";
  import { drawer, fade } from "../../lib/motion";

  let {
    label,
    wide = false,
    onclose,
    onescape,
    children,
  }: { label: string; wide?: boolean; onclose: () => void; onescape?: () => boolean; children: Snippet } = $props();

  let panel = $state<HTMLElement | null>(null);
  $effect(() => panel?.focus({ preventScroll: true }));

  function key(e: KeyboardEvent) {
    if (e.key !== "Escape" || e.defaultPrevented) return;
    e.preventDefault();
    if (!onescape?.()) onclose();
  }
</script>

<svelte:window onkeydown={key} />

<button class="scrim" aria-label="Close" onclick={onclose} transition:fade></button>
<div class="panel" class:wide bind:this={panel} tabindex="-1" role="dialog" aria-label={label} transition:drawer={{ x: 60 }}>
  {@render children()}
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0 0 var(--set-deck-h) 0;
    z-index: 30;
    background: rgb(4 10 18 / 0.45);
    cursor: default;
  }
  .panel {
    position: fixed;
    top: 0;
    right: 0;
    bottom: var(--set-deck-h);
    z-index: 31;
    width: min(440px, 100vw);
    overflow-y: auto;
    padding: 56px 20px 24px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    background: var(--surface);
    border-left: 1px solid var(--border);
    box-shadow: var(--shadow);
    outline: none;
    container-type: inline-size;
  }
  .panel.wide {
    width: min(760px, 100vw);
  }
</style>
