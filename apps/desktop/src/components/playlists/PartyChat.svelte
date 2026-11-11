<script lang="ts">
  // The optional talk with the user's own AI: it asks one short question per turn with a few
  // quick replies; tap one or type. Text only (never HTML). After the last turn the field is
  // replaced by a line, so there is no dead input.
  import { tick } from "svelte";
  import type { Conversation } from "../../lib/party.svelte";
  import { fade, rise } from "../../lib/motion";
  import Icon from "../Icon.svelte";

  let { chat, disabled = false }: { chat: Conversation; disabled?: boolean } = $props();

  let text = $state("");
  let input = $state<HTMLInputElement | null>(null);
  let log = $state<HTMLElement | null>(null);

  const closed = $derived(chat.chatClosed && !chat.thinking);
  const busy = $derived(disabled || chat.thinking);

  $effect(() => {
    chat.messages.length;
    chat.thinking;
    tick().then(() => log?.scrollTo({ top: log.scrollHeight }));
  });

  // a message the AI couldn't answer comes back to the field, ready to send again
  $effect(() => {
    const back = chat.retryText;
    if (back == null) return;
    text = back;
    chat.retryText = null;
    tick().then(() => input?.focus());
  });

  function send(e?: Event) {
    e?.preventDefault();
    const t = text.trim();
    if (!t || busy) return;
    text = "";
    chat.send(t).then(() => tick()).then(() => input?.focus());
  }
</script>

<section class="chat" aria-label="Chat with your AI">
  <div class="log" role="log" bind:this={log} aria-live="polite" in:rise={{ y: 6, duration: 180 }}>
    {#each chat.messages as m, i (i)}
      <div class="row" class:me={m.fromUser}>
        {#if !m.fromUser}<span class="avatar" aria-hidden="true"><Icon name="ai" size={15} /></span>{/if}
        <p class="bubble" class:me={m.fromUser}>{m.text}</p>
      </div>
    {/each}
    {#if chat.thinking}
      <div class="row">
        <span class="avatar" aria-hidden="true"><Icon name="ai" size={15} /></span>
        <div class="bubble typing" role="status" aria-label="Thinking" in:fade><i></i><i></i><i></i></div>
      </div>
    {/if}
    {#if chat.quickReplies.length && !busy && !closed}
      <div class="replies">
        {#each chat.quickReplies as q (q)}<button type="button" class="chip" onclick={() => chat.send(q)}>{q}</button>{/each}
      </div>
    {/if}
  </div>

  <div class="foot">
    {#if closed}
      <p class="muted closing">Build the playlist when you're ready.</p>
    {:else}
      <form class="field" onsubmit={send}>
        <input bind:this={input} bind:value={text} maxlength="500" placeholder="Type an answer or tap one" aria-label="Your answer" disabled={disabled} autocomplete="off" />
        <span class="count" title="Answers so far">{chat.userTurns}/{chat.maxTurns}</span>
        <button class="send" aria-label="Send" title="Send" disabled={!text.trim() || busy}><Icon name="send" size={16} /></button>
      </form>
    {/if}
  </div>
</section>

<style>
  /* one card: the conversation, and the field docked at its foot */
  .chat {
    display: flex;
    flex-direction: column;
    min-width: 0;
    border-radius: var(--r-lg);
    background: color-mix(in srgb, var(--surface) 72%, transparent);
    border: 1px solid var(--glass-border);
    box-shadow: 0 18px 40px -28px rgb(0 0 0 / 0.6);
    backdrop-filter: blur(18px);
    -webkit-backdrop-filter: blur(18px);
    overflow: hidden;
  }
  .log {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 180px;
    max-height: min(440px, 52vh);
    overflow-y: auto;
    padding: 18px 18px 12px;
  }
  .foot {
    padding: 10px;
    border-top: 1px solid var(--glass-border);
  }
  .row {
    display: flex;
    align-items: flex-end;
    gap: 8px;
  }
  .row.me {
    justify-content: flex-end;
  }
  .avatar {
    flex: none;
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    color: var(--primary);
    background: color-mix(in srgb, var(--primary) 16%, var(--surface));
  }
  .bubble {
    margin: 0;
    max-width: 75%;
    padding: 8px 12px;
    border-radius: var(--r-md) var(--r-md) var(--r-md) var(--r-xs);
    background: var(--surface);
    border: 1px solid var(--border);
    font-size: 13.5px;
    line-height: 1.45;
    white-space: pre-wrap;
    user-select: text;
  }
  .bubble.me {
    border-radius: var(--r-md) var(--r-md) var(--r-xs) var(--r-md);
    background: var(--primary);
    border-color: var(--primary);
    color: var(--on-primary);
  }
  .replies {
    display: flex;
    padding-left: 34px;
    flex-wrap: wrap;
    gap: 8px;
  }
  .chip {
    height: 28px;
    padding: 0 12px;
    border-radius: var(--r-pill);
    font-size: 12.5px;
    color: var(--muted);
    background: var(--glass-strong);
    border: 1px solid var(--glass-border);
  }
  .chip:hover:not(:disabled) {
    color: var(--text);
    border-color: color-mix(in srgb, var(--primary) 50%, var(--glass-border));
  }
  .field {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 4px 4px 12px;
    border-radius: var(--r-md);
    background: var(--surface-2);
    border: 1px solid var(--border);
    transition: border-color 0.15s, box-shadow 0.15s;
  }
  .field:focus-within {
    border-color: color-mix(in srgb, var(--primary) 70%, var(--border));
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--primary) 25%, transparent);
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
  .send {
    flex: none;
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: var(--r-sm);
    background: var(--primary);
    color: var(--on-primary);
  }
  .send:disabled {
    opacity: 0.45;
  }
  .count {
    flex: none;
    font: 11px var(--font-mono);
    color: var(--faint);
  }
  .closing {
    margin: 0;
    padding: 8px 6px;
    text-align: center;
  }
  .typing {
    display: flex;
    gap: 4px;
    padding: 12px;
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
