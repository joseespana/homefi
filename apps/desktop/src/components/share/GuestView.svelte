<script lang="ts">
  // Someone else's room, as a guest: the player in the chosen layout, the room
  // (what plays, skip / vote, volume, playing along here, the queue) and search.
  import { formatTime, type Collection, type ShareTrack } from "../../lib/api";
  import { share, type GuestTab } from "../../lib/share.svelte";
  import { provideSource, roomSource } from "../../lib/source.svelte";
  import { app } from "../../lib/state.svelte";
  import Cover from "../Cover.svelte";
  import Icon from "../Icon.svelte";
  import Room from "../Room.svelte";
  import RoomQueue from "./RoomQueue.svelte";

  // the room and its layouts draw the hub's song here
  provideSource(roomSource);

  const g = $derived(share.guest);
  const st = $derived(g.state);
  const now = $derived(st?.now_playing ?? null);
  const s = $derived(app.settings?.share);
  const tabs: { id: GuestTab; label: string; icon: string }[] = [
    { id: "player", label: "Player", icon: "headphones" },
    { id: "room", label: "Room", icon: "users" },
    { id: "search", label: "Search", icon: "search" },
  ];
  let query = $state("");
  const precision = $derived(g.round_trip_ms != null ? Math.max(1, Math.round(g.round_trip_ms / 2)) : null);
  const results = $derived(share.results);
  const page = $derived(share.page);
  const groups = $derived<{ label: string; list: Collection[] }[]>(
    results ? [{ label: "Albums", list: results.albums }, { label: "Artists", list: results.artists }, { label: "Playlists", list: results.playlists }] : [],
  );

  function saveShare(patch: Partial<NonNullable<typeof s>>) {
    if (s) app.saveSettings({ share: { ...s, ...patch } });
  }

  function search(e: SubmitEvent) {
    e.preventDefault();
    share.search(query);
  }
</script>

