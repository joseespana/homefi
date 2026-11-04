<script lang="ts">
  // A saved HomeFi playlist (Playlists → Made by HomeFi).
  import { onMount } from "svelte";
  import { playlists } from "../../lib/playlists.svelte";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";
  import BuildStatus from "./BuildStatus.svelte";
  import PlaylistPreview from "./PlaylistPreview.svelte";

  const view = $derived(app.view);
  // dev (mock): `#home-playlist` has no id, so it opens the newest one
  const playlist = $derived(view.name === "home-playlist" ? (playlists.list.find((p) => p.id === view.id) ?? (import.meta.env.DEV && !view.id ? playlists.list[0] : undefined)) : undefined);

  onMount(() => {
    playlists.loadSources();
    if (!playlists.loaded) playlists.load();
  });
</script>

<button class="back" onclick={() => (app.view = { name: "playlists" })}><Icon name="back" size={16} /> Playlists</button>
{#if playlist}
  <BuildStatus />
  <PlaylistPreview {playlist} saved={true} />
{:else if playlists.loaded}
  <p class="muted">This playlist isn't in HomeFi anymore.</p>
{/if}

<style>
  .back {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--muted);
    margin-bottom: 12px;
    font-size: 13px;
  }
  .back:hover {
    color: var(--text);
  }
</style>
