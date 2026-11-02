<script lang="ts">
  // "Set up my sound": one chat for both the AI assistant (the user's own key)
  // and the guided wizard. The conversation lives in lib/assistant.svelte.ts
  // (the inline "Ask the AI" panel on the Sound screen shares it). A confirmed
  // proposal is already saved and applied by the backend; then we jump to Sound.
  import { onDestroy, tick, untrack } from "svelte";
  import { assistant as a, type Msg } from "../../lib/assistant.svelte";
  import { fade, rise } from "../../lib/motion";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";
  import EqGraph from "./EqGraph.svelte";
  import SuggestionCard from "../sound/SuggestionCard.svelte";
  import TasteProposal from "../sound/TasteProposal.svelte";

  /** How long "Applied" shows before Sound opens by itself. */
  const OPEN_SOUND_AFTER_MS = 2200;

  let text = $state("");
  let list = $state<HTMLElement | null>(null);
  let input = $state<HTMLInputElement | null>(null);

  $effect(() => {
    a.loadStatus();
  });
  // opened with a first note ("Set up my sound" from a sound, the Add key flow)
  $effect(() => {
    const v = app.view;
    if (v.name === "assistant" && v.say) untrack(() => ((a.notes = v.say!), (a.phase = "intro")));
  });
  // "Ask the AI to refine" a sound: the AI starts from its curve right away
  $effect(() => {
    if (a.focus && a.loaded && a.ready && a.phase === "intro") untrack(() => a.start(false));
  });
  // an applied profile closes the chat after a moment
  $effect(() => {
    if (!(a.applied && a.done)) return;
    const t = setTimeout(openSound, OPEN_SOUND_AFTER_MS);
    return () => clearTimeout(t);
  });
  $effect(() => {
    if (!a.thinking) tick().then(() => input?.focus());
  });

  function send(reply: string) {
    text = "";
    a.send(reply);
  }
  const openSound = () => (app.view = { name: "sound", output: a.applied?.output_id });
  const keepTalking = (m: Msg) => (a.keepTalking(m), input?.focus());

  // the newest message stays in view
  $effect(() => {
    a.messages.length;
    a.thinking;
    // the page (not the log) scrolls: bring the last message above the composer
    tick().then(() => requestAnimationFrame(() => list?.lastElementChild?.scrollIntoView({ block: "end", behavior: "smooth" })));
  });

  onDestroy(() => (a.focus = null));
</script>

