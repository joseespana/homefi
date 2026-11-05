<script lang="ts">
  // "Ask the AI" on the hero: a field, a few chips and, once there is a
  // conversation, a panel with the last turns (each proposal with Try / Keep).
  // It shares the conversation with the "Set up my sound" page. Without an AI
  // key the field says so (Add key) and the chips still work: they make the
  // same small steps as "How does it sound?". Esc closes the panel.
  import { tick } from "svelte";
  import type { SoundReaction, TasteRule } from "../../lib/api";
  import { INLINE_TURNS, assistant, type Msg } from "../../lib/assistant.svelte";
  import { fade, rise } from "../../lib/motion";
  import { kindLabel, ruleWords, sound } from "../../lib/sound.svelte";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";

  /** The quick asks: what they say to the AI, and the reaction they stand for without one. */
  const CHIPS: { label: string; reaction: SoundReaction }[] = [
    { label: "Warmer", reaction: "needsBass" },
    { label: "More bass", reaction: "needsBass" },
    { label: "Vocals clearer", reaction: "vocalsBuried" },
    { label: "Less harsh", reaction: "tooBright" },
    { label: "Quieter highs", reaction: "tooBright" },
  ];

  let text = $state("");
  let input = $state<HTMLInputElement | null>(null);
  let log = $state<HTMLElement | null>(null);

  $effect(() => {
    assistant.loadStatus();
  });
  const noKey = $derived(assistant.loaded && !assistant.ready);
  const turns = $derived(assistant.messages.slice(-INLINE_TURNS));
  const panel = $derived(sound.askOpen && !noKey && (turns.length > 0 || assistant.thinking));

  // opened from the player button, the A key or the Sound ▸ menu: the field is ready to type in
  $effect(() => {
    if (!sound.askOpen) return;
    const seed = sound.askSeed;
    sound.askSeed = null;
    if (seed) text = seed;
    tick().then(() => input?.focus());
  });
  $effect(() => {
    turns.length;
    assistant.thinking;
    tick().then(() => log?.lastElementChild?.scrollIntoView({ block: "end" }));
  });

  function send(e?: Event) {
    e?.preventDefault();
    const t = text.trim();
    if (!t) return;
    text = "";
    sound.askOpen = true;
    assistant.ask(t);
  }
  function chip(c: (typeof CHIPS)[number]) {
    if (noKey) return sound.react(c.reaction);
    sound.askOpen = true;
    assistant.ask(c.label === "Warmer" ? "Make it warmer" : c.label);
  }
  function key(e: KeyboardEvent) {
    if (e.key === "Escape" && sound.askOpen) {
      e.stopPropagation();
      sound.askOpen = false;
      input?.focus();
    }
  }

  const keepFor = (rule: TasteRule) => sound.keepScopes(rule).primary;
  function keep(rule: TasteRule) {
    const scope = keepFor(rule);
    if (scope) sound.keep(rule, scope, sound.onlyHere ? sound.output : null);
    else app.note("Play something to keep it for", "info", "sound-try");
  }
  const mine = (m: Msg) => m.from === "me";
</script>

