<script lang="ts">
  // "Make a party playlist": a few tappable questions (already answered from Settings →
  // Playlists), an optional talk with the user's own AI, then HomeFi checks each suggested song
  // against the library and Tidal. All the rules live in homefi-core; this draws the state.
  import { onDestroy, tick } from "svelte";
  import { DEFAULT_PLAYLIST_SETTINGS } from "../../lib/api";
  import { conversations } from "../../lib/party.svelte";
  import { playlists } from "../../lib/playlists.svelte";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";
  import Skeleton from "../Skeleton.svelte";
  import BuildStatus from "./BuildStatus.svelte";
  import ChatClosed from "./ChatClosed.svelte";
  import PartyChat from "./PartyChat.svelte";
  import PartyQuestions from "./PartyQuestions.svelte";
  import PlaylistPreview from "./PlaylistPreview.svelte";

  /** The ?party... mock flags that build by themselves (screenshots). */
  const DEV_BUILD_FLAGS = ["partybuilt", "partybuilding", "partyshort"];

  const view = $derived(app.view);
  const chat = $derived(view.name === "party" ? conversations.get(view.conversation) : undefined);
  const defaults = $derived({ ...DEFAULT_PLAYLIST_SETTINGS, ...app.settings?.playlists });

  let result = $state<HTMLElement | null>(null);

  const needsAi = $derived(!!chat && !chat.checking && (!playlists.sources?.ai || chat.unavailable));
  const draft = $derived(chat?.draft);

  // dev mock flags: the screen talks or builds by itself (screenshots), once
  let devRan = false;
  $effect(() => {
    if (!import.meta.env.DEV || devRan || !chat?.opened) return;
    const flags = new URLSearchParams(location.search);
    const c = chat;
    if (flags.has("partychat")) void c.talk();
    else if (DEV_BUILD_FLAGS.some((f) => flags.has(f))) void c.build();
    else return;
    devRan = true;
  });
  // a chat without an AI to talk to isn't kept: leaving its screen closes it
  onDestroy(() => {
    if (chat?.unavailable) conversations.close(chat.id);
  });

  // the result arrives: the focus goes to its Play button
  $effect(() => {
    if (!draft) return;
    tick().then(() => result?.querySelector<HTMLElement>(".btn.primary")?.focus());
  });

  function submit(e: Event) {
    e.preventDefault();
    chat?.build();
  }
  async function save() {
    const c = chat;
    const id = await c?.save();
    if (!c || !id) return;
    // the playlist is kept: its chat is done ("Change answers" opens it again from the saved answers)
    conversations.close(c.id);
    app.view = { name: "home-playlist", id };
  }
  // Esc stops the build, unless the focus is in a text field
  function key(e: KeyboardEvent) {
    const typing = e.target instanceof HTMLElement && (e.target.isContentEditable || ["INPUT", "TEXTAREA"].includes(e.target.tagName) && e.target.getAttribute("type") !== "range");
    if (e.key === "Escape" && chat?.building && !typing) {
      e.preventDefault();
      chat?.cancel();
    }
  }
</script>

<svelte:window onkeydown={key} />

{#if !chat}
  <ChatClosed />
{:else}
<button class="back" onclick={() => (app.view = { name: "playlists" })}><Icon name="back" size={16} /> Playlists</button>
<h1>Party playlist</h1>

{#if chat.checking}
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

  {#if !chat.opened}
    <Skeleton kind="grid" count={1} />
  {:else}
    <div class="layout" class:with-chat={chat.chatting}>
      <form class="form" onsubmit={submit}>
        <PartyQuestions brief={chat.brief} disabled={chat.building} defaults={{ curve: defaults.party_curve, allow_explicit: defaults.party_allow_explicit }} onchange={(patch) => chat.setBrief(patch)} />
        <div class="actions">
          <button type="submit" class="btn primary" disabled={chat.building || chat.thinking}><Icon name="play" size={16} /> {draft ? "Build again" : "Build playlist"}</button>
          {#if !chat.chatting}
            <button type="button" class="btn tonal" disabled={chat.building || chat.thinking} onclick={() => chat.talk()}>Talk it over with the AI first</button>
          {/if}
        </div>
        {#if chat.error}<p class="error" role="alert">{chat.error}</p>{/if}
      </form>
      {#if chat.chatting}<PartyChat {chat} disabled={chat.building} />{/if}
    </div>

    <div class="status"><BuildStatus active={chat.building} text={chat.statusText} fraction={chat.fraction} oncancel={() => chat.cancel()} /></div>

    <div class="result" class:dim={chat.building} bind:this={result}>
      {#if draft}
        <span class="sr" role="status">{draft.playlist.entries.length} songs ready</span>
        <PlaylistPreview playlist={draft.playlist} fallbackNote={draft.note} saved={!!chat.replaceId} rebuildable={false} busy={chat.building} onsave={save} ondiscard={() => chat.discard()} />
      {/if}
    </div>
  {/if}
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
