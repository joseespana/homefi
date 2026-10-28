<script lang="ts">
  import { item } from "../lib/motion";
  import { untrack } from "svelte";
  import type { Paged } from "../lib/paged.svelte";
  import PageMore from "./PageMore.svelte";
  import { api, errorText, type Hit, type LocalPlaylist } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";
  import Skeleton from "./Skeleton.svelte";
  import TrackTable from "./TrackTable.svelte";

  // .m3u files in the music folders (tiddl writes these) and the Tidal account's playlists
  let local = $state<LocalPlaylist[]>([]);
  let tidalList = $state<Paged | null>(null);
  const tidal = $derived(tidalList?.items ?? []);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let filter = $state("");

  async function load(refresh = false) {
    loading = true;
    error = null;
    // Tidal's come page by page (shared with My Tidal), next to the local ones
    tidalList = app.account ? untrack(() => app.tidalList("playlists", refresh)) : null;
    try {
      local = await api.localPlaylists();
      if (refresh) await app.rescan();
    } catch (e) {
      error = errorText(e);
    } finally {
      loading = false;
    }
  }
  $effect(() => {
    void app.account;
    load();
  });

  const match = (text: string) => !filter.trim() || text.toLowerCase().includes(filter.trim().toLowerCase());
  const shownLocal = $derived(local.filter((p) => match(p.name)));
  const shownTidal = $derived(tidal.filter((h) => match(`${h.title} ${h.subtitle}`)));
  // a filter looks through every Tidal playlist, not only the pages loaded
  $effect(() => {
    if (filter.trim() && tidalList && !tidalList.done) tidalList.all().catch(() => {});
  });
  const tidalError = $derived(tidalList?.error && !tidalList.items.length ? tidalList.error : null);

  const view = $derived(app.view);
  const open = $derived(view.name === "playlist" ? local.find((p) => p.path === view.path) : undefined);
  // entries resolve to library files; ones that aren't on this computer are only counted
  const byPath = $derived(new Map(app.tracks.map((t) => [t.path, t])));
  const openTracks = $derived(open ? open.tracks.flatMap((p) => byPath.get(p) ?? []) : []);

  function play(shuffle = false) {
    if (openTracks.length) api.play(openTracks.map((t) => t.path), 0, shuffle, open ? { kind: "playlist", id: open.path, name: open.name } : null);
  }
  function openTidal(h: Hit) {
    app.view = { name: "tidal-item", kind: "playlist", id: h.id, back: app.view };
  }
</script>

{#if open}
  <header class="detail">
    <Cover path={openTracks[0]?.path ?? null} size={180} radius={14} icon="queue" />
    <div class="detail-info">
      <button class="back" onclick={() => (app.view = { name: "playlists" })}><Icon name="back" size={16} /> Playlists</button>
      <h1>{open.name}</h1>
      <div class="muted">
        {openTracks.length} songs{open.tracks.length > openTracks.length ? ` · ${open.tracks.length - openTracks.length} not on this computer` : ""}
      </div>
      <div class="dim ellipsis">{open.path}</div>
      <div class="actions">
        <button class="btn primary" onclick={() => play()} disabled={!openTracks.length}><Icon name="play" size={16} /> Play</button>
        <button class="btn tonal" onclick={() => play(true)} disabled={!openTracks.length}><Icon name="shuffle" size={16} /> Shuffle</button>
        <button
          class="btn tonal"
          disabled={!openTracks.length}
          onclick={(e) => app.openQueueMenu(e, open!.name, (next) => app.queueFiles(openTracks.map((t) => t.path), open!.name, next), { kind: "playlist", id: open!.path, name: open!.name })}
        >
          <Icon name="queue" size={16} /> Queue
        </button>
      </div>
    </div>
  </header>
  <TrackTable tracks={openTracks} />
{:else}
  <header class="head">
    <div>
      <h1>Playlists</h1>
      <div class="muted">{loading ? "Loading…" : `${local.length} on this computer · ${tidalList?.total ?? 0} on Tidal`}</div>
    </div>
    <div class="actions">
      <div class="filter"><Icon name="search" size={16} /><input placeholder="Filter playlists" bind:value={filter} /></div>
      <button class="icon-btn" title="Refresh" aria-label="Refresh" onclick={() => load(true)} disabled={loading}><Icon name="refresh" /></button>
      {#if app.account}
        <button class="btn tonal" onclick={() => (app.newPlaylist = { trackIds: [] })}><Icon name="plus" size={16} /> New playlist</button>
      {/if}
    </div>
  </header>
  {#if error}<p class="error">{error}</p>{/if}

  <h2>On this computer</h2>
  {#if loading && !local.length}
    <Skeleton kind="grid" count={6} />
  {:else if shownLocal.length}
    <div class="grid">
      {#each shownLocal as p (p.path)}
        <button class="card" in:item|global onclick={() => (app.view = { name: "playlist", path: p.path })}>
          <Cover path={p.tracks.find((t) => byPath.has(t)) ?? null} size="100%" radius={12} icon="queue" />
          <span class="title ellipsis">{p.name}</span>
          <span class="muted small">{p.tracks.length} songs</span>
        </button>
      {/each}
    </div>
  {:else if !loading}
    <p class="muted">No .m3u playlists in your music folders yet. Tidal downloads made with playlists create them.</p>
  {/if}

  <h2>On Tidal</h2>
  {#if !app.account}
    <p class="muted">
      <button class="link" onclick={() => (app.loginOpen = true)}>Connect Tidal</button> to see your playlists.
    </p>
  {:else if tidalError}
    <p class="error">{tidalError}</p>
  {:else if tidalList?.loading && !tidal.length}
    <Skeleton kind="grid" count={6} />
  {:else if shownTidal.length}
    <div class="grid">
      {#each shownTidal as h (h.id)}
        <button class="card" in:item|global onclick={() => openTidal(h)}>
          <Cover url={h.cover_url} size="100%" radius={12} icon="queue" />
          <span class="title ellipsis">{h.title}</span>
          <span class="muted small ellipsis">{h.subtitle}</span>
        </button>
      {/each}
      {#if tidalList && !filter.trim()}<PageMore list={tidalList} />{/if}
    </div>
  {:else if tidalList && !tidalList.loading && tidalList.done}
    <p class="muted">No playlists on Tidal yet.</p>
  {/if}
{/if}

<style>
  .head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 8px;
    flex-wrap: wrap;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .filter {
    display: flex;
    align-items: center;
    gap: 6px;
    padding-left: 10px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: min(999px, var(--r-max));
    color: var(--muted);
  }
  .filter input {
    border: 0;
    background: none;
    width: 200px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(168px, 1fr));
    gap: 22px 18px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 4px;
    text-align: left;
    padding: 8px;
    margin: -8px;
    border-radius: min(14px, var(--r-max));
    transition: background 0.15s;
    min-width: 0;
  }
  .card:hover {
    background: var(--glass-strong);
  }
  .card :global(.cover) {
    aspect-ratio: 1;
    margin-bottom: 6px;
  }
  .title {
    font-weight: 600;
  }
  .small {
    font-size: 12.5px;
  }
  .dim {
    color: var(--faint);
    font-size: 12px;
    max-width: 520px;
  }
  .detail {
    display: flex;
    gap: 24px;
    align-items: flex-end;
    margin-bottom: 24px;
  }
  .detail-info {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .back {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--muted);
    margin-bottom: 6px;
    font-size: 13px;
  }
  .back:hover {
    color: var(--text);
  }
  .link {
    color: var(--accent-text);
    text-decoration: underline;
  }
  .error {
    color: var(--danger);
  }
</style>