<!-- the key handler only closes the panel; the field inside is the keyboard way in -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="ask" onkeydown={key}>
  {#if panel}
    <div class="log" bind:this={log} aria-live="polite" in:rise={{ y: 6, duration: 180 }}>
      <span class="cap">Ask the AI</span>
      {#each turns as m (m.id)}
        <div class="turn" class:me={mine(m)} class:err={m.from === "error"}>
          {#if m.text}<p>{mine(m) ? "You: " : ""}{m.text}</p>{/if}
          {#if m.tasteProposal}
            {@const r = m.tasteProposal}
            <div class="proposal">
              <div class="what">
                <b class="ellipsis">{r.label}</b>
                <span class="muted mono">{ruleWords(r)}</span>
              </div>
              {#if sound.ruleById(r.id)}
                <span class="muted saved"><Icon name="check" size={12} /> Kept</span>
              {:else}
                <div class="acts">
                  <button class="btn tonal" onclick={() => sound.tryNow(r)}>Try</button>
                  <button class="btn primary" onclick={() => keep(r)}>Keep{keepFor(r) ? ` for ${kindLabel(keepFor(r)!.kind).toLowerCase()}` : ""}</button>
                </div>
              {/if}
            </div>
          {/if}
          {#if m.retry && m === turns[turns.length - 1]}<button class="link" onclick={() => assistant.retry(m)}>Try again</button>{/if}
        </div>
      {/each}
      {#if assistant.thinking}<div class="typing" aria-label="Thinking" in:fade><i></i><i></i><i></i></div>{/if}
      {#if assistant.chips.length && !assistant.thinking && !assistant.done}
        <div class="replies">
          {#each assistant.chips as q (q)}<button class="chip" onclick={() => assistant.send(q)}>{q}</button>{/each}
        </div>
      {/if}
    </div>
  {/if}

  {#if noKey}
    <div class="field nokey">
      <span class="prompt">Connect an AI to ask for sounds in words</span>
      <button class="btn tonal" onclick={() => (app.view = { name: "settings", page: "ai" })}><Icon name="key" size={14} /> Add key</button>
    </div>
  {:else}
    <form class="field" onsubmit={send}>
      <input
        bind:this={input}
        bind:value={text}
        maxlength="500"
        placeholder={'Ask the AI: "make the vocals clearer", "less bass on this song"…'}
        aria-label="Ask the AI for a change"
        onfocus={() => (sound.askOpen = turns.length > 0 || sound.askOpen)}
      />
      <button class="btn primary" disabled={!text.trim() || assistant.thinking}>Send</button>
    </form>
  {/if}

  <div class="chips">
    {#each CHIPS as c (c.label)}
      <button class="chip" disabled={!!sound.busy} onclick={() => chip(c)}>{c.label}</button>
    {/each}
  </div>
</div>

<style>
  .ask {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .log {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-height: 260px;
    overflow-y: auto;
    padding: 10px 12px;
    border-radius: var(--r-sm);
    background: var(--surface-2);
    border: 1px solid var(--border);
  }
  .cap {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--faint);
  }
  .turn {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 10px;
    border-radius: var(--r-sm);
    background: var(--surface);
    border-left: 3px solid color-mix(in srgb, var(--primary) 60%, transparent);
  }
  .turn.me {
    border-left-color: var(--border);
    color: var(--muted);
  }
  .turn.err {
    border-left-color: var(--danger);
    color: var(--danger);
  }
  p {
    margin: 0;
    font-size: 13px;
    line-height: 1.45;
    white-space: pre-wrap;
    user-select: text;
  }
  .proposal {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    flex-wrap: wrap;
  }
  .what {
    display: flex;
    flex-direction: column;
    min-width: 0;
    font-size: 13px;
  }
  .mono {
    font-family: var(--font-mono);
    font-size: 11.5px;
  }
  .saved {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
  }
  .acts {
    display: flex;
    gap: 6px;
  }
  .acts .btn {
    height: 30px;
    padding: 0 14px;
    font-size: 12.5px;
  }
  .field {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px;
    border-radius: var(--r-md);
    background: var(--surface-2);
    border: 1.5px solid color-mix(in srgb, var(--primary) 70%, var(--border));
  }
  .field:focus-within {
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--primary) 25%, transparent);
  }
  .field.nokey {
    border-color: var(--border);
    padding-left: 14px;
  }
  .prompt {
    flex: 1;
    font-size: 13px;
    color: var(--muted);
  }
  .field input {
    flex: 1;
    min-width: 0;
    height: 34px;
    padding: 0 8px;
    border: 0;
    outline: none;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 13.5px;
  }
  .field .btn {
    height: 32px;
    padding: 0 16px;
  }
  .chips,
  .replies {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .chip {
    height: 28px;
    padding: 0 12px;
    border-radius: var(--r-pill);
    font-size: 12px;
    color: var(--muted);
    background: var(--glass-strong);
    border: 1px solid var(--glass-border);
  }
  .chip:hover:not(:disabled) {
    color: var(--text);
    border-color: color-mix(in srgb, var(--primary) 50%, var(--glass-border));
  }
  .link {
    align-self: flex-start;
    font-size: 12.5px;
    color: var(--primary);
  }
  .typing {
    display: flex;
    gap: 4px;
    padding: 6px 4px;
  }
  .typing i {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--muted);
    animation: bounce calc(1.1s / var(--motion-speed, 1)) infinite ease-in-out;
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
      opacity: 0.4;
    }
    30% {
      opacity: 1;
    }
  }
</style>
