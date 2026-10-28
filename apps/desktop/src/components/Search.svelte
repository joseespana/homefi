<script lang="ts">
  import { api, errorText, type Hit, type SearchResults } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import HitRow from "./HitRow.svelte";
  import Icon from "./Icon.svelte";
  import Skeleton from "./Skeleton.svelte";

  let query = $state("");
  let results = $state<SearchResults | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;

  const LINK = /https?:\/\/\S*tidal\.com\/\S+|^(track|album|playlist|artist|mix|video)\/\S+$/i;
  const isLink = $derived(LINK.test(query.trim()));

  function onInput() {
    clearTimeout(timer);
    error = null;
    if (!query.trim() || isLink) return;
    timer = setTimeout(run, 350);
  }

  async function run() {
    loading = true;
    try {
      results = await api.search(query);
    } catch (e) {
      error = errorText(e);
    } finally {
      loading = false;
    }
  }

  async function downloadLink() {
    try {
      await api.download(query.trim(), [query.trim()]);
      query = "";
      app.view = { name: "downloads" };
    } catch (e) {
      error = errorText(e);
    }
  }

  const sections = $derived(
    results
      ? ([
          ["Top hit", results.top_hit ? [results.top_hit] : []],
          ["Tracks", results.tracks.slice(0, 10)],
          ["Albums", results.albums.slice(0, 8)],
          ["Artists", results.artists.slice(0, 6)],
          ["Playlists", results.playlists.slice(0, 6)],
        ] as [string, Hit[]][])
      : [],
  );
</script>

<header class="head">
  <h1>Search Tidal</h1>
</header>

{#if !app.account}
  <div class="empty">
    <img src="/logo.svg" alt="" width="88" height="88" />
    <h2>Connect your Tidal account</h2>
    <p class="muted">Search the catalog and download in up to 24-bit / 192 kHz FLAC.</p>
    <button class="btn primary" onclick={() => (app.loginOpen = true)}>Connect Tidal</button>
  </div>
{:else}
  <div class="searchbox">
    <Icon name="search" />
    <!-- svelte-ignore a11y_autofocus -->
    <input
      autofocus
      placeholder="Artists, albums, tracks… or paste a Tidal link"
      bind:value={query}
      oninput={onInput}
      onkeydown={(e) => e.key === "Enter" && (isLink ? downloadLink() : run())}
    />
    {#if loading}<span class="spinner"></span>{/if}
  </div>

  {#if isLink}
    <div class="link-card">
      <Icon name="download" />
      <div class="grow">
        <div><b>Tidal link</b></div>
        <div class="muted ellipsis">{query.trim()}</div>
      </div>
      <button class="btn primary" onclick={downloadLink}>Download</button>
    </div>
  {/if}

  {#if error}<p class="error">{error}</p>{/if}

  {#if loading && !results}<Skeleton count={8} />{/if}

  {#each sections as [title, hits]}
    {#if hits.length}
      <h2>{title}</h2>
      <div class="hits">
        {#each hits as hit (hit.kind + hit.id + title)}
          <HitRow {hit} />
        {/each}
      </div>
    {/if}
  {/each}
{/if}

<style>
  .head {
    margin-bottom: 16px;
  }
  .searchbox {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 16px;
    height: 48px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: min(999px, var(--r-max));
    color: var(--muted);
  }
  .searchbox input {
    flex: 1;
    border: 0;
    background: none;
    font-size: 15px;
  }
  .spinner {
    width: 16px;
    height: 16px;
    border: 2px solid var(--surface-3);
    border-top-color: var(--primary);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .link-card {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-top: 16px;
    padding: 16px;
    border-radius: min(14px, var(--r-max));
    background: var(--surface-3);
  }
  .hits {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
    gap: 4px 12px;
  }
  .error {
    color: var(--danger);
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    padding: 80px 20px;
    gap: 8px;
  }
  .empty img {
    border-radius: min(20px, var(--r-max));
  }
</style>
