<script lang="ts">
  import { sound } from "../lib/sound.svelte";
  import { untrack } from "svelte";
  import { api, errorText, formatTime, type Hit, type Kind, type TidalDetail, type SourceContext, type TasteScope, type VersionHit } from "../lib/api";
  import { Paged } from "../lib/paged.svelte";
  import { Windowed } from "../lib/virtual.svelte";
  import { app } from "../lib/state.svelte";
  import AlbumCard from "./AlbumCard.svelte";
  import Cover from "./Cover.svelte";
  import HitRow from "./HitRow.svelte";
  import Icon from "./Icon.svelte";
  import PageMore from "./PageMore.svelte";
  import Skeleton from "./Skeleton.svelte";

  const v = $derived(app.view.name === "tidal-item" ? app.view : null);
  let detail = $state<TidalDetail | null>(null);
  /** The tracks: the first page right away, more while scrolling (or all at once with lazy lists off). */
  let list = $state<Paged | null>(null);
  let error = $state<string | null>(null);
  let queued = $state(false);
  /** Every release of this album (itself included), for "Other versions". */
  let versions = $state<VersionHit[]>([]);

  // the first page shows right away; with lazy lists off the rest streams in behind it
  async function fetchDetail(kind: Kind, id: string) {
    const d = await api.tidalDetail(kind, id, true);
    const pages = Paged.from((offset, rest) => api.tidalDetailPage(kind, id, offset, rest), d.tracks, d.total, d.next_offset, false, {
      pages: (offset, onPage, load) => api.tidalDetailStream(kind, id, offset, onPage, load),
      cancel: api.tidalCancel,
    });
    return { d, pages };
  }

  $effect(() => {
    if (!v) return;
    const { kind, id } = v;
    untrack(() => open(kind, id));
  });

  // only the view starts a load (not the state the load itself touches)
  const win = new Windowed(60);

  function open(kind: Kind, id: string) {
    win.reset();
    detail = null;
    list = null;
    error = null;
    queued = false;
    versions = [];
    // the other releases come from the artist's page: they show up a moment later
    if (kind === "album" && app.albumVersions.group) {
      api.tidalAlbumVersions(id).then(
        (list) => {
          if (v?.kind === kind && v?.id === id) versions = list;
        },
        () => {},
      );
    }
    fetchDetail(kind, id).then(
      ({ d, pages }) => {
        if (v?.kind !== kind || v?.id !== id) return;
        detail = d;
        list = pages;
        if (!app.lazyLists) pages.rest();
      },
      (e) => (error = errorText(e)),
    );
  }

  // leaving (another item, another view): a list still streaming stops
  $effect(() => {
    const current = list;
    return () => current?.cancel();
  });

  const tracks = $derived(list?.items ?? []);
  const done = $derived(list?.done ?? true);
  /** Every track, fetching the pages not loaded yet: what Play, Shuffle, Queue and Download work on. */
  const allTracks = () => list?.all() ?? Promise.resolve([] as Hit[]);
  const trackCount = $derived(done ? tracks.length : Math.max(detail?.total ?? 0, tracks.length));

  const isOwned = (t: Hit) => !!(t.isrc && app.localIsrc.has(t.isrc));
  const owned = $derived(tracks.filter(isOwned));
  const missing = $derived(tracks.filter((t) => !isOwned(t)));
  const complete = $derived(done && !!tracks.length && missing.length === 0);
  // more than this may be downloaded among the pages not loaded yet
  const more = $derived(done ? "" : "+");
  let wiping = $state(false);

  async function run(job: () => unknown) {
    try {
      await job();
    } catch (e) {
      app.notify(errorText(e), true);
    }
  }

  // app.deleteDownloads asks first; "Deleting…" only once the user said yes
  async function deleteDownloads() {
    if (!detail) return;
    await run(async () => {
      const have = (await allTracks()).filter(isOwned);
      const n = have.length;
      try {
        await app.deleteDownloads(have, n === 1 ? "1 song" : `${n} songs`, () => (wiping = true));
      } finally {
        wiping = false;
      }
    });
  }
  // progress of this list's download: counts the rows by state
  const dlCount = $derived.by(() => {
    const c = { waiting: 0, downloading: 0, done: 0, failed: 0 };
    for (const t of tracks) {
      const d = app.trackDownloads[t.id];
      if (d) c[d.state]++;
    }
    return c;
  });
  const dlActive = $derived(dlCount.waiting + dlCount.downloading > 0);
  const total = $derived(detail?.duration_ms ?? tracks.reduce((a, t) => a + (t.duration_ms ?? 0), 0));

  function back() {
    app.view = v?.back ?? { name: "tidal", tab: "playlists" };
  }

  async function downloadAll() {
    if (!v || !detail) return;
    const { kind, id } = v;
    const title = detail.header.title;
    await run(async () => {
      const every = await allTracks();
      const lacking = every.filter((t) => !isOwned(t));
      if (!lacking.length && kind !== "artist") return app.notify("Every track is already in your library");
      // partly downloaded: fetch only what's missing (the list link would re-check everything)
      const partial = lacking.length < every.length && kind !== "artist";
      const links = partial ? lacking.map((t) => `track/${t.id}`) : [`${kind}/${id}`];
      await app.download(title, links, (partial ? lacking : every).map((t) => t.id));
      queued = true;
    });
  }

  const fav = $derived(v ? app.isFavorite(v.kind, v.id) : false);
  /** This release's labels (Hi-Res, Remastered 2023, Deluxe +16...); explicit is the E badge. */
  const mine = $derived(versions.find((x) => x.id === v?.id));
  const labels = $derived(mine ? mine.labels.filter((_, i) => mine.label_codes[i] !== "explicit" && mine.labels[i] !== detail?.header.quality) : []);

  // after an edit: Tidal's version, with as many tracks as were loaded (the scroll stays put)
  async function reload() {
    if (!v) return;
    const { kind, id } = v;
    const had = tracks.length;
    const { d, pages } = await fetchDetail(kind, id);
    while (!pages.done && pages.items.length < had && !pages.error) await pages.more();
    if (v?.kind !== kind || v?.id !== id) return;
    detail = d;
    list = pages;
  }

  async function remove(index: number) {
    if (!v || !detail || !list) return;
    const id = v.id;
    const title = tracks[index].title;
    const ok = await app.confirm({
      title: "Remove from this playlist?",
      text: `“${title}” will be removed from “${detail.header.title}” on Tidal.`,
      confirmLabel: "Remove",
      danger: true,
    });
    // the list may have changed (or the view closed) while the dialog was open
    if (!ok || v?.id !== id || !list || list.items[index]?.title !== title) return;
    // optimistic: drop the row, then confirm with Tidal's version
    list.items = list.items.filter((_, i) => i !== index);
    try {
      await api.playlistRemove(v.id, index);
      app.tidalCache.delete("playlists");
      app.notify(`Removed “${title}”`);
    } catch (e) {
      app.notify(errorText(e), true);
    }
    reload().catch((e) => app.notify(errorText(e), true));
  }

  async function deletePlaylist() {
    if (!v || !detail) return;
    const id = v.id;
    const n = trackCount;
    const ok = await app.confirm({
      title: `Delete “${detail.header.title}”?`,
      text: `The playlist${n ? ` and its ${n === 1 ? "1 track" : `${n} tracks`}` : ""} will be deleted from your Tidal account. This can't be undone. Downloaded files stay.`,
      confirmLabel: "Delete playlist",
      danger: true,
    });
    if (!ok || v?.id !== id || !detail) return;
    try {
      await api.playlistDelete(v.id);
      app.tidalCache.delete("playlists");
      app.myPlaylists = app.myPlaylists?.filter((p) => p.id !== v.id) ?? null;
      app.notify(`Deleted “${detail.header.title}”`);
      app.view = { name: "tidal", tab: "playlists" };
    } catch (e) {
      app.notify(errorText(e), true);
    }
  }

  // downloaded tracks play from the library, the others stream from Tidal
  const source = $derived<SourceContext | null>(v && detail && v.kind !== "artist" ? { kind: v.kind, id: v.id, name: detail.header.title } : null);
  const scope = $derived<TasteScope | null>(v && detail && v.kind !== "mix" ? { kind: v.kind, id: v.id, name: detail.header.title } : null);
  const play = (shuffle = false) => run(async () => app.playTidal(await allTracks(), 0, shuffle, source));
