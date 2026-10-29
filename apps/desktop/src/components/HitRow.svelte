<script lang="ts">
  import { item } from "../lib/motion";
  import { api, errorText, formatTime, type Hit, type SourceContext } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";
  import { snap } from "../lib/snap";

  /** A Tidal track / album / playlist / artist with download (or play, when already local). */
  let {
    hit,
    index = null,
    list = null,
    whole = null,
    onremove = null,
    source = null,
    enter = true,
  }: {
    hit: Hit;
    index?: number | null;
    list?: Hit[] | null;
    /** The whole of `list` when it is still loading pages: playing goes on through all of it. */
    whole?: (() => Promise<Hit[]>) | null;
    onremove?: (() => void) | null;
    /** Where `list` comes from (an album, a playlist), for taste layers and album loudness. */
    source?: SourceContext | null;
    /** Play the entrance (false for rows a scroll brings in). */
    enter?: boolean;
  } = $props();
  const canFavorite = $derived(!!app.account && hit.kind !== "mix" && hit.kind !== "video");
  const fav = $derived(app.isFavorite(hit.kind, hit.id));

  const icons: Record<string, string> = { track: "music", album: "disc", artist: "user", playlist: "queue", video: "monitor", mix: "queue" };
  const local = $derived(hit.kind === "track" && hit.isrc ? app.localIsrc.get(hit.isrc) : undefined);
  let error = $state<string | null>(null);
  // per-track state from the download events; albums / playlists follow their job
  const dl = $derived(hit.kind === "track" ? app.trackDownloads[hit.id] : undefined);
  const job = $derived(hit.kind !== "track" ? app.jobs[hit.title] : undefined);
  const ring = 2 * Math.PI * 9;
  // a 48 px row needs Tidal's 160 px image, not the 320 px one (a quarter of the bytes to fetch and decode)
  const thumb = (url: string | null) => url?.replace(/\/320x320\.jpg$/, "/160x160.jpg") ?? null;

  // delete animation: "Deleting…" while the file goes to the Trash, then the row turns to dust
  // and comes back without its downloaded mark
  let row = $state<HTMLElement>();
  const deleting = $derived(app.deletingIds.has(hit.id));
  const wave = $derived(app.snapping[hit.id]);
  let dusted = false;
  $effect(() => {
    if (!row) return;
    if (wave != null && !dusted) {
      dusted = true;
      const el = row;
      setTimeout(() => snap(el, { reform: false }), Math.min(wave, 25) * 60);
    } else if (wave == null && dusted) {
      dusted = false;
      row.style.opacity = "";
      row.style.transform = "";
      row.animate([{ opacity: 0, transform: "scale(0.98)" }, { opacity: 1, transform: "none" }], { duration: 380, easing: "ease-out" });
    }
  });
  const openable = $derived(hit.kind === "album" || hit.kind === "playlist" || hit.kind === "artist" || hit.kind === "mix");

  async function download() {
    try {
      error = null;
      await app.download(hit.title, [`${hit.kind}/${hit.id}`], hit.kind === "track" ? [hit.id] : []);
    } catch (e) {
      error = errorText(e);
    }
  }

  // the track being heard (same Tidal id, or same recording when it plays from a local file)
  const isCurrent = $derived(
    hit.kind === "track" && (app.currentTidalId === hit.id || (!!hit.isrc && app.currentIsrc === hit.isrc)),
  );
  const playing = $derived(isCurrent && app.player?.status === "playing");

  /** Plays this track, followed by the rest of `list`; the current one pauses / resumes. */
  async function play() {
    if (isCurrent) return api.toggle();
    let siblings = list ?? [hit];
    try {
      if (whole) siblings = await whole();
    } catch (e) {
      app.notify(errorText(e), true);
    }
    const at = siblings.indexOf(hit);
    app.playTidal(siblings, at >= 0 ? at : Math.max(0, siblings.findIndex((h) => h.id === hit.id)), false, source);
  }

  function queueMenu(e: MouseEvent) {
    if (hit.versions && hit.versions.length > 1) return app.openVersions(e, hit.title, hit.versions);
    if (hit.kind === "track") app.openQueueMenu(e, hit.title, (next) => app.queueHits([hit], hit.title, next), { kind: "track", id: hit.id, name: hit.title });
  }

  function open() {
    if (openable) app.view = { name: "tidal-item", kind: hit.kind as "album", id: hit.id, back: app.view };
  }
</script>

