<script lang="ts">
  import { item } from "../lib/motion";
  import { api, formatTime, type SourceContext, type Track } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import { Windowed } from "../lib/virtual.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";

  let {
    tracks,
    numbered = false,
    showAlbum = true,
    source = null,
  }: { tracks: Track[]; numbered?: boolean; showAlbum?: boolean; source?: SourceContext | null } = $props();

  const currentPath = $derived(app.current?.path);
  const playing = $derived(app.player?.status === "playing");

  // only the rows near the screen exist, so a library of thousands scrolls like a short list
  const win = new Windowed(52);

  function queueMenu(e: MouseEvent, track: Track) {
    app.openQueueMenu(e, track.title, (next) => app.queueFiles([track.path], track.title, next), { kind: "track", id: track.path, name: track.title });
  }

  // the track being heard pauses / resumes instead of starting over
  function play(index: number) {
    if (tracks[index]?.path === currentPath) return api.toggle();
    api.play(
      tracks.map((t) => t.path),
      index,
      false,
      source,
    );
  }
</script>

<div class="table" role="list" {@attach win.attach(() => tracks.length)}>
  <div aria-hidden="true" style:height="{win.before}px"></div>
  {#each tracks.slice(win.start, win.end) as track, j (track.path)}
    {@const i = win.start + j}
    {@const current = track.path === currentPath}
    <!-- double-click is a mouse shortcut; keyboard users use the play button -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      class="row"
      data-row
      in:item|global={{ skip: win.scrolled }}
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
  <div aria-hidden="true" style:height="{win.after}px"></div>
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
    padding: 0;
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
</style>