</script>

<button class="back" onclick={back}><Icon name="back" size={16} /> Back</button>

{#if error}
  <p class="error">{error}</p>
{:else if !detail}
  <Skeleton kind="detail" round={v?.kind === "artist"} count={8} />
{:else}
  <header class="detail">
    <Cover url={detail.header.cover_url} size={200} radius={v?.kind === "artist" ? 100 : 16} icon={v?.kind === "artist" ? "user" : "disc"} />
    <div class="info">
      <div class="eyebrow">{v?.kind}</div>
      <h1>{detail.header.title}{#if detail.header.explicit && v?.kind === "album"}<span class="badge e big" title="Explicit">E</span>{/if}</h1>
      <div class="muted">
        {#if v?.kind !== "playlist"}{detail.header.subtitle} · {/if}
        {#if trackCount}{trackCount} tracks{#if total}{` · ${formatTime(total)}`}{/if}{/if}
        {#if owned.length} · <span class="have">{owned.length}{more} downloaded</span>{/if}
      </div>
      {#if detail.header.quality || labels.length || versions.length > 1}
        <div class="labels">
          {#if detail.header.quality}<span class="badge" class:atmos={detail.header.quality === "Dolby Atmos"}>{detail.header.quality}</span>{/if}
          {#each labels as label, i (i)}<span class="badge" class:atmos={label === "Dolby Atmos"}>{label}</span>{/each}
          {#if versions.length > 1}
            <button class="versions" title="Explicit, clean, Dolby Atmos, remasters… switch to another release" onclick={(e) => app.openVersions(e, detail!.header.title, versions, v?.id ?? null)}>
              <Icon name="layers" size={13} /> Other versions ({versions.length - 1})
            </button>
          {/if}
        </div>
      {/if}
      {#if mine?.atmos}<p class="muted small">Dolby Atmos needs an Atmos device, and downloads skip Atmos tracks: pick a stereo version to keep it.</p>{/if}
      {#if detail.description}<p class="desc muted">{detail.description}</p>{/if}
      <div class="actions">
        {#if tracks.length}
          <button class="btn primary" onclick={() => play()}><Icon name="play" size={16} /> Play</button>
          <button class="btn tonal" onclick={() => play(true)}><Icon name="shuffle" size={16} /> Shuffle</button>
          <button
            class="btn tonal"
            onclick={(e) => app.openQueueMenu(e, detail!.header.title, (next) => run(async () => app.queueHits(await allTracks(), detail!.header.title, next)), scope ?? undefined)}
          >
            <Icon name="queue" size={16} /> Queue
          </button>
        {/if}
        {#if scope}
          {@const s = scope}
          <button class="btn tonal" title="How this {s.kind} sounds: ask the assistant or set it by hand" onclick={() => (sound.forScope = { scope: s, label: s.name })}><Icon name="soundwave" size={16} /> Sound</button>
        {/if}
        {#if complete && !dlActive}
          <span class="btn tonal done" title="Every track is in your library"><Icon name="check" size={16} /> Downloaded</span>
        {:else}
          <button class="btn tonal" onclick={downloadAll} disabled={queued && dlActive}>
            <Icon name={queued && dlActive ? "clock" : "download"} size={16} />
            {#if queued && dlActive}Downloading…
            {:else if v?.kind === "artist"}Download discography
            {:else if owned.length}Download {done ? missing.length : ""} missing
            {:else}Download all{/if}
          </button>
        {/if}
        {#if (owned.length || wiping) && !dlActive}
          <button class="btn tonal danger" class:wiping onclick={deleteDownloads} disabled={wiping} title="Moves the downloaded files to the Trash">
            {#if wiping}
              <span class="mini-spin" aria-hidden="true"></span> Deleting…
            {:else}
              <Icon name="trash" size={16} />
              {complete ? "Delete download" : `Delete ${owned.length}${more} downloaded`}
            {/if}
          </button>
        {/if}
        {#if app.account && v && v.kind !== "mix"}
          <button class="icon-btn" class:fav title={fav ? "Remove from favorites" : "Add to favorites"} aria-pressed={fav} onclick={() => app.toggleFavorite(v.kind, v.id, detail!.header.title)}>
            <Icon name={fav ? "heartFill" : "heart"} />
          </button>
        {/if}
        {#if app.account && tracks.length}
          <button class="icon-btn" title="Add all to a playlist" onclick={(e) => app.pickPlaylist(e, allTracks().then((all) => all.map((t) => t.id)))}>
            <Icon name="plus" />
          </button>
        {/if}
        {#if detail.owned}
          <button class="btn tonal danger" onclick={deletePlaylist}>
            <Icon name="trash" size={16} /> Delete playlist
          </button>
        {/if}
      </div>
      {#if dlActive || (queued && dlCount.done)}
        {@const n = trackCount}
        <div class="dl-progress">
          <div class="bar"><span style:width="{(100 * dlCount.done) / Math.max(1, n)}%"></span></div>
          <span class="muted">
            {dlCount.done} of {n} downloaded{dlCount.downloading ? ` · ${dlCount.downloading} downloading` : ""}{dlCount.waiting ? ` · ${dlCount.waiting} waiting` : ""}{dlCount.failed ? ` · ${dlCount.failed} failed` : ""}
          </span>
        </div>
      {/if}
    </div>
  </header>

  {#if detail.albums.length}
    <h2>Albums & singles</h2>
    <div class="grid">
      {#each detail.albums as album (album.id)}
        <AlbumCard hit={album} />
      {/each}
    </div>
  {/if}

  <!-- only the rows near the screen exist (a long playlist stays light) -->
  <div class="list" {@attach win.attach(() => tracks.length)}>
    <div aria-hidden="true" style:height="{win.before}px"></div>
    {#each tracks.slice(win.start, win.end) as hit, j (hit.id + ":" + (win.start + j))}
      {@const i = win.start + j}
      <HitRow {hit} index={i + 1} list={tracks} whole={done ? null : allTracks} onremove={detail.owned ? () => remove(i) : null} {source} enter={!win.scrolled} />
    {/each}
    <div aria-hidden="true" style:height="{win.after}px"></div>
    {#if list}<PageMore {list} />{/if}
  </div>
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
  .detail {
    display: flex;
    gap: 26px;
    align-items: flex-end;
    margin-bottom: 22px;
  }
  .info {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .eyebrow {
    font: 700 11px var(--font-mono);
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--accent-text);
  }
  .desc {
    max-width: 640px;
    font-size: 13px;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .have {
    color: var(--ok);
  }
  .actions {
    display: flex;
    gap: 8px;
    margin-top: 4px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 20px 16px;
    margin-bottom: 20px;
  }
  .labels {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .badge.atmos {
    background: color-mix(in srgb, #8b5cf6 22%, transparent);
  }
  .badge.e.big {
    margin-left: 10px;
    vertical-align: middle;
    font-size: 13px;
    padding: 1px 7px;
    background: var(--surface-3);
    color: var(--muted);
  }
  .versions {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 2px 10px;
    border-radius: min(999px, var(--r-max));
    font-size: 12px;
    font-weight: 600;
    background: color-mix(in srgb, var(--room-accent) 18%, transparent);
    border: 1px solid color-mix(in srgb, var(--room-accent) 45%, transparent);
  }
  .versions:hover {
    background: color-mix(in srgb, var(--room-accent) 30%, transparent);
  }
  .small {
    font-size: 12px;
  }
  .list {
    display: flex;
    flex-direction: column;
  }
  /* long playlists: rows far off screen aren't laid out or painted */
  .list > :global(.hit) {
    content-visibility: auto;
    contain-intrinsic-size: auto 60px;
  }
  .error {
    color: var(--danger);
  }
  .dl-progress {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-width: 420px;
    font-size: 12.5px;
  }
  .dl-progress .bar {
    height: 5px;
    border-radius: min(5px, var(--r-max));
    background: var(--glass-border);
    overflow: hidden;
  }
  .dl-progress .bar span {
    display: block;
    height: 100%;
    background: var(--room-accent);
    transition: width 0.3s;
  }
  .fav {
    color: #f472b6;
  }
  .danger {
    color: var(--danger);
  }
  .done {
    color: var(--ok);
    cursor: default;
  }
  .mini-spin {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid color-mix(in srgb, currentColor 30%, transparent);
    border-top-color: currentColor;
    animation: mini-spin 0.8s linear infinite;
  }
  @keyframes mini-spin {
    to {
      transform: rotate(360deg);
    }
  }
  .wiping {
    opacity: 1 !important;
  }
</style>
