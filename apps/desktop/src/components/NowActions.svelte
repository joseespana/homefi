<script lang="ts">
  // What can be done with the track being heard: download it (when it streams
  // from Tidal), like it, add it to a playlist. `compact` = icons only (deck).
  import { useSource } from "../lib/source.svelte";
  import { app } from "../lib/state.svelte";
  import Icon from "./Icon.svelte";

  let { compact = false }: { compact?: boolean } = $props();
  // someone else's room: its songs aren't this computer's to like or download
  const src = useSource();

  const track = $derived(app.current);
  const id = $derived(app.currentTidalId);
  const local = $derived(app.currentIsLocal);
  const dl = $derived(id ? app.trackDownloads[id] : undefined);
  const fav = $derived(!!id && app.isFavorite("track", id));
  const ring = 2 * Math.PI * 9;

  // local files don't know their Tidal id: find it from the ISRC once
  $effect(() => {
    const isrc = track?.isrc;
    if (isrc && !id && app.account) app.lookupIsrc(isrc);
  });

  function download() {
    if (!id || !track) return;
    app.download(track.title, [`track/${id}`], [id]);
  }
</script>

{#if track && !src.remote}
  <div class="actions" class:compact>
    {#if local}
      <span class="have" title="This track is in your library">
        <Icon name="check" size={compact ? 14 : 15} />
        {#if !compact}In your library{/if}
      </span>
    {:else if dl?.state === "waiting"}
      <span class="state" title="Waiting for its turn"><Icon name="clock" size={16} />{#if !compact}Waiting…{/if}</span>
    {:else if dl?.state === "downloading"}
      <span class="state" title="Downloading · {Math.round(dl.progress * 100)}%">
        <svg viewBox="0 0 24 24" width="20" height="20" class="ring" class:spin={dl.progress === 0}>
          <circle cx="12" cy="12" r="9" class="track" />
          <circle cx="12" cy="12" r="9" class="bar" stroke-dasharray={ring} stroke-dashoffset={ring * (1 - (dl.progress || 0.25))} />
        </svg>
        {#if !compact}Downloading {Math.round(dl.progress * 100)}%{/if}
      </span>
    {:else if dl?.state === "done"}
      <span class="have"><Icon name="check" size={15} />{#if !compact}Downloaded{/if}</span>
    {:else if id && app.account}
      <button class="act primary" class:failed={dl?.state === "failed"} title={dl?.state === "failed" ? `Failed: ${dl.detail ?? ""} · retry` : "Download to your library"} onclick={download}>
        <Icon name={dl?.state === "failed" ? "alert" : "download"} size={16} />
        {#if !compact}{dl?.state === "failed" ? "Retry download" : "Download"}{/if}
      </button>
    {:else if !compact}
      <span class="muted stream">Streaming from Tidal</span>
    {/if}

    {#if id && app.account}
      <button class="act" class:fav title={fav ? "Remove from favorites" : "Add to favorites"} aria-pressed={fav} onclick={() => app.toggleFavorite("track", id!, track!.title)}>
        <Icon name={fav ? "heartFill" : "heart"} size={16} />
      </button>
      <button class="act" title="Add to playlist" aria-label="Add to playlist" onclick={(e) => app.pickPlaylist(e, [id!])}>
        <Icon name="plus" size={16} />
      </button>
    {/if}
  </div>
{/if}

<style>
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .compact {
    gap: 2px;
    flex-wrap: nowrap;
  }
  .act,
  .have,
  .state {
    height: 32px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 0 12px;
    border-radius: min(999px, var(--r-max));
    font-size: 12.5px;
    font-weight: 600;
  }
  .act {
    background: var(--glass-strong);
    border: 1px solid var(--glass-border);
    color: var(--text);
  }
  .act:hover {
    border-color: var(--room-accent);
  }
  .act.primary {
    color: var(--accent-text);
  }
  .compact .act,
  .compact .have,
  .compact .state {
    width: 32px;
    padding: 0;
    justify-content: center;
    background: none;
    border: 0;
  }
  .compact .act {
    color: var(--muted);
  }
  .compact .act:hover {
    color: var(--text);
    background: var(--glass-strong);
  }
  .compact .act.primary {
    color: var(--accent-text);
  }
  .have {
    color: var(--ok);
    background: color-mix(in srgb, var(--ok) 14%, transparent);
  }
  .state {
    color: var(--accent-text);
  }
  .fav,
  .compact .act.fav {
    color: #f472b6;
  }
  .failed {
    color: var(--danger) !important;
  }
  .stream {
    font-size: 12.5px;
  }
  .ring {
    transform: rotate(-90deg);
    fill: none;
    stroke-width: 2.5;
  }
  .ring.spin {
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(270deg);
    }
  }
  .ring .track {
    stroke: var(--glass-border);
  }
  .ring .bar {
    stroke: currentColor;
    stroke-linecap: round;
    transition: stroke-dashoffset 0.25s;
  }
</style>
