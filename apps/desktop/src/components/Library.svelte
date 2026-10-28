<script lang="ts">
  import { item } from "../lib/motion";
  import { api, bumpCovers, errorText, on, type ArtworkProgress, type Track } from "../lib/api";
  import { pickFolder } from "./Folder.svelte";
  import { app, filterTracks, groupAlbums, groupArtists } from "../lib/state.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";
  import Skeleton from "./Skeleton.svelte";
  import TrackTable from "./TrackTable.svelte";

  let query = $state("");
  const tracks = $derived(filterTracks(app.tracks, query));
  const albums = $derived(groupAlbums(tracks));
  const artists = $derived(groupArtists(tracks));

  const view = $derived(app.view);
  const album = $derived(view.name === "album" ? groupAlbums(app.tracks).find((a) => a.key === view.key) : undefined);
  const artist = $derived(view.name === "artist" ? groupArtists(app.tracks).find((a) => a.name === view.name_) : undefined);

  const title = $derived(
    { songs: "Songs", albums: "Albums", artists: "Artists" }[view.name as string] ?? "",
  );

  let artwork = $state<ArtworkProgress | null>(null);
  let artworkError = $state<string | null>(null);
  let folderMenu = $state(false);

  async function fixCovers() {
    artworkError = null;
    artwork = { done: 0, total: 0, fixed: 0, current: "Looking for songs without a cover…" };
    const stop = await on.artwork((p) => (artwork = p));
    try {
      artwork = await api.fixArtwork();
      bumpCovers();
      app.tracks = [...app.tracks];
    } catch (e) {
      artworkError = errorText(e);
      artwork = null;
    } finally {
      stop();
    }
  }

  function queueAll(e: MouseEvent, label: string, list: Track[]) {
    app.openQueueMenu(e, label, (next) => app.queueFiles(list.map((t) => t.path), label, next));
  }

  function playAll(list = tracks, shuffle = false) {
    if (list.length) api.play(list.map((t) => t.path), 0, shuffle);
  }
</script>

