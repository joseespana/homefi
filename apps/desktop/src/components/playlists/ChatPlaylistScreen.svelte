<script lang="ts">
  // "Make a playlist with AI": the chat with the user's own AI is the whole input. It opens at
  // once with a fixed question, the AI shapes the brief (title, length, energy, explicit) as
  // you talk, then HomeFi checks each suggested song against the library and Tidal. The same
  // session store as the party screen; all the rules live in homefi-core.
  import { onDestroy, tick } from "svelte";
  import { DEFAULT_PLAYLIST_SETTINGS, ENERGY_CURVES, partyLength } from "../../lib/api";
  import { conversations } from "../../lib/party.svelte";
  import { playlists } from "../../lib/playlists.svelte";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";
  import Skeleton from "../Skeleton.svelte";
  import BuildStatus from "./BuildStatus.svelte";
  import ChatClosed from "./ChatClosed.svelte";
  import PartyChat from "./PartyChat.svelte";
  import PlaylistPreview from "./PlaylistPreview.svelte";

  /** The ?chat... mock flags that build by themselves (screenshots). */
  const DEV_BUILD_FLAGS = ["chatbuilt", "chatbuilding"];

  const view = $derived(app.view);
  const chat = $derived(view.name === "chat-playlist" ? conversations.get(view.conversation) : undefined);

  let result = $state<HTMLElement | null>(null);

  const needsAi = $derived(!!chat && !chat.checking && (!playlists.sources?.ai || chat.unavailable));
  const draft = $derived(chat?.draft);
  const defaults = $derived({ ...DEFAULT_PLAYLIST_SETTINGS, ...app.settings?.playlists });
  const brief = $derived(chat?.chatBrief ?? { wish: "", title: "", minutes: defaults.chat_minutes, curve: defaults.chat_curve, allow_explicit: defaults.chat_allow_explicit });
  /** What the AI understood besides the title: "1 h", "Steady", "Explicit allowed". */
  const facts = $derived(
    [partyLength(brief.minutes), ENERGY_CURVES.find((c) => c.id === brief.curve)?.label ?? "", brief.allow_explicit ? "Explicit allowed" : "No explicit"].filter(Boolean),
  );
  /** Nothing to build from until the user has said something. */
  const canBuild = $derived(!!chat && chat.userTurns > 0 && !chat.building && !chat.thinking);

  // dev mock flags: the chat sends a first message and builds by itself (screenshots), once
  let devRan = false;
  $effect(() => {
    if (!import.meta.env.DEV || devRan || !chat?.opened || chat.messages.length > 1) return;
    const flags = new URLSearchParams(location.search);
    if (!DEV_BUILD_FLAGS.some((f) => flags.has(f))) return;
    devRan = true;
    const c = chat;
    void c.send("Sad 90s in English").then(() => c.build());
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

  async function save() {
    const c = chat;
    const id = await c?.save();
    if (!c || !id) return;
    // the playlist is kept: its chat is done ("Keep chatting" opens it again from the saved brief)
    conversations.close(c.id);
    app.view = { name: "home-playlist", id };
  }
  // Esc stops the build, unless the focus is in a text field
  function key(e: KeyboardEvent) {
    const typing = e.target instanceof HTMLElement && (e.target.isContentEditable || ["INPUT", "TEXTAREA"].includes(e.target.tagName));
    if (e.key === "Escape" && chat?.building && !typing) {
      e.preventDefault();
      chat.cancel();
    }
  }
</script>

<svelte:window onkeydown={key} />

{#if !chat}
  <ChatClosed />
{:else}
<button class="back" onclick={() => (app.view = { name: "playlists" })}><Icon name="back" size={16} /> Playlists</button>

<div class="screen">
  <header class="hero">
    <span class="mark" aria-hidden="true"><Icon name="ai" size={22} /></span>
    <h1>Playlist with AI</h1>
    <p class="muted">Say what you're in the mood for and your AI shapes the playlist with you. HomeFi checks every song against your library and TIDAL.</p>
  </header>

  {#if chat.checking}
    <div class="main"><Skeleton kind="grid" count={1} /></div>
  {:else if needsAi}
    <div class="main setup-box">
      <p class="muted">Playlists with AI are made by your AI. Set one up in Settings → AI (your own key, nothing else is needed).</p>
      <!-- svelte-ignore a11y_autofocus -->
      <button class="btn primary" autofocus onclick={() => (app.view = { name: "settings", page: "ai" })}>Set up an AI</button>
    </div>
  {:else if !chat.opened}
    <div class="main"><Skeleton kind="grid" count={1} /></div>
  {:else}
    <div class="main">
      <PartyChat {chat} disabled={chat.building} />
      <div class="bar">
        <ul class="brief" aria-label="What the AI understood">
          <li class="tag lead" class:pending={!brief.title}>{brief.title || "Tell your AI what you want"}</li>
          {#each brief.songs ?? [] as song (song.artist + song.title)}<li class="tag song" title="Opens the playlist"><Icon name="music" size={12} /> <span class="ellipsis">{song.title} · {song.artist}</span></li>{/each}
          {#each facts as fact (fact)}<li class="tag">{fact}</li>{/each}
        </ul>
        <button type="button" class="btn primary build" disabled={!canBuild} onclick={() => chat.build()}><Icon name="play" size={16} /> {draft ? "Build again" : "Build playlist"}</button>
      </div>
      {#if chat.error}<p class="alert" role="alert"><Icon name="alert" size={16} /><span>{chat.error}</span></p>{/if}
      <BuildStatus active={chat.building} text={chat.statusText} fraction={chat.fraction} oncancel={() => chat.cancel()} />
    </div>

    <div class="result" class:dim={chat.building} bind:this={result}>
      {#if draft}
        <span class="sr" role="status">{draft.playlist.entries.length} songs ready</span>
        <PlaylistPreview playlist={draft.playlist} fallbackNote={draft.note} saved={!!chat.replaceId} rebuildable={false} busy={chat.building} onsave={save} ondiscard={() => chat.discard()} />
      {/if}
    </div>
  {/if}
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
  /* one centred column for the talk; the playlist below gets the full reading width */
  .screen {
    max-width: 1040px;
    margin-inline: auto;
  }
  .hero {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 8px;
    margin: 4px auto 22px;
    max-width: 560px;
  }
  .hero h1 {
    margin: 4px 0 0;
  }
  .hero p {
    margin: 0;
    line-height: 1.5;
    text-wrap: balance;
  }
  .mark {
    display: grid;
    place-items: center;
    width: 52px;
    height: 52px;
    border-radius: var(--r-md);
    color: var(--on-primary);
    background: linear-gradient(135deg, color-mix(in srgb, var(--primary) 80%, white), var(--primary));
    box-shadow: 0 10px 28px -10px color-mix(in srgb, var(--primary) 70%, transparent);
  }
  .main {
    display: flex;
    flex-direction: column;
    gap: 12px;
    width: min(720px, 100%);
    margin-inline: auto;
  }
  .setup-box {
    align-items: center;
    text-align: center;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  .brief {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    flex: 1;
    min-width: 0;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .tag {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    max-width: 100%;
    height: 26px;
    padding: 0 10px;
    border-radius: var(--r-sm);
    background: var(--glass-strong);
    border: 1px solid var(--glass-border);
    color: var(--muted);
    font-size: 12px;
    white-space: nowrap;
  }
  .tag.lead {
    color: var(--text);
    font-weight: 600;
  }
  .tag.lead.pending {
    color: var(--muted);
    font-weight: 400;
    font-style: italic;
  }
  .tag.song {
    color: var(--accent-text);
  }
  .build {
    flex: none;
    margin-left: auto;
  }
  .alert {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin: 0;
    padding: 10px 12px;
    border-radius: var(--r-sm);
    border: 1px solid color-mix(in srgb, var(--danger) 45%, transparent);
    background: color-mix(in srgb, var(--danger) 12%, transparent);
    color: var(--text);
    font-size: 13px;
    line-height: 1.45;
  }
  .alert :global(svg) {
    flex: none;
    margin-top: 1px;
    color: var(--danger);
  }
  .result {
    margin-top: 32px;
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