<div class="guest">
  <header>
    <span class="mark"><Icon name="cast" size={18} /></span>
    <div class="grow">
      <h1 class="ellipsis">{g.session_name}</h1>
      <p class="muted small">
        {g.playing_here ? "Playing on this computer" : "Remote control"}
        {#if st}· {st.guests.length + 1} in the room{/if}
      </p>
    </div>
    <nav class="tabs" aria-label="Room">
      {#each tabs as t}
        <button class:selected={share.tab === t.id} onclick={() => (share.tab = t.id)}><Icon name={t.icon} size={15} /> {t.label}</button>
      {/each}
    </nav>
    <button class="btn tonal leave" onclick={() => share.leave()}>Leave</button>
  </header>

  {#if share.tab === "player"}
    <div class="player"><Room /></div>
  {:else if share.tab === "room"}
    <div class="room">
      <section class="card now">
        {#if now}
          <Cover url={now.cover_url} size={120} radius={14} icon="disc" />
          <div class="grow">
            <div class="eyebrow">{st?.playing ? "Now playing" : "Paused"}</div>
            <h2 class="ellipsis">{now.title}</h2>
            <p class="muted ellipsis">{now.artist}{now.album ? ` · ${now.album}` : ""}</p>
            <p class="by"><span class="dot" style:background={now.added_by.color}></span> {now.added_by.guest_id === g.guest_id ? "You" : now.added_by.name}</p>
            <div class="bar"><span style:width="{now.duration_ms ? (100 * share.position) / now.duration_ms : 0}%"></span></div>
            <p class="muted small time">{formatTime(share.position)} / {formatTime(now.duration_ms)}</p>
          </div>
        {:else}
          <p class="muted">Nothing playing</p>
        {/if}
        <div class="controls">
          {#if share.canToggle}
            <button class="btn tonal" onclick={() => share.toggle()}><Icon name={st?.playing ? "pause" : "play"} size={15} /> {st?.playing ? "Pause" : "Play"}</button>
          {/if}
          {#if share.votes}
            <button class="btn tonal" disabled={!now} onclick={() => share.skip()}><Icon name="vote" size={15} /> Vote to skip {st?.skip_votes ?? 0}/{st?.skip_votes_needed ?? 1}</button>
          {:else}
            <button class="btn tonal" disabled={!now || !share.canSkip} title={share.canSkip ? "" : "Only the host can skip"} onclick={() => share.skip()}><Icon name="next" size={15} /> Skip</button>
          {/if}
          {#if share.canToggle && st?.volume != null}
            <label class="volume" title="The hub’s volume">
              <Icon name="volume" size={15} />
              <input type="range" min="0" max="1" step="0.01" value={st.volume} onchange={(e) => share.volume(Number(e.currentTarget.value))} aria-label="Volume" />
            </label>
          {/if}
        </div>
      </section>

      <section class="card">
        <label class="toggle">
          <input type="checkbox" checked={g.play_along} onchange={(e) => share.playAlong(e.currentTarget.checked)} />
          <span><b>Play on this computer</b><small class="muted">{st?.audio ? "On your own speakers, in sync with the hub" : "The hub is not streaming its music right now"}</small></span>
        </label>
        {#if s}
          <label class="delay">
            <span>Speaker delay: {s.speaker_delay_ms} ms</span>
            <input type="range" min="-500" max="1000" step="10" value={s.speaker_delay_ms} onchange={(e) => saveShare({ speaker_delay_ms: Number(e.currentTarget.value) })} />
            <small class="muted">Raise it if this speaker sounds late (Bluetooth), lower it if it sounds early</small>
          </label>
        {/if}
        {#if g.playing_here && precision != null}<p class="muted small">Sync within about {precision} ms · this computer only jumps to stay in sync (no speed changes yet)</p>{/if}
      </section>

      <section>
        <h2>Up next ({st?.queue.length ?? 0})</h2>
        <RoomQueue />
      </section>
    </div>
  {:else}
    <div class="search">
      <form class="box" onsubmit={search}>
        <Icon name="search" size={16} />
        <input type="search" bind:value={query} placeholder="Search the hub’s music" />
        <button class="btn primary" type="submit" disabled={!query.trim()}>Search</button>
      </form>
      {#if share.searching}<p class="muted">Searching…</p>{/if}
      {#if page}
        <button class="btn tonal back" onclick={() => share.closePage()}><Icon name="back" size={15} /> Back</button>
        <div class="page-head">
          <Cover url={page.header.cover_url} size={96} radius={12} icon="disc" />
          <div>
            <h2>{page.header.title}</h2>
            <p class="muted">{page.header.subtitle}</p>
          </div>
        </div>
        {@render songs(page.tracks)}
      {:else if results}
        {#if !results.tracks.length && !results.albums.length && !results.artists.length && !results.playlists.length}
          <p class="muted">Nothing found</p>
        {/if}
        {#if results.tracks.length}
          <h2>Songs</h2>
          {@render songs(results.tracks)}
        {/if}
        {#each groups as group (group.label)}
          {#if group.list.length}
            <h2>{group.label}</h2>
            <div class="grid">
              {#each group.list as c (c.kind + c.id)}
                <button class="tile" onclick={() => share.browse(c)}>
                  <Cover url={c.cover_url} size={56} radius={c.kind === "artist" ? 999 : 8} icon={c.kind === "artist" ? "user" : "disc"} />
                  <span class="grow"><b class="ellipsis">{c.title}</b><small class="muted ellipsis">{c.subtitle}</small></span>
                </button>
              {/each}
            </div>
          {/if}
        {/each}
      {/if}
    </div>
  {/if}
</div>

{#snippet songs(list: ShareTrack[])}
  <div class="songs">
    {#each list as t, i (t.source + i)}
      <div class="song">
        <Cover url={t.cover_url} size={40} radius={6} />
        <div class="grow">
          <div class="ellipsis">{t.title}{#if t.explicit}<span class="e">E</span>{/if}</div>
          <div class="muted small ellipsis">{t.artist}{t.album ? ` · ${t.album}` : ""}</div>
        </div>
        {#if t.duration_ms}<span class="muted small">{formatTime(t.duration_ms)}</span>{/if}
        <button class="icon-btn" title="Play it before your other songs" aria-label="Add next: {t.title}" onclick={() => share.add(t, true)}><Icon name="next" size={15} /></button>
        <button class="btn tonal add" onclick={() => share.add(t)}><Icon name="plus" size={14} /> Add</button>
      </div>
    {/each}
  </div>
{/snippet}

<style>
  .guest {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-height: 100%;
  }
  header {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  header h1 {
    font-size: 24px;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .small {
    font-size: 12.5px;
  }
  p {
    margin: 0;
  }
  .mark {
    display: grid;
    place-items: center;
    flex: none;
    width: 38px;
    height: 38px;
    border-radius: min(12px, var(--r-max));
    background: var(--gradient);
    color: #fff;
  }
  .tabs {
    display: flex;
    gap: 4px;
    padding: 4px;
    border-radius: min(999px, var(--r-max));
    background: var(--glass);
    border: 1px solid var(--glass-border);
  }
  .tabs button {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 14px;
    border-radius: min(999px, var(--r-max));
    font-weight: 600;
    color: var(--muted);
  }
  .tabs button.selected {
    background: var(--primary-strong);
    color: var(--on-primary-strong);
  }
  .player {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .room {
    display: grid;
    grid-template-columns: minmax(0, 1.1fr) minmax(0, 0.9fr);
    gap: 16px;
    align-items: start;
  }
  .room > section:last-child {
    grid-column: 1 / -1;
  }
  .room h2 {
    margin: 4px 0 10px;
  }
  .card {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
    padding: 16px;
    border-radius: min(18px, var(--r-max));
    background: var(--glass);
    border: 1px solid var(--glass-border);
    backdrop-filter: blur(20px) saturate(1.3);
  }
  .card.now h2 {
    margin: 2px 0;
    font-size: 22px;
  }
  .eyebrow {
    font: 700 11px var(--font-mono);
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--accent-text);
  }
  .by {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 6px;
    font-size: 13px;
  }
  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
  }
  .bar {
    height: 4px;
    margin-top: 12px;
    border-radius: min(2px, var(--r-max));
    background: var(--glass-strong);
    overflow: hidden;
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--accent-text);
  }
  .time {
    margin-top: 4px;
    font-variant-numeric: tabular-nums;
  }
  .controls {
    flex-basis: 100%;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .volume {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--muted);
  }
  .toggle {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    flex-basis: 100%;
  }
  .toggle input {
    width: 16px;
    height: 16px;
    margin-top: 3px;
    accent-color: var(--primary-strong);
  }
  .toggle span,
  .delay {
    display: flex;
    flex-direction: column;
  }
  .toggle small,
  .delay small {
    font-size: 12px;
  }
  .delay {
    flex-basis: 100%;
    gap: 4px;
    font-size: 13px;
  }
  .delay input {
    width: 100%;
    accent-color: var(--primary-strong);
  }
  .search {
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-width: 920px;
  }
  .box {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 6px 6px 14px;
    border-radius: min(999px, var(--r-max));
    background: var(--surface-2);
    border: 1px solid var(--border);
    color: var(--muted);
  }
  .box input {
    flex: 1;
    border: 0;
    background: transparent;
    padding: 6px 0;
  }
  .search h2 {
    margin: 14px 0 4px;
  }
  .back {
    align-self: flex-start;
  }
  .page-head {
    display: flex;
    align-items: center;
    gap: 16px;
  }
  .page-head h2 {
    margin: 0;
  }
  .songs {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .song {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 8px;
    border-radius: min(10px, var(--r-max));
  }
  .song:hover {
    background: var(--surface-2);
  }
  .e {
    margin-left: 6px;
    padding: 0 4px;
    border-radius: min(3px, var(--r-max));
    font-size: 10px;
    background: var(--surface-3);
    color: var(--muted);
  }
  .add {
    height: 30px;
    padding: 0 12px;
    font-size: 13px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 8px;
  }
  .tile {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px;
    border-radius: min(12px, var(--r-max));
    text-align: left;
    background: var(--glass);
    border: 1px solid var(--glass-border);
  }
  .tile span {
    display: flex;
    flex-direction: column;
  }
  .tile small {
    font-size: 12px;
  }
  @container (max-width: 760px) {
    .room {
      grid-template-columns: 1fr;
    }
  }
</style>