<div class="assistant">
  <header class="head">
    <button class="icon-btn" title="Back to Sound" aria-label="Back to Sound" onclick={() => (app.view = { name: "sound" })}><Icon name="back" /></button>
    <div class="titles">
      <h1>Set up my sound</h1>
      {#if a.phase === "chat"}
        <p class="muted small">{a.guided ? "Guided setup · a few quick questions" : `Chatting with ${a.provider?.name ?? "your AI"}`}</p>
      {/if}
    </div>
    {#if a.phase === "chat"}<button class="btn tonal" onclick={() => a.restart()}><Icon name="refresh" size={16} /> Start over</button>{/if}
  </header>

  {#if a.phase === "intro"}
    <section class="intro" in:rise>
      <div class="hero"><Icon name="ai" size={30} /></div>
      <h2>Your headphones, your taste</h2>
      <p class="muted">Tell HomeFi what you listen with and how you like it. It finds the measured correction for your headphones, adds your taste and shows you the curve before anything changes.</p>
      <label class="notes">
        <span>Anything to tell first? <small class="muted">optional</small></span>
        <textarea value={a.notes} oninput={(e) => (a.notes = e.currentTarget.value)} rows="2" maxlength="500" placeholder="e.g. Sennheiser HD 650 on a USB DAC, I listen to jazz and find them a bit dark"></textarea>
      </label>
      {#if !a.loaded}
        <p class="muted small">Checking your AI setup…</p>
      {:else if a.ready}
        <div class="choices">
          <button class="btn primary" onclick={() => a.start(false)}><Icon name="ai" size={16} /> Chat with {a.provider?.name}</button>
          <button class="btn tonal" onclick={() => a.start(true)}>Guided questions instead</button>
        </div>
        <p class="muted tiny"><Icon name="shield" size={12} /> Only what the conversation needs goes to {a.provider?.name} (your outputs, the genres and artists you play most): never your music files or your Tidal session.</p>
      {:else}
        <div class="choices">
          <button class="btn primary" onclick={() => a.start(true)}>Start the guided setup</button>
        </div>
        <div class="no-ai">
          <Icon name="key" size={16} />
          <span>
            {#if a.provider && a.provider.needs_key}
              {a.provider.name} has no key yet.
            {:else}
              Prefer to just talk about it?
            {/if}
            Bring your own AI key (Claude, ChatGPT, Gemini…) or a local Ollama to chat instead.
            <button class="link" onclick={() => (app.view = { name: "settings", page: "ai" })}>Set up AI</button>
          </span>
        </div>
      {/if}
      <div class="suggest"><SuggestionCard /></div>
    </section>
  {:else}
    <section class="chat">
      <div class="log" bind:this={list} aria-live="polite">
        {#each a.messages as m (m.id)}
          <div class="msg {m.from}" in:rise={{ y: 8, duration: 220 }}>
            {#if m.from === "bot"}<span class="avatar" aria-hidden="true"><Icon name="ai" size={14} /></span>{/if}
            <div class="bubble-col">
              {#if m.text}
                <div class="bubble">
                  {#if m.from === "error"}<Icon name="alert" size={14} />{/if}
                  <span class="text">{m.text}</span>
                  {#if m.retry && m.id === a.messages[a.messages.length - 1].id}<button class="link" onclick={() => a.retry(m)}>Try again</button>{/if}
                </div>
              {/if}
              {#if m.tasteProposal}
                <TasteProposal rule={m.tasteProposal} open={!m.settled && !a.done} busy={a.thinking} onapply={() => a.applyTaste()} onkeep={() => keepTalking(m)} />
              {/if}
              {#if m.proposal}
                {@const p = m.proposal}
                <div class="proposal">
                  <div class="ptitle">
                    <b>{p.profile.name}</b>
                    <span class="muted">for {p.output_id}</span>
                  </div>
                  <EqGraph profile={p.profile} compare={m.before ?? null} height={170} />
                  <div class="legend"><span class="now-line"></span> New <span class="old-line"></span> Now</div>
                  {#if p.explanation && !m.text.includes(p.explanation)}<p class="explain">{p.explanation}</p>{/if}
                  {#if !m.settled && !a.done}
                    <div class="pbuttons">
                      <button class="btn primary" disabled={a.thinking} onclick={() => a.applyProfile()}><Icon name="check" size={16} /> Apply</button>
                      <button class="btn tonal" disabled={a.thinking} onclick={() => keepTalking(m)}>Keep talking</button>
                    </div>
                  {/if}
                </div>
              {/if}
            </div>
          </div>
        {/each}
        {#if a.thinking}
          <div class="msg bot" in:fade>
            <span class="avatar" aria-hidden="true"><Icon name="ai" size={14} /></span>
            <div class="bubble typing" aria-label="Thinking"><i></i><i></i><i></i></div>
          </div>
        {/if}
        {#if a.tasteSaved}
          <div class="success" in:rise>
            <Icon name="layers" size={18} />
            <span><b>Saved.</b> {a.tasteSaved}</span>
            <button class="btn tonal" onclick={() => (app.view = { name: "sound" })}>Sound</button>
          </div>
        {/if}
        {#if a.applied}
          <div class="success" in:rise>
            <Icon name="check" size={18} />
            <span><b>Applied.</b> “{a.applied.profile.name}” is on for {a.applied.output_id}.{a.done ? " Opening Sound…" : ""}</span>
            <button class="btn tonal" onclick={openSound}>Open now</button>
          </div>
        {:else if a.done}
          <div class="ended" in:rise>
            <span class="muted">The conversation is over.</span>
            <button class="btn tonal" onclick={() => a.restart()}>Start again</button>
            <button class="btn tonal" onclick={() => (app.view = { name: "sound" })}>Open Sound</button>
          </div>
        {/if}
      </div>

      {#if !a.done}
        {#if a.chips.length && !a.thinking}
          <div class="chips" in:fade>
            {#each a.chips as q}<button onclick={() => send(q)}>{q}</button>{/each}
          </div>
        {/if}
        <form class="composer" onsubmit={(e) => (e.preventDefault(), send(text))}>
          <input bind:this={input} bind:value={text} placeholder={a.guided ? "Type an answer, or pick one above" : "Write a message"} maxlength="1000" disabled={a.thinking} aria-label="Your answer" />
          <button class="btn primary" disabled={!text.trim() || a.thinking} aria-label="Send"><Icon name="send" size={16} /></button>
        </form>
      {/if}
    </section>
  {/if}
</div>

<style>
  .suggest {
    width: 100%;
    margin-top: 8px;
    text-align: left;
  }
  .assistant {
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 820px;
    margin: 0 auto;
    min-height: calc(100vh - 190px);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .titles {
    flex: 1;
    min-width: 0;
  }
  .titles p {
    margin: 2px 0 0;
  }
  .small {
    font-size: 12.5px;
  }
  .tiny {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    margin: 0;
  }
  .intro {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
    padding: 24px;
    border-radius: min(18px, var(--r-max));
    background: var(--glass);
    border: 1px solid var(--glass-border);
    backdrop-filter: blur(18px);
  }
  .intro h2 {
    margin: 0;
  }
  .intro p {
    margin: 0;
    max-width: 620px;
  }
  .hero {
    display: grid;
    place-items: center;
    width: 56px;
    height: 56px;
    border-radius: min(16px, var(--r-max));
    background: var(--gradient);
    color: #fff;
  }
  .notes {
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: 100%;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--muted);
  }
  textarea {
    font: inherit;
    font-weight: 400;
    font-size: 14px;
    color: var(--text);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: min(10px, var(--r-max));
    padding: 8px 10px;
    resize: vertical;
    outline: none;
  }
  .choices {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }
  .no-ai {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 10px 12px;
    border-radius: min(12px, var(--r-max));
    background: var(--surface-2);
    font-size: 13px;
    color: var(--muted);
  }
  .link {
    padding: 0 2px;
    color: var(--primary);
    font-weight: 600;
  }
  .link:hover {
    text-decoration: underline;
  }
  .chat {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 0;
  }
  .log {
    flex: 1;
    scroll-padding-bottom: 90px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 4px 2px 8px;
  }
  .msg,
  .success,
  .ended {
    scroll-margin-bottom: 90px;
  }
  .msg {
    display: flex;
    gap: 8px;
    align-items: flex-end;
    max-width: 85%;
  }
  .msg.me {
    align-self: flex-end;
  }
  .bubble-col {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .avatar {
    flex: none;
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: var(--gradient);
    color: #fff;
  }
  .bubble {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    padding: 10px 14px;
    border-radius: min(18px, var(--r-max)) min(18px, var(--r-max)) min(18px, var(--r-max)) min(6px, var(--r-max));
    background: var(--surface-2);
    border: 1px solid var(--border);
    line-height: 1.5;
    user-select: text;
  }
  .text {
    white-space: pre-wrap;
  }
  .me .bubble {
    border-radius: min(18px, var(--r-max)) min(18px, var(--r-max)) min(6px, var(--r-max)) min(18px, var(--r-max));
    background: var(--primary-strong);
    border-color: transparent;
    color: var(--on-primary-strong);
  }
  .error .bubble {
    flex-wrap: wrap;
    color: var(--danger);
    background: color-mix(in srgb, var(--danger) 10%, var(--surface));
    border-color: color-mix(in srgb, var(--danger) 35%, transparent);
  }
  .typing {
    gap: 4px;
    padding: 14px 16px;
  }
  .typing i {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--muted);
    animation: bounce calc(1.1s / var(--motion-speed)) infinite ease-in-out;
  }
  .typing i:nth-child(2) {
    animation-delay: 0.15s;
  }
  .typing i:nth-child(3) {
    animation-delay: 0.3s;
  }
  @keyframes bounce {
    0%,
    60%,
    100% {
      transform: translateY(0);
      opacity: 0.45;
    }
    30% {
      transform: translateY(-5px);
      opacity: 1;
    }
  }
  :global(:root[data-motion="subtle"]) .typing i {
    animation-name: blink;
  }
  @keyframes blink {
    50% {
      opacity: 1;
    }
  }
  .proposal {
    width: min(560px, 70vw);
    padding: 12px 14px;
    border-radius: min(16px, var(--r-max));
    background: var(--glass);
    border: 1px solid color-mix(in srgb, var(--accent-text) 40%, var(--glass-border));
    box-shadow: 0 10px 30px -18px color-mix(in srgb, var(--accent-text) 80%, transparent);
  }
  .ptitle {
    display: flex;
    gap: 8px;
    align-items: baseline;
    margin-bottom: 6px;
    font-size: 13px;
  }
  .legend {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 4px;
    font-size: 11.5px;
    color: var(--muted);
  }
  .now-line,
  .old-line {
    width: 18px;
    height: 0;
    border-top: 2.5px solid var(--accent-text);
  }
  .old-line {
    margin-left: 10px;
    border-top: 1.6px dashed var(--muted);
  }
  .explain {
    margin: 8px 0 0;
    font-size: 13px;
    line-height: 1.5;
  }
  .pbuttons {
    display: flex;
    gap: 8px;
    margin-top: 10px;
  }
  .success,
  .ended {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px;
    padding: 12px 14px;
    border-radius: min(14px, var(--r-max));
    background: color-mix(in srgb, var(--ok) 12%, var(--surface));
    border: 1px solid color-mix(in srgb, var(--ok) 40%, transparent);
  }
  .success > :global(svg) {
    color: var(--ok);
  }
  .success span {
    flex: 1;
    min-width: 200px;
  }
  .ended {
    background: var(--surface-2);
    border-color: var(--border);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .chips button {
    height: 34px;
    padding: 0 14px;
    border-radius: min(999px, var(--r-max));
    border: 1px solid color-mix(in srgb, var(--primary) 55%, var(--border));
    color: var(--primary);
    background: color-mix(in srgb, var(--primary) 8%, var(--surface));
    font-size: 13px;
    font-weight: 600;
    transition: background 0.15s;
  }
  .chips button:hover {
    background: color-mix(in srgb, var(--primary) 18%, var(--surface));
  }
  .composer {
    position: sticky;
    bottom: 0;
    display: flex;
    gap: 8px;
    padding: 8px;
    border-radius: min(16px, var(--r-max));
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
  }
  .composer input {
    flex: 1;
    min-width: 0;
    border: 0;
    background: transparent;
  }
  .composer .btn {
    width: 40px;
    padding: 0;
    justify-content: center;
  }
</style>
