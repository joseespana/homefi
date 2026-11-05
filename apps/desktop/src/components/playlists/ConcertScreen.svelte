<script lang="ts">
  // "Get ready for a concert": type an artist and HomeFi puts together the songs they are
  // playing on this tour (from recent setlists found by the user's AI with web search), or
  // Tidal's most played when that isn't possible. All the rules live in homefi-core.
  import { onMount } from "svelte";
  import { CONCERT_SHOWS_RANGE, DEFAULT_PLAYLIST_SETTINGS } from "../../lib/api";
  import { playlists } from "../../lib/playlists.svelte";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";
  import BuildStatus from "./BuildStatus.svelte";
  import PlaylistPreview from "./PlaylistPreview.svelte";

  const settings = $derived(app.settings?.playlists ?? DEFAULT_PLAYLIST_SETTINGS);
  const sources = $derived(playlists.sources);

  let artist = $state("");
  // the stepper starts at the user's setting; a change here is for this search only
  let shows = $state<number | null>(null);
  const showsValue = $derived(shows ?? settings.concert_shows);
  const ready = $derived(artist.trim().length > 0 && !playlists.building);

  function build(popularOnly: boolean) {
    if (!ready) return;
    playlists.build(playlists.request(settings, { artist: artist.trim(), shows: showsValue, popular_only: popularOnly }));
  }

  async function save() {
    const draft = playlists.draft;
    if (!draft) return;
    await playlists.save(draft.playlist);
    playlists.discard();
    app.view = { name: "home-playlist", id: draft.playlist.id };
  }

  onMount(() => {
    playlists.loadSources();
    // dev (mock): ?concert... builds Korn's playlist by itself, for screenshots
    if (import.meta.env.DEV && ["concert", "concertbuilding", "concertpopular", "concertgemini"].some((f) => new URLSearchParams(location.search).has(f)) && !playlists.draft && !playlists.building) {
      artist = "Korn";
      build(false);
    }
  });
</script>

<button class="back" onclick={() => (app.view = { name: "playlists" })}><Icon name="back" size={16} /> Playlists</button>
<h1>Get ready for a concert</h1>
<p class="intro muted">HomeFi looks up the setlists of the artist's latest shows and puts together the songs they're playing on this tour.</p>

<form class="form" onsubmit={(e) => (e.preventDefault(), build(false))}>
  <label class="field">
    <span>Artist</span>
    <!-- svelte-ignore a11y_autofocus -->
    <input type="text" autofocus placeholder="Who are you going to see?" bind:value={artist} disabled={playlists.building} autocomplete="off" spellcheck="false" />
  </label>

  <label class="field">
    <span>Shows to consider <b>{showsValue} recent shows</b></span>
    <input
      type="range"
      min={CONCERT_SHOWS_RANGE.min}
      max={CONCERT_SHOWS_RANGE.max}
      step="1"
      value={showsValue}
      disabled={playlists.building}
      aria-valuetext="{showsValue} recent shows"
      oninput={(e) => (shows = Number(e.currentTarget.value))}
    />
  </label>

  {#if sources}
    <p class="source muted">
      <Icon name="info" size={15} />
      <span>
        {#if sources.ai?.web_search}
          Uses your AI ({sources.ai.provider_name}) with web search · up to {settings.search_budget} searches
        {:else if sources.ai}
          {sources.ai.provider_name} can't search the web, so HomeFi will use Tidal's most played.
        {:else}
          No AI is set up, so HomeFi will use Tidal's most played. Set one up to get the real setlist.
          <button type="button" class="btn tonal inline" onclick={() => (app.view = { name: "settings", page: "ai" })}>Set up AI</button>
        {/if}
        {#if !sources.tidal}<br />Connect Tidal to find songs that aren't in your library.{/if}
      </span>
    </p>
  {/if}

  <div class="actions">
    <button type="submit" class="btn primary" disabled={!ready}><Icon name="play" size={16} /> Build playlist</button>
    <button type="button" class="btn tonal" disabled={!ready} onclick={() => build(true)}>Use Tidal's most played</button>
  </div>
</form>

<BuildStatus />
{#if playlists.error && !playlists.draft}<p class="error" role="alert">{playlists.error}</p>{/if}

{#if playlists.draft}
  <div class="result">
    <PlaylistPreview playlist={playlists.draft.playlist} fallbackNote={playlists.draft.fallback_note} saved={false} onsave={save} ondiscard={() => playlists.discard()} />
  </div>
{/if}

<style>
  .back {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--muted);
    margin-bottom: 8px;
    font-size: 13px;
  }
  .back:hover {
    color: var(--text);
  }
  .intro {
    max-width: 62ch;
    line-height: 1.5;
    margin: 8px 0 20px;
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 520px;
    margin-bottom: 20px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-weight: 600;
  }
  .field span {
    display: flex;
    justify-content: space-between;
  }
  .field b {
    font-weight: 500;
    color: var(--muted);
  }
  input[type="text"] {
    height: 40px;
    padding: 0 12px;
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    background: var(--surface-2);
    font-size: 15px;
  }
  input[type="range"] {
    accent-color: var(--primary-strong);
    width: 100%;
  }
  .source {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    margin: 0;
    line-height: 1.5;
    font-size: 13.5px;
  }
  .source :global(svg) {
    flex: none;
    margin-top: 3px;
  }
  .inline {
    height: 28px;
    margin-left: 6px;
    padding: 0 12px;
    font-size: 12.5px;
  }
  .actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .error {
    color: var(--danger);
  }
  .result {
    margin-top: 28px;
  }
</style>
