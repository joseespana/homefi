<script lang="ts">
  import { pop } from "../lib/motion";
  import Skeleton from "./Skeleton.svelte";
  // "Add to playlist" menu: the user's own Tidal playlists plus "New playlist".
  import { errorText } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";

  const picker = $derived(app.playlistPicker!);
  let error = $state<string | null>(null);
  let filter = $state("");

  $effect(() => {
    app.loadMyPlaylists().catch((e) => (error = errorText(e)));
  });

  const shown = $derived(
    (app.myPlaylists ?? []).filter((p) => p.title.toLowerCase().includes(filter.trim().toLowerCase())),
  );
  // keep the menu on screen
  const left = $derived(Math.max(12, Math.min(picker.x - 280, window.innerWidth - 292)));
  const top = $derived(Math.min(picker.y, window.innerHeight - 380));

  const close = () => (app.playlistPicker = null);

  function add(p: (typeof shown)[number]) {
    const ids = picker.trackIds;
    close();
    app.addToPlaylist(p, ids);
  }

  function create() {
    const ids = picker.trackIds;
    close();
    app.newPlaylist = { trackIds: ids };
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

<button class="scrim" aria-label="Close" onclick={close}></button>
<div class="menu" style:left="{left}px" style:top="{top}px" role="menu" transition:pop>
  <div class="head">Add {picker.trackIds.length === 1 ? "to playlist" : `${picker.trackIds.length} tracks to playlist`}</div>
  <button class="row new" onclick={create} role="menuitem">
    <span class="plus"><Icon name="plus" size={16} /></span> New playlist…
  </button>
  {#if (app.myPlaylists?.length ?? 0) > 6}
    <input class="filter" placeholder="Find a playlist" bind:value={filter} />
  {/if}
  <div class="list">
    {#if error}
      <p class="error">{error}</p>
    {:else if !app.myPlaylists}
      <Skeleton count={4} />
    {:else if !shown.length}
      <p class="muted">{filter ? "No match" : "You haven't created playlists yet"}</p>
    {/if}
    {#each shown as p (p.id)}
      <button class="row" onclick={() => add(p)} role="menuitem">
        <Cover url={p.cover_url} size={32} radius={6} icon="queue" />
        <div class="grow">
          <div class="ellipsis">{p.title}</div>
          <div class="muted ellipsis small">{p.subtitle}</div>
        </div>
      </button>
    {/each}
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
    cursor: default;
  }
  .menu {
    position: fixed;
    z-index: 41;
    width: 280px;
    max-height: 370px;
    display: flex;
    flex-direction: column;
    padding: 8px;
    border-radius: min(14px, var(--r-max));
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
  }
  .head {
    padding: 6px 8px 8px;
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
  }
  .list {
    overflow-y: auto;
    min-height: 0;
  }
  .row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 8px;
    border-radius: min(8px, var(--r-max));
    text-align: left;
  }
  .row:hover {
    background: var(--surface-2);
  }
  .new {
    font-weight: 600;
    color: var(--primary);
  }
  .plus {
    width: 32px;
    height: 32px;
    display: grid;
    place-items: center;
    border-radius: min(6px, var(--r-max));
    background: color-mix(in srgb, var(--primary) 16%, transparent);
  }
  .filter {
    margin: 4px 0 6px;
    padding: 7px 10px;
    border-radius: min(8px, var(--r-max));
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text);
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .small {
    font-size: 11.5px;
  }
  p {
    margin: 8px;
    font-size: 12.5px;
  }
</style>