<div class="hit" data-row in:item|global={{ skip: !enter }} class:openable class:track={hit.kind === "track"} class:now-row={isCurrent} class:live={playing} class:deleting bind:this={row} role="group" aria-label={hit.title} oncontextmenu={queueMenu}>
  {#if index != null}
    <!-- the one play control of a track row: the number (or the cover, below) -->
    {#if hit.kind === "track"}
      <button class="n go-btn" onclick={play} aria-label="{playing ? 'Pause' : isCurrent ? 'Resume' : 'Play'} {hit.title}" title={playing ? "Pause" : isCurrent ? "Resume" : "Play"}>
        {#if playing}<span class="num sound"><Icon name="volume" size={14} /></span>{:else}<span class="num">{index}</span>{/if}
        <span class="go"><Icon name={playing ? "pause" : "play"} size={14} /></span>
      </button>
    {:else}
      <span class="n"><span class="num">{index}</span></span>
    {/if}
  {/if}
  <button
    class="main"
    onclick={hit.kind === "track" ? play : open}
    disabled={!openable && hit.kind !== "track"}
    title={hit.kind === "track"
      ? playing
        ? `Pause ${hit.title}`
        : isCurrent
          ? `Resume ${hit.title}`
          : `Play ${hit.title}${local ? " (from your library)" : " from Tidal"}`
      : openable
        ? `Open ${hit.title}`
        : undefined}
    aria-label={hit.kind === "track" ? `${playing ? "Pause" : "Play"} ${hit.title}` : undefined}
  >
    <span class="art">
      <Cover url={thumb(hit.cover_url)} size={48} radius={hit.kind === "artist" ? 24 : 8} icon={icons[hit.kind]} />
      {#if hit.kind === "track" && index == null}
        <span class="art-state" class:sound={playing}>
          <span class="idle"><Icon name={playing ? "volume" : "play"} size={18} /></span>
          <span class="hover"><Icon name={playing ? "pause" : "play"} size={18} /></span>
        </span>
      {/if}
    </span>
    <div class="grow">
      <div class="ellipsis title">
        <span class:now-title={isCurrent}>{hit.title}</span>
        {#if hit.explicit}<span class="badge e">E</span>{/if}
      </div>
      <div class="ellipsis sub">
        {#if hit.quality}<span class="badge">{hit.quality}</span>{/if}
        {#if hit.versions && hit.versions.length > 1}<span class="badge versions">{hit.versions.length} versions</span>{/if}
        {#if error}<span class="error">{error}</span>{:else}{hit.subtitle}{/if}
      </div>
    </div>
  </button>
  {#if hit.duration_ms}<span class="time">{formatTime(hit.duration_ms)}</span>{/if}
  {#if canFavorite}
    <button
      class="icon-btn quiet"
      class:fav
      title={fav ? "Remove from favorites" : "Add to favorites"}
      aria-label={fav ? `Remove ${hit.title} from favorites` : `Add ${hit.title} to favorites`}
      aria-pressed={fav}
      onclick={() => app.toggleFavorite(hit.kind, hit.id, hit.title)}
    >
      <Icon name={fav ? "heartFill" : "heart"} size={16} />
    </button>
  {/if}
  {#if hit.versions && hit.versions.length > 1}
    {@const versions = hit.versions}
    <button class="icon-btn quiet" title="{versions.length} versions: explicit, clean, Dolby Atmos, remasters…" aria-label="Versions of {hit.title}" onclick={(e) => app.openVersions(e, hit.title, versions)}>
      <Icon name="layers" size={16} />
    </button>
  {/if}
  {#if hit.kind === "track"}
    <button class="icon-btn quiet" title="Play next / add to queue" aria-label="Queue {hit.title}" onclick={queueMenu}>
      <Icon name="queue" size={16} />
    </button>
  {/if}
  {#if app.account && hit.kind === "track"}
    <button class="icon-btn quiet" title="Add to playlist" aria-label="Add {hit.title} to a playlist" onclick={(e) => app.pickPlaylist(e, [hit.id])}>
      <Icon name="plus" size={16} />
    </button>
  {/if}
  {#if onremove}
    <button class="icon-btn quiet" title="Remove from this playlist" aria-label="Remove {hit.title} from this playlist" onclick={onremove}>
      <Icon name="minus" size={16} />
    </button>
  {/if}
  {#if deleting}
    <span class="state deleting-state" title="Moving to the Trash…">
      <svg viewBox="0 0 24 24" width="24" height="24" class="spin">
        <circle cx="12" cy="12" r="9" class="track" />
        <circle cx="12" cy="12" r="9" class="bar" stroke-dasharray={ring} stroke-dashoffset={ring * 0.7} />
      </svg>
    </span>
  {:else if local}
    <button
      class="icon-btn quiet trash"
      title="Delete download (moves the file to the Trash)"
      aria-label="Delete the download of {hit.title}"
      onclick={() => app.deleteDownloads([hit], `“${hit.title}”`)}
    >
      <Icon name="trash" size={15} />
    </button>
    <span class="tick" title="Downloaded"><Icon name="check" size={14} /></span>
  {:else if dl?.state === "waiting" || job === "waiting"}
    <span class="state waiting" title="Waiting for its turn"><Icon name="clock" size={16} /></span>
  {:else if dl?.state === "downloading" || job === "running"}
    <span class="state" title={dl ? `Downloading · ${Math.round(dl.progress * 100)}%` : "Downloading"}>
      <svg viewBox="0 0 24 24" width="24" height="24" class:spin={!dl || dl.progress === 0}>
        <circle cx="12" cy="12" r="9" class="track" />
        <circle cx="12" cy="12" r="9" class="bar" stroke-dasharray={ring} stroke-dashoffset={ring * (1 - (dl?.progress || 0.25))} />
      </svg>
      {#if dl && dl.progress > 0}<span class="pct">{Math.round(dl.progress * 100)}</span>{/if}
    </span>
  {:else if dl?.state === "done"}
    <span class="tick" title="Downloaded"><Icon name="check" size={14} /></span>
  {:else}
    <button
      class="icon-btn"
      class:failed={dl?.state === "failed"}
      title={dl?.state === "failed" ? `Failed: ${dl.detail ?? ""} · retry` : "Download"}
      aria-label="Download {hit.title}"
      onclick={download}
    >
      <Icon name={dl?.state === "failed" ? "alert" : "download"} />
    </button>
  {/if}
</div>

<style>
  .hit {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 8px;
    border-radius: min(12px, var(--r-max));
    min-width: 0;
  }
  .hit:hover {
    background: var(--glass-strong);
  }
  .n {
    position: relative;
    width: 26px;
    display: flex;
    justify-content: flex-end;
    font-size: 12px;
    color: var(--faint);
    font-variant-numeric: tabular-nums;
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 12px;
    text-align: left;
    opacity: 1 !important;
  }
  .main:disabled {
    cursor: default;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .title {
    font-weight: 500;
  }
  .sub {
    font-size: 12.5px;
    color: var(--muted);
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .badge.versions {
    margin-right: 6px;
    background: color-mix(in srgb, var(--room-accent) 18%, transparent);
  }
  .badge.e {
    margin-left: 4px;
    background: var(--surface-3);
    color: var(--muted);
  }
  .time {
    font: 12px var(--font-mono);
    color: var(--muted);
  }
  .tick {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: color-mix(in srgb, var(--ok) 25%, transparent);
    color: var(--ok);
  }
  .error {
    color: var(--danger);
  }
  .state {
    position: relative;
    width: 34px;
    height: 34px;
    display: grid;
    place-items: center;
    color: var(--accent-text);
  }
  .waiting {
    color: var(--muted);
    animation: breathe 1.6s ease-in-out infinite alternate;
  }
  @keyframes breathe {
    to {
      opacity: 0.4;
    }
  }
  .state svg {
    transform: rotate(-90deg);
    fill: none;
    stroke-width: 2.5;
  }
  .state svg.spin {
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(270deg);
    }
  }
  .state .track {
    stroke: var(--glass-border);
  }
  .state .bar {
    stroke: currentColor;
    stroke-linecap: round;
    transition: stroke-dashoffset 0.25s;
  }
  .pct {
    position: absolute;
    font: 700 8.5px var(--font-mono);
  }
  .failed {
    color: var(--danger);
  }
  .go {
    display: none;
    color: var(--accent-text);
  }
  /* the number is the row's play button: it reacts like one */
  .go-btn {
    padding: 0;
    display: inline-grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 999px;
    transition: background 0.15s, transform 0.15s;
  }
  .go-btn:hover {
    background: color-mix(in srgb, var(--accent-text) 18%, transparent);
    transform: scale(1.08);
  }
  .go-btn:active {
    transform: scale(0.94);
  }
  .go-btn:focus-visible .go {
    display: inline;
  }
  .go-btn:focus-visible .num {
    display: none;
  }
  .num.sound {
    display: inline-flex;
    color: var(--accent-text);
  }
  /* over the cover when the row has no number: the track's state, and play / pause on hover */
  .art {
    position: relative;
    flex: none;
    display: grid;
  }
  .art-state {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    border-radius: min(8px, var(--r-max));
    color: #fff;
    background: rgb(0 0 0 / 0.45);
    opacity: 0;
    transition: opacity 0.15s;
  }
  .art-state > span {
    grid-area: 1 / 1;
    display: grid;
  }
  .art-state .hover {
    display: none;
  }
  .art-state.sound {
    opacity: 1;
  }
  .track:hover .art-state {
    opacity: 1;
  }
  .track:hover .art-state .hover {
    display: grid;
  }
  .track:hover .art-state .idle {
    display: none;
  }
  .track:hover .go {
    display: inline;
  }
  .track:hover .num {
    display: none;
  }
  /* secondary row actions show on hover (favorites stay visible) */
  .quiet {
    opacity: 0;
    transition: opacity 0.15s;
  }
  .hit:hover .quiet,
  .quiet:focus-visible,
  .quiet.fav {
    opacity: 1;
  }
  .fav {
    color: #f472b6;
  }
  .trash:hover {
    color: var(--danger);
  }
  .deleting {
    background: linear-gradient(90deg, transparent, color-mix(in srgb, var(--danger) 14%, transparent), transparent) 0 0 / 200% 100%;
    animation: sweep 1.1s linear infinite;
  }
  .deleting .main {
    opacity: 0.55 !important;
  }
  @keyframes sweep {
    to {
      background-position: -200% 0;
    }
  }
  .deleting-state {
    color: var(--danger);
  }
</style>
