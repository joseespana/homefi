<script lang="ts">
  // "Make a playlist with AI": the chat with the user's own AI is the whole input. It opens at
  // once with a fixed question, the AI shapes the brief (title, length, energy, explicit) as
  // you talk, then HomeFi checks each suggested song against the library and Tidal. The same
  // session store as the party screen; all the rules live in homefi-core.
  import { onDestroy, onMount, tick } from "svelte";
  import { DEFAULT_PLAYLIST_SETTINGS, ENERGY_CURVES, partyLength } from "../../lib/api";
  import { party } from "../../lib/party.svelte";
  import { playlists } from "../../lib/playlists.svelte";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";
  import Skeleton from "../Skeleton.svelte";
  import BuildStatus from "./BuildStatus.svelte";
  import PartyChat from "./PartyChat.svelte";
  import PlaylistPreview from "./PlaylistPreview.svelte";

  /** The ?chat... mock flags that build by themselves (screenshots). */
  const DEV_BUILD_FLAGS = ["chatbuilt", "chatbuilding"];

  const view = $derived(app.view);
  const replaceId = $derived(view.name === "chat-playlist" ? (view.replace ?? null) : null);

  const seed = $derived(view.name === "chat-playlist" ? view.seed : undefined);

  let checked = $state(false);
  let result = $state<HTMLElement | null>(null);

  const needsAi = $derived(checked && (!playlists.sources?.ai || party.unavailable));
  const draft = $derived(party.draft);
  const defaults = $derived({ ...DEFAULT_PLAYLIST_SETTINGS, ...app.settings?.playlists });
  const brief = $derived(party.chatBrief ?? { wish: "", title: "", minutes: defaults.chat_minutes, curve: defaults.chat_curve, allow_explicit: defaults.chat_allow_explicit });
  /** "Sad 90s in English · 1 h · Steady · Explicit allowed" (the title shows "Not set yet" until the AI sets one). */
  const summary = $derived(
    [
      brief.title || "Not set yet",
      partyLength(brief.minutes),
      ENERGY_CURVES.find((c) => c.id === brief.curve)?.label ?? "",
      brief.allow_explicit ? "Explicit allowed" : "No explicit",
    ].join(" · "),
  );
  /** Nothing to build from until the user has said something. */
  const canBuild = $derived(party.userTurns > 0 && !party.building && !party.thinking);

  onMount(() => {
    (async () => {
      if (replaceId && !playlists.loaded) await playlists.load();
      await playlists.loadSources();
      checked = true;
      if (!playlists.sources?.ai) return;
      await party.open(replaceId, "chat", seed);
      if (!import.meta.env.DEV) return;
      const flags = new URLSearchParams(location.search);
      if (DEV_BUILD_FLAGS.some((f) => flags.has(f))) {
        await party.send("Sad 90s in English");
        await party.build();
      }
    })();
  });
  onDestroy(() => party.close());

  // the result arrives: the focus goes to its Play button
  $effect(() => {
    if (!draft) return;
    tick().then(() => result?.querySelector<HTMLElement>(".btn.primary")?.focus());
  });

  async function save() {
    const id = await party.save();
    if (id) app.view = { name: "home-playlist", id };
  }
  // Esc stops the build, unless the focus is in a text field
  function key(e: KeyboardEvent) {
    const typing = e.target instanceof HTMLElement && (e.target.isContentEditable || ["INPUT", "TEXTAREA"].includes(e.target.tagName));
    if (e.key === "Escape" && party.building && !typing) {
      e.preventDefault();
      party.cancel();
    }
  }
</script>

<svelte:window onkeydown={key} />

<button class="back" onclick={() => (app.view = { name: "playlists" })}><Icon name="back" size={16} /> Playlists</button>
<h1>Playlist with AI</h1>

{#if !checked}
  <Skeleton kind="grid" count={1} />
{:else if needsAi}
  <p class="source muted">
    <Icon name="info" size={15} />
    <span>Playlists with AI are made by your AI. Set one up in Settings → AI (your own key, nothing else is needed).</span>
  </p>
  <!-- svelte-ignore a11y_autofocus -->
  <button class="btn primary setup" autofocus onclick={() => (app.view = { name: "settings", page: "ai" })}>Set up an AI</button>
{:else}
  <p class="intro muted">Say what you're in the mood for and your AI shapes the playlist with you. HomeFi checks every song against your library and TIDAL.</p>

  {#if !party.opened}
    <Skeleton kind="grid" count={1} />
  {:else}
    <div class="main">
      <PartyChat disabled={party.building} />
      <p class="summary muted" aria-label="What the AI understood">{summary}</p>
      <div class="actions">
        <button type="button" class="btn primary" disabled={!canBuild} onclick={() => party.build()}><Icon name="play" size={16} /> {draft ? "Build again" : "Build playlist"}</button>
      </div>
      {#if party.error}<p class="error" role="alert">{party.error}</p>{/if}
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
  .main {
    display: flex;
    flex-direction: column;
    gap: 14px;
    max-width: 640px;
    margin-bottom: 20px;
  }
  .summary {
    margin: 0;
    font-size: 12.5px;
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
    max-width: 640px;
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
