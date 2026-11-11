<script lang="ts">
  // The AI chats menu: every open playlist chat (party or "Playlist with AI") with where it
  // stands, its last activity and a Close. Opens beside the Rail's button; clicking a chat brings
  // its screen back exactly as it was. The list and its rules live in `conversations`.
  import { agoWords, isWorking, statusWords } from "../../lib/conversations";
  import { pop } from "../../lib/motion";
  import { conversations } from "../../lib/party.svelte";
  import Icon from "../Icon.svelte";

  /** The Rail's button the menu grows up from. */
  let { anchor }: { anchor: HTMLElement | undefined } = $props();

  /** How often "5 min ago" is refreshed while the menu is open. */
  const REFRESH_MS = 30_000;
  /** Space kept between the menu and the window's bottom edge (px). */
  const EDGE_PX = 12;

  let now = $state(Date.now());
  $effect(() => {
    if (!conversations.menuOpen) return;
    now = Date.now();
    const timer = setInterval(() => (now = Date.now()), REFRESH_MS);
    return () => clearInterval(timer);
  });

  const bottom = $derived.by(() => {
    if (!conversations.menuOpen || !anchor) return EDGE_PX;
    return Math.max(EDGE_PX, window.innerHeight - anchor.getBoundingClientRect().bottom);
  });

  const close = () => (conversations.menuOpen = false);
  function key(e: KeyboardEvent) {
    if (conversations.menuOpen && e.key === "Escape") {
      e.preventDefault();
      close();
    }
  }
</script>

<svelte:window onkeydown={key} />

{#if conversations.menuOpen}
  <button class="scrim" aria-label="Close" tabindex="-1" onclick={close}></button>
  <div class="menu" role="menu" aria-label="AI chats" style:bottom="{bottom}px" transition:pop={{ origin: "bottom left", y: 0 }}>
    <h2>AI chats</h2>
    {#if conversations.list.length === 0}
      <p class="empty muted">No AI chats open. Start one from Playlists.</p>
    {/if}
    {#each conversations.list as c (c.id)}
      {@const status = c.status}
      <div class="row">
        <button class="open" role="menuitem" onclick={() => conversations.show(c.id)}>
          <Icon name="ai" size={17} />
          <span class="text">
            <span class="title ellipsis">{c.title}</span>
            <span class="meta" class:working={isWorking(status)} class:bad={status.kind === "failed"}>
              {statusWords(status)} · {agoWords(c.lastActivityMs, now)}
            </span>
          </span>
          {#if isWorking(status)}<span class="mdot" aria-hidden="true"></span>{/if}
        </button>
        <button class="x" role="menuitem" aria-label={`Close chat “${c.title}”`} title="Close" onclick={() => conversations.close(c.id)}><Icon name="x" size={14} /></button>
      </div>
    {/each}
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 49;
    background: transparent;
    cursor: default;
  }
  .menu {
    position: fixed;
    left: 84px;
    z-index: 50;
    width: min(340px, calc(100vw - 100px));
    max-height: calc(100vh - 24px);
    overflow-y: auto;
    padding: 6px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    border-radius: var(--r-md);
    background: var(--surface-solid);
    border: 1px solid var(--glass-border);
    box-shadow: var(--shadow);
  }
  h2 {
    margin: 0;
    padding: 8px 12px 6px;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .empty {
    margin: 0;
    padding: 6px 12px 10px;
    font-size: 13px;
    line-height: 1.45;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 2px;
    border-radius: var(--r-sm);
  }
  .row:hover,
  .row:focus-within {
    background: var(--glass-strong);
  }
  .open {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 46px;
    padding: 4px 8px 4px 12px;
    border-radius: var(--r-sm);
    color: var(--text);
    text-align: left;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .title {
    font-size: 13.5px;
  }
  .meta {
    font-size: 12px;
    color: var(--muted);
  }
  .meta.working {
    color: var(--accent-text);
  }
  .meta.bad {
    color: var(--danger);
  }
  .mdot {
    flex: none;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--room-accent);
    animation: pulse 1.2s infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.3;
    }
  }
  .x {
    flex: none;
    width: 32px;
    height: 32px;
    display: grid;
    place-items: center;
    border-radius: var(--r-sm);
    color: var(--muted);
  }
  .x:hover,
  .x:focus-visible {
    color: var(--text);
    background: var(--glass-border);
  }
</style>