{#if album}
  <header class="detail">
    <Cover path={album.tracks[0].path} size={180} radius={14} icon="disc" />
    <div class="detail-info">
      <button class="back" onclick={() => (app.view = { name: "albums" })}><Icon name="back" size={16} /> Albums</button>
      <h1>{album.title || "Unknown album"}</h1>
      <div class="muted">{album.artist || "Unknown artist"}{album.year ? ` · ${album.year}` : ""} · {album.tracks.length} songs</div>
      <div><span class="badge">{album.tracks[0].quality}</span></div>
      <div class="actions">
        <button class="btn primary" onclick={() => playAll(album.tracks)}><Icon name="play" size={16} /> Play</button>
        <button class="btn tonal" onclick={() => playAll(album.tracks, true)}><Icon name="shuffle" size={16} /> Shuffle</button>
        <button class="btn tonal" onclick={(e) => queueAll(e, album.title || "Unknown album", album.tracks)}><Icon name="queue" size={16} /> Queue</button>
      </div>
    </div>
  </header>
  <TrackTable tracks={album.tracks} numbered showAlbum={false} />
{:else if artist}
  <header class="head">
    <div>
      <button class="back" onclick={() => (app.view = { name: "artists" })}><Icon name="back" size={16} /> Artists</button>
      <h1>{artist.name || "Unknown artist"}</h1>
      <div class="muted">{artist.albums} albums · {artist.tracks.length} songs</div>
    </div>
    <div class="actions">
      <button class="btn primary" onclick={() => playAll(artist.tracks)}><Icon name="play" size={16} /> Play</button>
      <button class="btn tonal" onclick={() => playAll(artist.tracks, true)}><Icon name="shuffle" size={16} /> Shuffle</button>
      <button class="btn tonal" onclick={(e) => queueAll(e, artist.name || "Unknown artist", artist.tracks)}><Icon name="queue" size={16} /> Queue</button>
    </div>
  </header>
  <TrackTable tracks={artist.tracks} />
{:else}
  <header class="head">
    <div>
      <h1>{title}</h1>
      <div class="muted">
        {tracks.length} songs{app.scanning ? " · scanning…" : ""}
      </div>
    </div>
    <div class="actions">
      <div class="filter">
        <Icon name="search" size={16} />
        <input placeholder="Filter your music" bind:value={query} />
      </div>
      <div class="menu-wrap">
        <button class="icon-btn" title="Play a folder" aria-label="Play a folder" onclick={() => (folderMenu = !folderMenu)}><Icon name="folder" /></button>
        {#if folderMenu}
          <button class="scrim" aria-label="Close" onclick={() => (folderMenu = false)}></button>
          <div class="menu">
            <button onclick={() => ((folderMenu = false), pickFolder())}><Icon name="folder" size={16} /> Open a folder…</button>
            {#each [...(app.settings?.recent_folders ?? []), ...(app.settings?.library_dirs ?? [])].filter((d, i, a) => a.indexOf(d) === i) as dir}
              <button onclick={() => ((folderMenu = false), (app.view = { name: "folder", path: dir }))}>
                <span class="ellipsis">{dir.split(/[\\/]/).pop()}</span><span class="dim ellipsis">{dir}</span>
              </button>
            {/each}
          </div>
        {/if}
      </div>
      <button class="icon-btn" title="Fetch missing covers from Tidal" aria-label="Fetch missing covers" onclick={fixCovers} disabled={!app.account || (artwork !== null && artwork.current !== "")}>
        <Icon name="disc" />
      </button>
      <button class="icon-btn" title="Rescan folders" onclick={() => app.rescan()} disabled={app.scanning}>
        <Icon name="refresh" />
      </button>
      {#if view.name === "songs"}
        <button class="btn tonal" onclick={() => playAll(tracks, true)} disabled={!tracks.length}><Icon name="shuffle" size={16} /> Shuffle</button>
        <button class="btn primary" onclick={() => playAll(tracks)} disabled={!tracks.length}><Icon name="play" size={16} /> Play</button>
      {/if}
    </div>
  </header>

  {#if artwork}
    <div class="artwork">
      {#if artwork.current}
        <span class="spin"></span> {artwork.current} · {artwork.done}/{artwork.total || "…"}
      {:else}
        <Icon name="check" size={16} /> Covers: {artwork.fixed} added · {artwork.total - artwork.fixed} not found on Tidal
        <button class="icon-btn" aria-label="Dismiss" onclick={() => (artwork = null)}><Icon name="x" size={14} /></button>
      {/if}
    </div>
  {/if}
  {#if artworkError}<p class="error">{artworkError}</p>{/if}

  {#if !app.tracks.length && app.scanning}
    <Skeleton kind={view.name === "songs" ? "rows" : "grid"} round={view.name === "artists"} count={view.name === "songs" ? 10 : 12} />
  {:else if !app.tracks.length}
    <div class="empty">
      <img src="/logo.svg" alt="" width="96" height="96" />
      <h2>No music yet</h2>
      <p class="muted">Download something from Tidal, or add the folders where you keep your music in Settings.</p>
      <div class="actions">
        <button class="btn primary" onclick={() => (app.view = { name: "search" })}><Icon name="search" size={16} /> Search Tidal</button>
        <button class="btn tonal" onclick={() => (app.view = { name: "settings" })}><Icon name="folder" size={16} /> Music folders</button>
      </div>
    </div>
  {:else if view.name === "songs"}
    <TrackTable {tracks} />
  {:else if view.name === "albums"}
    <div class="grid">
      {#each albums as a (a.key)}
        <button class="card" in:item|global onclick={() => (app.view = { name: "album", key: a.key })} ondblclick={() => playAll(a.tracks)}>
          <div class="sleeve">
            <span class="vinyl" aria-hidden="true"><i></i></span>
            <Cover path={a.tracks[0].path} size="100%" radius={10} icon="disc" />
          </div>
          <div class="card-title ellipsis">{a.title || "Unknown album"}</div>
          <div class="card-sub ellipsis">{a.artist || "Unknown artist"}{a.year ? ` · ${a.year}` : ""}</div>
        </button>
      {/each}
    </div>
  {:else}
    <div class="artists">
      {#each artists as a (a.name)}
        <button class="artist" in:item|global onclick={() => (app.view = { name: "artist", name_: a.name })}>
          <Cover path={a.tracks[0].path} size={52} radius={26} icon="user" />
          <div>
            <div class="ellipsis">{a.name || "Unknown artist"}</div>
            <div class="card-sub">{a.albums} albums · {a.tracks.length} songs</div>
          </div>
        </button>
      {/each}
    </div>
  {/if}
{/if}

<style>
  .head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 18px;
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
  .detail {
    display: flex;
    gap: 24px;
    align-items: flex-end;
    margin-bottom: 24px;
  }
  .detail-info {
    display: flex;
    flex-direction: column;
    gap: 8px;
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
  }
  .card:hover {
    background: var(--glass-strong);
  }
  .sleeve {
    position: relative;
    perspective: 800px;
  }
  .sleeve :global(.cover) {
    position: relative;
    z-index: 1;
    box-shadow: 0 14px 30px rgb(0 0 0 / 0.4);
    transition: transform 0.35s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .card:hover .sleeve :global(.cover) {
    transform: rotateY(-14deg) translateX(-6%);
  }
  /* the record slides out of its sleeve on hover */
  .vinyl {
    position: absolute;
    top: 4%;
    right: 4%;
    width: 92%;
    aspect-ratio: 1;
    border-radius: 50%;
    background: repeating-radial-gradient(circle, #111 0 2px, #1c1c1c 2px 4px);
    box-shadow: 0 8px 20px rgb(0 0 0 / 0.5);
    transition: transform 0.45s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .vinyl i {
    position: absolute;
    inset: 34%;
    border-radius: 50%;
    background: var(--room-accent);
    box-shadow: inset 0 0 0 6px rgb(0 0 0 / 0.25);
  }
  .vinyl i::after {
    content: "";
    position: absolute;
    inset: 44%;
    border-radius: 50%;
    background: #111;
  }
  .card:hover .vinyl {
    transform: translateX(34%) rotate(40deg);
  }
  .card-title {
    margin-top: 6px;
    font-weight: 600;
  }
  .card-sub {
    font-size: 12.5px;
    color: var(--muted);
  }
  .artists {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 6px;
  }
  .artist {
    display: flex;
    gap: 12px;
    align-items: center;
    text-align: left;
    padding: 8px;
    border-radius: min(10px, var(--r-max));
    min-width: 0;
  }
  .artist > div {
    min-width: 0;
  }
  .artist:hover {
    background: var(--surface-2);
  }
  .menu-wrap {
    position: relative;
  }
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 10;
    cursor: default;
  }
  .menu {
    position: absolute;
    right: 0;
    top: calc(100% + 8px);
    z-index: 11;
    width: 320px;
    padding: 6px;
    border-radius: min(14px, var(--r-max));
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
  }
  .menu button {
    width: 100%;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
    padding: 8px 10px;
    border-radius: min(10px, var(--r-max));
    text-align: left;
  }
  .menu button:first-child {
    flex-direction: row;
    align-items: center;
    gap: 8px;
    font-weight: 600;
  }
  .menu button:hover {
    background: var(--surface-2);
  }
  .menu .dim {
    font-size: 11.5px;
    color: var(--muted);
    max-width: 100%;
  }
  .artwork {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 14px;
    padding: 10px 14px;
    border-radius: min(12px, var(--r-max));
    background: var(--glass-strong);
    border: 1px solid var(--glass-border);
    font-size: 13px;
  }
  .spin {
    width: 14px;
    height: 14px;
    border: 2px solid var(--glass-border);
    border-top-color: var(--room-accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
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
    border-radius: min(22px, var(--r-max));
  }
  .empty p {
    max-width: 420px;
  }
</style>
