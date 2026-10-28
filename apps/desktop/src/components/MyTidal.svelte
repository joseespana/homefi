<script lang="ts">
  import { untrack } from "svelte";
  import { api, errorText, type Hit } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import AlbumCard from "./AlbumCard.svelte";
  import HitRow from "./HitRow.svelte";
  import Icon from "./Icon.svelte";
  import PageMore from "./PageMore.svelte";
  import Skeleton from "./Skeleton.svelte";
  import type { Paged } from "../lib/paged.svelte";
  import { Progressive, Windowed } from "../lib/virtual.svelte";

  type Tab = "playlists" | "albums" | "artists" | "tracks";
  const tabs: { id: Tab; label: string }[] = [
    { id: "playlists", label: "Playlists" },
    { id: "albums", label: "Albums" },
    { id: "artists", label: "Artists" },
    { id: "tracks", label: "Tracks" },
  ];
  const tab = $derived((app.view.name === "tidal" && app.view.tab) || "playlists");

  let list = $state<Paged | null>(null);
  /** The track rows near the screen; the grid's covers a chunk per frame. */
  const win = new Windowed(60);
  const grow = new Progressive();
  $effect(() => grow.track(shown.length));
  let filter = $state("");
  const items = $derived(list?.items ?? []);
  const loading = $derived(!!list?.loading);
  // the first page failed: nothing to show (a later page failing gets a retry row at the end)
  const error = $derived(list?.error && !list.items.length ? list.error : null);

  function load(t: Tab, force = false) {
    win.reset();
    list = app.tidalList(t, force);
  }

  $effect(() => {
    const t = tab;
    // only the account and the tab start a load (not the list's own state)
    if (app.account) untrack(() => load(t));
  });

  // another tab or view: the list stops streaming (it goes on when it's shown again)
  $effect(() => {
    const current = list;
    return () => current?.cancel();
  });

  // a filter looks through the whole collection, not only the pages on screen
  $effect(() => {
    if (filter.trim() && list && !list.done) list.all().catch(() => {});
  });

  const shown = $derived(
    filter.trim() ? items.filter((h) => `${h.title} ${h.subtitle}`.toLowerCase().includes(filter.trim().toLowerCase())) : items,
  );
  const missingTracks = $derived(tab === "tracks" ? shown.filter(isMissing) : []);
  function isMissing(h: Hit) {
    return !(h.isrc && app.localIsrc.has(h.isrc)) && (app.trackDownloads[h.id]?.state ?? "failed") === "failed";
  }
  // every favorite track, for playing on from a row and for "Download missing"
  const allTracks = () => (list && !filter.trim() ? list.all() : Promise.resolve(shown));

  async function downloadMissing() {
    try {
      const missing = (await allTracks()).filter(isMissing);
      if (missing.length) await app.download(`${missing.length} favorite tracks`, missing.map((h) => `track/${h.id}`), missing.map((h) => h.id));
    } catch (e) {
      app.notify(errorText(e), true);
    }
  }

</script>

<header class="head">
  <div>
    <h1>My Tidal</h1>
    <div class="muted">{loading && !items.length ? "Loading…" : filter.trim() ? `${shown.length} of ${list?.total ?? 0} ${tab}` : `${list?.total ?? shown.length} ${tab}`}</div>
  </div>
  <div class="actions">
    <div class="filter"><Icon name="search" size={16} /><input placeholder="Filter" bind:value={filter} /></div>
    <button class="icon-btn" title="Refresh" onclick={() => load(tab, true)} disabled={loading && !items.length}><Icon name="refresh" /></button>
    {#if tab === "playlists" && app.account}
      <button class="btn tonal" onclick={() => (app.newPlaylist = { trackIds: [] })}><Icon name="plus" size={16} /> New playlist</button>
    {/if}
    {#if tab === "tracks" && missingTracks.length}
      <button class="btn primary" onclick={downloadMissing}>
        <Icon name="download" size={16} /> Download {list?.done || filter.trim() ? missingTracks.length : ""} missing
      </button>
    {/if}
  </div>
</header>

<nav class="tabs">
  {#each tabs as t}
    <button class:active={tab === t.id} onclick={() => (app.view = { name: "tidal", tab: t.id })}>{t.label}</button>
  {/each}
</nav>

{#if !app.account}
  <div class="empty">
    <img src="/logo.svg" alt="" width="88" height="88" />
    <h2>Connect your Tidal account</h2>
    <p class="muted">See your playlists, albums, artists and favorite tracks here.</p>
    <button class="btn primary" onclick={() => (app.loginOpen = true)}>Connect Tidal</button>
  </div>
{:else if error}
  <p class="error">{error}</p>
{:else if loading && !shown.length}
  <Skeleton kind={tab === "tracks" ? "rows" : "grid"} round={tab === "artists"} count={tab === "tracks" ? 10 : 12} />
{:else if tab === "tracks"}
  <!-- only the rows near the screen exist -->
  <div class="list" {@attach win.attach(() => shown.length)}>
    <div aria-hidden="true" style:height="{win.before}px"></div>
    {#each shown.slice(win.start, win.end) as hit, j (hit.id)}
      <HitRow {hit} index={win.start + j + 1} list={shown} whole={allTracks} enter={!win.scrolled} />
    {/each}
    <div aria-hidden="true" style:height="{win.after}px"></div>
    {#if list && !filter.trim()}<PageMore {list} />{/if}
  </div>
{:else}
  <div class="grid">
    {#each shown.slice(0, grow.shown) as hit (hit.id)}
      <AlbumCard {hit} contained />
    {/each}
    {#if list && !filter.trim() && grow.shown >= shown.length}<PageMore {list} />{/if}
  </div>
{/if}

<style>
  .head {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    gap: 16px;
    flex-wrap: wrap;
    margin-bottom: 14px;
  }
  .actions {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .filter {
    display: flex;
    align-items: center;
    gap: 6px;
    padding-left: 10px;
    background: var(--glass-strong);
    border: 1px solid var(--glass-border);
    border-radius: min(999px, var(--r-max));
    color: var(--muted);
  }
  .filter input {
    border: 0;
    background: none;
    width: 180px;
  }
  .tabs {
    display: flex;
    gap: 6px;
    margin-bottom: 18px;
  }
  .tabs button {
    padding: 7px 16px;
    border-radius: min(999px, var(--r-max));
    background: var(--glass-strong);
    border: 1px solid var(--glass-border);
    color: var(--muted);
    font-weight: 600;
    transition: background 0.2s, border-color 0.2s, color 0.2s;
  }
  .tabs button.active {
    background: color-mix(in srgb, var(--room-accent) 25%, transparent);
    border-color: color-mix(in srgb, var(--room-accent) 60%, transparent);
    color: var(--text);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(168px, 1fr));
    gap: 22px 18px;
  }
  .list {
    display: flex;
    flex-direction: column;
  }
  .list > :global(.hit) {
    content-visibility: auto;
    contain-intrinsic-size: auto 60px;
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    padding: 60px 20px;
    gap: 8px;
  }
  .empty img {
    border-radius: min(20px, var(--r-max));
  }
  .error {
    color: var(--danger);
  }
</style>
