<script lang="ts">
  import { foldLabels } from "../../lib/foldLabels";
  // A playlist HomeFi made: the draft just built, or a saved one. The songs with how often
  // the artist plays them, the ones that weren't found, the shows they came from, and the
  // actions (play, queue with Sounds, sources, save / search again / delete, save to TIDAL).
  import { DEFAULT_PLAYLIST_SETTINGS, formatTime, type HomePlaylist } from "../../lib/api";
  import { conversations } from "../../lib/party.svelte";
  import { isChat, isConcert, playable, playlists, requestOf } from "../../lib/playlists.svelte";
  import { app } from "../../lib/state.svelte";
  import Cover from "../Cover.svelte";
  import Icon from "../Icon.svelte";
  import SearchSuggestions from "./SearchSuggestions.svelte";
  import SourcesSheet from "./SourcesSheet.svelte";
  import TagChip from "./TagChip.svelte";

  let {
    playlist,
    fallbackNote = null,
    saved,
    onsave,
    ondiscard,
    rebuildable = true,
    busy = false,
  }: {
    playlist: HomePlaylist;
    fallbackNote?: string | null;
    /** Saved in HomeFi (otherwise it is a draft with Save / Discard). */
    saved: boolean;
    onsave?: () => void;
    ondiscard?: () => void;
    /** Saved playlists show "Search again" / "Change answers" / "Keep chatting" (off on the screen that is already doing it). */
    rebuildable?: boolean;
    /** Another build is running (the party screen's own); the concert store's is always counted. */
    busy?: boolean;
  } = $props();
  const blocked = $derived(busy || playlists.building);

  const MS_PER_MIN = 60_000;
  // concert fields exist only on concert playlists; a party playlist has no shows or setlist
  const concert = $derived(isConcert(playlist) ? playlist : null);
  const origin = $derived(concert?.origin ?? null);
  const popular = $derived(origin?.source.kind === "tidal_popular");
  const minutes = $derived(Math.max(1, Math.round(playlist.entries.reduce((sum, e) => sum + e.duration_ms, 0) / MS_PER_MIN)));
  const canPlay = $derived(playable(playlist).items.length > 0);
  const firstCover = $derived(playlist.entries.find((e) => e.cover_url)?.cover_url ?? null);
  const hasSources = $derived(playlist.sources.length > 0 || !!playlist.search_widget_html);
  const providerName = $derived(
    origin?.source.kind === "ai" ? (playlists.sources?.ai?.provider_id === origin.source.provider_id ? playlists.sources.ai.provider_name : origin.source.provider_id) : "",
  );
  const showMode = $derived(origin?.mode.kind === "show" ? origin.mode.show_id : null);
  let sourcesOpen = $state(false);

  // TIDAL: the copy in the account (if any), what the settings do with it, and the last save of this playlist
  const tidalUuid = $derived(playlist.tidal_uuid ?? null);
  const tidalReady = $derived(!!playlists.sources?.tidal);
  const tidalSettings = $derived({ ...DEFAULT_PLAYLIST_SETTINGS, ...app.settings?.playlists });
  const tidalStatus = $derived(playlists.tidal?.id === playlist.id ? playlists.tidal : null);
  const tidalSaving = $derived(tidalStatus?.state === "saving");
  const savedNote = $derived(
    tidalUuid ? "Saved in HomeFi and in your TIDAL account." : tidalReady ? "Saved in HomeFi only. “Save to TIDAL” adds it to your TIDAL account." : "Saved in HomeFi only.",
  );
  const draftNote = $derived(
    tidalSettings.tidal_auto_save && tidalReady ? "Not saved yet. Saving keeps it in HomeFi and adds it to your TIDAL account." : "Not saved yet. Saving keeps it in HomeFi only; nothing is added to your TIDAL account.",
  );

  const dateLabel = (iso: string) => new Date(`${iso}T12:00:00`).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });

  async function remove() {
    const withTidal = tidalSettings.tidal_delete_with && !!tidalUuid;
    const ok = await app.confirm({
      title: `Delete “${playlist.name}”?`,
      text: withTidal
        ? "It's removed from HomeFi and its copy in your TIDAL account is deleted too. Your music isn't touched."
        : "It's removed from HomeFi only; your music and your TIDAL account aren't touched.",
      confirmLabel: "Delete playlist",
      danger: true,
    });
    if (!ok) return;
    await playlists.remove(playlist.id);
    app.view = { name: "playlists" };
  }
</script>

