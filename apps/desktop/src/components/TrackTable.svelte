<script lang="ts">
  import { item } from "../lib/motion";
  import { api, formatTime, type Track } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";

  let { tracks, numbered = false, showAlbum = true }: { tracks: Track[]; numbered?: boolean; showAlbum?: boolean } = $props();

  const currentPath = $derived(app.current?.path);
  const playing = $derived(app.player?.status === "playing");

  // render in pages so huge libraries stay smooth
  let limit = $state(200);
  const visible = $derived(tracks.slice(0, limit));

  function queueMenu(e: MouseEvent, track: Track) {
    app.openQueueMenu(e, track.title, (next) => app.queueFiles([track.path], track.title, next));
  }

  // the track being heard pauses / resumes instead of starting over
  function play(index: number) {
    if (tracks[index]?.path === currentPath) return api.toggle();
    api.play(
      tracks.map((t) => t.path),
      index,
    );
  }
</script>

<div class="table" role="list">
  {#each visible as track, i (track.path)}
    {@const current = track.path === currentPath}
    <!-- double-click is a mouse shortcut; keyboard users use the play button -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      class="row"
      in:item|global
      class:noalbum={!showAlbum}
      class:now-row={current}
      class:live={current && playing}
      role="listitem"
      ondblclick={() => play(i)}
      oncontextmenu={(e) => queueMenu(e, track)}
    >
      <button class="lead" onclick={() => play(i)} aria-label="{current && playing ? 'Pause' : 'Play'} {track.title}">
        {#if current && playing}
          <span class="sound"><Icon name="volume" size={16} /></span>
        {:else if numbered}
          <span class="num">{track.trackNumber || "–"}</span>
        {:else}
          <Cover path={track.path} size={40} radius={6} />
        {/if}
        <span class="hover-play"><Icon name={current && playing ? "pause" : "play"} size={16} /></span>
      </button>
      <div class="main">
        <div class="title ellipsis" class:now-title={current}>{track.title}</div>
        <div class="sub ellipsis">{track.artist || "Unknown artist"}</div>
      </div>
      {#if showAlbum}
        <div class="album ellipsis muted">{track.album}</div>
      {/if}
      <div class="quality"><span class="badge" title={track.quality}>{track.quality}</span></div>
      <div class="time muted">{track.durationMs ? formatTime(track.durationMs) : ""}</div>
      <button class="icon-btn more-btn" title="Play next / add to queue" aria-label="Queue {track.title}" onclick={(e) => queueMenu(e, track)}>
        <Icon name="queue" size={16} />
      </button>
    </div>
  {/each}
  {#if tracks.length > limit}
    <button class="more btn tonal" onclick={() => (limit += 500)}>Show more ({tracks.length - limit})</button>
  {/if}
</div>

<style>
  .table {
    display: flex;
    flex-direction: column;
  }
  .row {
    display: grid;
    grid-template-columns: 48px minmax(180px, 2fr) minmax(120px, 1.2fr) minmax(130px, auto) 56px 32px;
    align-items: center;
    gap: 14px;
    padding: 6px 10px;
    border-radius: min(8px, var(--r-max));
    outline: none;
  }
  .row.noalbum {
    grid-template-columns: 48px minmax(180px, 2fr) minmax(130px, auto) 56px 32px;
  }
  .row:hover,
  .row:focus-within {
    background: var(--surface-2);
  }
  /* shows on hover, like the other secondary row actions */
  .more-btn {
    opacity: 0;
    transition: opacity 0.15s;
  }
  .row:hover .more-btn,
  .more-btn:focus-visible {
    opacity: 1;
  }
  .lead {
    position: relative;
    width: 40px;
    height: 40px;
    display: grid;
    place-items: center;
  }
  .hover-play {
    position: absolute;
    inset: 0;
    display: none;
    place-items: center;
    border-radius: min(6px, var(--r-max));
    background: rgb(0 0 0 / 0.55);
    color: #fff;
  }
  .row:hover .hover-play {
    display: grid;
  }
  .num {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .main {
    min-width: 0;
  }
  .title {
    font-weight: 500;
  }
  .sub {
    font-size: 12.5px;
    color: var(--muted);
  }
  .album {
    font-size: 13px;
  }
  .quality {
    text-align: right;
  }
  .time {
    text-align: right;
    font-variant-numeric: tabular-nums;
    font-size: 13px;
  }
  .sound {
    display: grid;
    color: var(--accent-text);
  }
  .more {
    align-self: center;
    margin: 12px;
  }
</style>
