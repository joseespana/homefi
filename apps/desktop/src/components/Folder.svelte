<script lang="ts">
  import { api, errorText, type Track } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import Icon from "./Icon.svelte";
  import Skeleton from "./Skeleton.svelte";
  import TrackTable from "./TrackTable.svelte";

  /** Plays any folder on disk without adding it to the library. */
  const path = $derived(app.view.name === "folder" ? app.view.path : "");
  let tracks = $state<Track[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);

  $effect(() => {
    if (!path) return;
    const p = path;
    loading = true;
    error = null;
    api.scanFolder(p).then(
      (t) => {
        if (p === path) tracks = t;
        loading = false;
      },
      (e) => {
        error = errorText(e);
        loading = false;
      },
    );
  });

  const inLibrary = $derived(
    !!app.settings && [app.settings.download_dir, ...app.settings.library_dirs].some((d) => path === d || path.startsWith(d + "/") || path.startsWith(d + "\\")),
  );
  const name = $derived(path.split(/[\\/]/).filter(Boolean).pop() ?? path);

  async function addToLibrary() {
    if (!app.settings) return;
    await app.saveSettings({ library_dirs: [...app.settings.library_dirs, path] });
  }
</script>

<header class="head">
  <div class="min">
    <div class="eyebrow">Folder</div>
    <h1 class="ellipsis">{name || "Open a folder"}</h1>
    <div class="muted ellipsis small">{path}</div>
    <div class="muted">{loading ? "Scanning…" : `${tracks.length} songs`}</div>
  </div>
  <div class="actions">
    <button class="btn tonal" onclick={() => pickFolder()}><Icon name="folder" size={16} /> Other folder</button>
    {#if path && !inLibrary}
      <button class="btn tonal" onclick={addToLibrary}>Add to library</button>
    {/if}
    <button class="btn tonal" onclick={() => api.play(tracks.map((t) => t.path), 0, true)} disabled={!tracks.length}><Icon name="shuffle" size={16} /> Shuffle</button>
    <button class="btn primary" onclick={() => api.play(tracks.map((t) => t.path))} disabled={!tracks.length}><Icon name="play" size={16} /> Play</button>
  </div>
</header>

{#if error}<p class="error">{error}</p>{/if}
{#if !loading && path && !tracks.length && !error}
  <p class="muted">No playable files here (FLAC, ALAC, WAV, AIFF, MP3, AAC, Vorbis…).</p>
{/if}
{#if loading && !tracks.length}<Skeleton count={10} />{:else}<TrackTable {tracks} />{/if}

<script lang="ts" module>
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { app as appState } from "../lib/state.svelte";

  /** Asks for a folder and opens it (remembered in "recent folders"). */
  export async function pickFolder() {
    const dir = await openDialog({ directory: true, title: "Play a folder" });
    if (typeof dir !== "string") return;
    appState.view = { name: "folder", path: dir };
    const recent = [dir, ...(appState.settings?.recent_folders ?? []).filter((d) => d !== dir)].slice(0, 8);
    await appState.saveSettings({ recent_folders: recent });
  }
</script>

<style>
  .head {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    gap: 16px;
    flex-wrap: wrap;
    margin-bottom: 18px;
  }
  .min {
    min-width: 0;
  }
  .eyebrow {
    font: 700 11px var(--font-mono);
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--accent-text);
  }
  .small {
    font-size: 12px;
  }
  .actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .error {
    color: var(--danger);
  }
</style>
