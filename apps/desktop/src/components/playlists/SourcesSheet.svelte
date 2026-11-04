<script lang="ts">
  // Where a playlist came from: the pages the AI read, and Google's Search Suggestions
  // when it was Gemini (always shown when present: Google requires it).
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { drawer, fade } from "../../lib/motion";
  import type { HomePlaylist } from "../../lib/api";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";
  import SearchSuggestions from "./SearchSuggestions.svelte";

  let { playlist, providerName, onclose }: { playlist: HomePlaylist; providerName: string; onclose: () => void } = $props();

  const showPages = $derived(app.settings?.playlists.show_sources !== false && playlist.sources.length > 0);
  const byAi = $derived(playlist.origin.kind === "concert" && playlist.origin.source.kind === "ai");

  let panel = $state<HTMLElement>();
  $effect(() => panel?.focus({ preventScroll: true }));

  function key(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onclose();
    } else if (e.key === "Tab" && panel) {
      // keep the focus inside the sheet
      const items = [...panel.querySelectorAll<HTMLElement>("button, a[href], iframe")];
      if (!items.length) return;
      const i = items.indexOf(document.activeElement as HTMLElement);
      if (e.shiftKey && i <= 0) (e.preventDefault(), items[items.length - 1].focus());
      else if (!e.shiftKey && i === items.length - 1) (e.preventDefault(), items[0].focus());
    }
  }
</script>

<svelte:window onkeydown={key} />

<button class="scrim" aria-label="Close" onclick={onclose} transition:fade></button>
<div class="sheet" bind:this={panel} tabindex="-1" role="dialog" aria-modal="true" aria-label="Sources" transition:drawer={{ x: 60 }}>
  <header>
    <h2>Sources</h2>
    <button class="icon-btn" aria-label="Close" onclick={onclose}><Icon name="x" size={16} /></button>
  </header>
  {#if byAi}
    <p class="muted">Found by {providerName} searching the web. Check the pages below.</p>
  {/if}
  {#if showPages}
    <ul>
      {#each playlist.sources as s (s.url)}
        <li>
          <button class="link" onclick={() => openUrl(s.url).catch(() => {})}>
            <span class="t ellipsis">{s.title || s.url}</span>
            <span class="u ellipsis muted">{s.url}</span>
          </button>
          <Icon name="external" size={14} />
        </li>
      {/each}
    </ul>
  {/if}
  {#if playlist.search_widget_html}
    <h3>Google Search suggestions</h3>
    <SearchSuggestions html={playlist.search_widget_html} />
  {/if}
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 30;
    background: rgb(4 10 18 / 0.45);
    cursor: default;
  }
  .sheet {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    z-index: 31;
    width: min(440px, 100vw);
    overflow-y: auto;
    padding: 24px 20px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    background: var(--surface);
    border-left: 1px solid var(--border);
    box-shadow: var(--shadow);
    outline: none;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  h2,
  h3 {
    margin: 0;
  }
  h3 {
    font-size: 14px;
  }
  p {
    margin: 0;
    line-height: 1.45;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    border-radius: min(10px, var(--r-max));
    background: var(--surface-2);
    color: var(--muted);
  }
  .link {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    text-align: left;
    color: var(--text);
  }
  .link:hover .t {
    text-decoration: underline;
  }
  .u {
    font-size: 12px;
  }
</style>
