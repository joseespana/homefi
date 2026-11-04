<script lang="ts">
  // "Make a party playlist": a few tappable questions (already answered from Settings →
  // Playlists), an optional talk with the user's own AI, then HomeFi checks each suggested song
  // against the library and Tidal. All the rules live in homefi-core; this draws the state.
  import { onDestroy, onMount, tick } from "svelte";
  import { DEFAULT_PLAYLIST_SETTINGS } from "../../lib/api";
  import { party } from "../../lib/party.svelte";
  import { playlists } from "../../lib/playlists.svelte";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";
  import Skeleton from "../Skeleton.svelte";
  import BuildStatus from "./BuildStatus.svelte";
  import PartyChat from "./PartyChat.svelte";
  import PartyQuestions from "./PartyQuestions.svelte";
  import PlaylistPreview from "./PlaylistPreview.svelte";

  /** The ?party... mock flags that build by themselves (screenshots). */
  const DEV_BUILD_FLAGS = ["partybuilt", "partybuilding", "partyshort"];

  const view = $derived(app.view);
  const replaceId = $derived(view.name === "party" ? (view.replace ?? null) : null);
  const defaults = $derived({ ...DEFAULT_PLAYLIST_SETTINGS, ...app.settings?.playlists });

  let checked = $state(false);
  let result = $state<HTMLElement | null>(null);

  const needsAi = $derived(checked && (!playlists.sources?.ai || party.unavailable));
  const draft = $derived(party.draft);

  onMount(() => {
    (async () => {
      if (replaceId && !playlists.loaded) await playlists.load();
      await playlists.loadSources();
      checked = true;
      if (!playlists.sources?.ai) return;
      await party.open(replaceId);
      if (!import.meta.env.DEV) return;
      const flags = new URLSearchParams(location.search);
      if (flags.has("partychat")) await party.talk();
      else if (DEV_BUILD_FLAGS.some((f) => flags.has(f))) await party.build();
    })();
  });
  onDestroy(() => party.close());

  // the result arrives: the focus goes to its Play button
  $effect(() => {
    if (!draft) return;
    tick().then(() => result?.querySelector<HTMLElement>(".btn.primary")?.focus());
  });

  function submit(e: Event) {
    e.preventDefault();
    party.build();
  }
  async function save() {
    const id = await party.save();
    if (id) app.view = { name: "home-playlist", id };
  }
  // Esc stops the build, unless the focus is in a text field
  function key(e: KeyboardEvent) {
    const typing = e.target instanceof HTMLElement && (e.target.isContentEditable || ["INPUT", "TEXTAREA"].includes(e.target.tagName) && e.target.getAttribute("type") !== "range");
    if (e.key === "Escape" && party.building && !typing) {
      e.preventDefault();
      party.cancel();
    }
  }
</script>

<svelte:window onkeydown={key} />

<button class="back" onclick={() => (app.view = { name: "playlists" })}><Icon name="back" size={16} /> Playlists</button>
<h1>Party playlist</h1>

{#if !checked}
  <Skeleton kind="grid" count={1} />
{:else if needsAi}
  <p class="source muted">
    <Icon name="info" size={15} />
    <span>Party playlists are made by your AI. Set one up in Settings → AI (your own key, nothing else is needed).</span>
  </p>
  <!-- svelte-ignore a11y_autofocus -->
  <button class="btn primary setup" autofocus onclick={() => (app.view = { name: "settings", page: "ai" })}>Set up an AI</button>
{:else}
  <p class="intro muted">Answer a few questions and your AI picks the songs. HomeFi checks each one against your library and TIDAL.</p>

  {#if !party.opened}
    <Skeleton kind="grid" count={1} />
  {:else}
    <div class="layout" class:with-chat={party.chatting}>
      <form class="form" onsubmit={submit}>
        <PartyQuestions brief={party.brief} disabled={party.building} defaults={{ curve: defaults.party_curve, allow_explicit: defaults.party_allow_explicit }} onchange={(patch) => party.setBrief(patch)} />
        <div class="actions">
          <button type="submit" class="btn primary" disabled={party.building || party.thinking}><Icon name="play" size={16} /> {draft ? "Build again" : "Build playlist"}</button>
          {#if !party.chatting}
            <button type="button" class="btn tonal" disabled={party.building || party.thinking} onclick={() => party.talk()}>Talk it over with the AI first</button>
          {/if}
        </div>
        {#if party.error}<p class="error" role="alert">{party.error}</p>{/if}
      </form>
      {#if party.chatting}<PartyChat disabled={party.building} />{/if}
    </div>

    <div class="status"><BuildStatus active={party.building} text={party.statusText} fraction={party.fraction} oncancel={() => party.cancel()} /></div>

    <div class="result" class:dim={party.building} bind:this={result}>
      {#if draft}
        <span class="sr" role="status">{draft.playlist.entries.length} songs ready</span>
        <PlaylistPreview playlist={draft.playlist} fallbackNote={draft.note} saved={!!party.replaceId} rebuildable={false} busy={party.building} onsave={save} ondiscard={() => party.discard()} />
      {/if}
    </div>
  {/if}
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
  .layout {
    display: grid;
    gap: 24px;
    grid-template-columns: minmax(0, 560px);
    margin-bottom: 20px;
  }
  @media (min-width: 1100px) {
    .layout.with-chat {
      grid-template-columns: 560px minmax(320px, 1fr);
      gap: 32px;
      align-items: start;
    }
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }
  .actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .source {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    max-width: 62ch;
    margin: 8px 0 16px;
    line-height: 1.5;
    font-size: 13.5px;
  }
  .source :global(svg) {
    flex: none;
    margin-top: 3px;
  }
  .error {
    color: var(--danger);
    margin: 0;
  }
  .status {
    max-width: 560px;
  }
  .result {
    margin-top: 28px;
  }
  .result.dim {
    opacity: 0.55;
    transition: opacity 0.2s;
  }
  .result:empty {
    margin: 0;
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