<section class="preview">
  <header class="head">
    <Cover url={firstCover} size={150} radius={14} icon="queue" />
    <div class="info">
      <h1>{popular && origin ? `Most played by ${origin.artist} on Tidal` : playlist.name}</h1>
      {#if popular}<span class="not-real"><Icon name="alert" size={15} /> Not the real setlist</span>{/if}
      <div class="muted">{playlist.description}</div>
      <div class="muted">{playlist.entries.length} songs · {minutes} min</div>
      <div class="actions fold" use:foldLabels>
        <button class="btn primary" disabled={!canPlay || blocked} onclick={() => playlists.play(playlist)}><Icon name="play" size={16} /> Play</button>
        <button class="btn tonal" disabled={!canPlay || blocked} onclick={() => playlists.play(playlist, 0, true)}><Icon name="shuffle" size={16} /> Shuffle</button>
        <button class="btn tonal" disabled={!canPlay} onclick={(e) => app.openQueueMenu(e, playlist.name, (next) => playlists.queue(playlist, next), { kind: "playlist", id: playlist.id, name: playlist.name })}>
          <Icon name="queue" size={16} /> Queue
        </button>
        {#if hasSources}
          <button class="btn tonal" onclick={() => (sourcesOpen = true)}><Icon name="external" size={16} /> Sources</button>
        {/if}
      </div>
      <div class="actions fold" use:foldLabels>
        {#if saved}
          {#if concert}
            <button class="btn tonal" disabled={playlists.building} onclick={() => playlists.build(requestOf(concert), concert.id)}><Icon name="refresh" size={16} /> Search again</button>
          {:else if rebuildable && isChat(playlist)}
            <button class="btn tonal" onclick={() => conversations.begin("chat", { replaceId: playlist.id })}><Icon name="edit" size={16} /> Keep chatting</button>
          {:else if rebuildable}
            <button class="btn tonal" onclick={() => conversations.begin("party", { replaceId: playlist.id })}><Icon name="edit" size={16} /> Change answers</button>
          {/if}
          {#if tidalReady}
            <button class="btn tonal" disabled={tidalSaving} aria-busy={tidalSaving} onclick={() => playlists.saveToTidal(playlist)}>
              <Icon name={tidalUuid ? "refresh" : "save"} size={16} />
              {tidalSaving ? "Saving to TIDAL…" : tidalUuid ? "Update on TIDAL" : "Save to TIDAL"}
            </button>
            {#if tidalUuid}
              <button class="btn tonal" onclick={() => (app.view = { name: "tidal-item", kind: "playlist", id: tidalUuid, back: app.view })}><Icon name="external" size={16} /> Open in TIDAL</button>
            {/if}
          {/if}
          <button class="btn tonal" onclick={remove}><Icon name="trash" size={16} /> Delete playlist</button>
        {:else}
          <button class="btn primary" onclick={onsave}><Icon name="check" size={16} /> Save playlist</button>
          <button class="btn tonal" onclick={ondiscard}>Discard</button>
        {/if}
      </div>
      <div class="muted small">{saved ? savedNote : draftNote}</div>
      {#if tidalStatus?.state === "saved"}
        <div class="small tidal-done" role="status"><Icon name="check" size={14} /> {tidalStatus.summary}</div>
        {#if tidalStatus.skipped.length}
          <details class="small muted">
            <summary>Not on TIDAL ({tidalStatus.skipped.length})</summary>
            <ul>
              {#each tidalStatus.skipped as name, i (i)}<li>{name}</li>{/each}
            </ul>
          </details>
        {/if}
      {:else if tidalStatus?.state === "failed"}
        <div class="small error" role="alert">Couldn't save to TIDAL: {tidalStatus.message}</div>
      {/if}
    </div>
  </header>

  {#if fallbackNote}
    <p class="notice" role="note"><Icon name="info" size={16} /> <span>{fallbackNote}</span></p>
  {/if}

  {#if playlist.search_widget_html}
    <SearchSuggestions html={playlist.search_widget_html} />
  {/if}

  {#if concert && concert.origin.shows.length}
    <details class="shows">
      <summary>Shows ({concert.origin.shows.length})</summary>
      {#if showMode}
        <button class="btn tonal whole" disabled={playlists.building} onclick={() => playlists.rearrange(playlist, requestOf(concert, { kind: "tour" }))}>Whole tour</button>
      {/if}
      <ul>
        {#each concert.origin.shows as s (s.id)}
          <li class:current={showMode === s.id}>
            <span class="when">{dateLabel(s.date)}</span>
            <span class="where ellipsis">{s.venue}, {s.city}</span>
            <button class="btn tonal small-btn" disabled={playlists.building || showMode === s.id} onclick={() => playlists.rearrange(playlist, requestOf(concert, { kind: "show", show_id: s.id }))}>Only this show</button>
          </li>
        {/each}
      </ul>
    </details>
  {/if}

  {#if playlists.error}<p class="error" role="alert">{playlists.error}</p>{/if}

  <ol class="songs">
    {#each playlist.entries as e, i (i)}
      {@const playableRow = !!e.tidal_id || !!e.local_path}
      <li>
        <button class="row" disabled={!playableRow || blocked} onclick={() => playlists.play(playlist, i)}>
          <span class="n muted">{i + 1}</span>
          <Cover url={e.cover_url} path={e.local_path} size={40} radius={6} />
          <span class="text">
            <span class="title ellipsis">{e.title}</span>
            <span class="sub ellipsis muted">{e.artist}{e.note ? ` · ${e.note}` : ""}</span>
          </span>
          <TagChip tag={e.tag} />
          {#if e.local_path}<span class="local" title="A file in your library will play"><Icon name="folder" size={13} /> On this computer</span>{/if}
          <span class="time muted">{e.duration_ms ? formatTime(e.duration_ms) : ""}</span>
        </button>
      </li>
    {/each}
  </ol>

  {#if playlist.unmatched.length}
    <h2>Not found on Tidal or in your library ({playlist.unmatched.length})</h2>
    <ul class="songs missing">
      {#each playlist.unmatched as u, i (i)}
        <li class="row dim">
          <span class="text">
            <span class="title ellipsis">{u.title}</span>
            <span class="sub ellipsis muted">{u.artist}{u.note ? ` · ${u.note}` : ""}</span>
          </span>
          <TagChip tag="missing" />
        </li>
      {/each}
    </ul>
  {/if}
</section>

{#if sourcesOpen}
  <SourcesSheet {playlist} {providerName} onclose={() => (sourcesOpen = false)} />
{/if}

<style>
  .preview {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .head {
    display: flex;
    gap: 24px;
    align-items: flex-end;
    flex-wrap: wrap;
  }
  .info {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
    flex: 1;
  }
  .actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    margin-top: 4px;
  }
  .small {
    font-size: 12.5px;
  }
  .tidal-done {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--accent-text);
  }
  details.small ul {
    margin: 4px 0 0;
    padding-left: 18px;
  }
  .not-real {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 12px;
    border-radius: var(--r-pill);
    background: var(--warn-bg);
    border: 1.5px solid var(--warn);
    color: var(--warn);
    font-weight: 800;
    font-size: 13px;
    letter-spacing: 0.03em;
    text-transform: uppercase;
  }
  .notice {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    margin: 0;
    padding: 12px 14px;
    border: 1px solid var(--warn-border);
    border-radius: var(--r-sm);
    background: var(--warn-bg);
    line-height: 1.45;
  }
  .notice :global(svg) {
    flex: none;
    margin-top: 2px;
  }
  .error {
    color: var(--danger);
    margin: 0;
  }
  .shows {
    padding: 8px 14px;
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    background: var(--surface-2);
  }
  .shows summary {
    cursor: pointer;
    font-weight: 600;
    padding: 4px 0;
  }
  .shows ul {
    list-style: none;
    margin: 6px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .shows li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 4px 0;
  }
  .shows li.current {
    color: var(--accent-text);
  }
  .when {
    flex: none;
    width: 110px;
    color: var(--muted);
  }
  .where {
    flex: 1;
    min-width: 0;
  }
  .small-btn {
    height: 28px;
    padding: 0 12px;
    font-size: 12.5px;
  }
  .whole {
    margin-top: 6px;
  }
  .songs {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 6px 10px;
    border-radius: var(--r-sm);
    text-align: left;
  }
  button.row:hover:not(:disabled) {
    background: var(--glass-strong);
  }
  button.row:disabled {
    cursor: default;
  }
  .n {
    width: 24px;
    text-align: right;
    flex: none;
    font-variant-numeric: tabular-nums;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .title {
    font-weight: 600;
  }
  .sub {
    font-size: 12.5px;
  }
  .local {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    flex: none;
    font-size: 12px;
    color: var(--accent-text);
  }
  .time {
    flex: none;
    width: 44px;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  h2 {
    margin: 8px 0 0;
  }
  .dim {
    opacity: 0.65;
  }
</style>
