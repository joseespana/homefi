<script lang="ts">
  // The optional talk with the user's own AI: it asks one short question per turn with a few
  // quick replies; tap one or type. Text only (never HTML). After the last turn the field is
  // replaced by a line, so there is no dead input.
  import { tick } from "svelte";
  import { party } from "../../lib/party.svelte";
  import { fade, rise } from "../../lib/motion";
  import Icon from "../Icon.svelte";

  let { disabled = false }: { disabled?: boolean } = $props();

  let text = $state("");
  let input = $state<HTMLInputElement | null>(null);
  let log = $state<HTMLElement | null>(null);

  const closed = $derived(party.chatClosed && !party.thinking);
  const busy = $derived(disabled || party.thinking);

  $effect(() => {
    party.messages.length;
    party.thinking;
    tick().then(() => log?.scrollTo({ top: log.scrollHeight }));
  });

  function send(e?: Event) {
    e?.preventDefault();
    const t = text.trim();
    if (!t || busy) return;
    text = "";
    party.send(t).then(() => tick()).then(() => input?.focus());
  }
</script>

<section class="chat" aria-label="Chat with your AI">
  <div class="log" role="log" bind:this={log} aria-live="polite" in:rise={{ y: 6, duration: 180 }}>
    {#each party.messages as m, i (i)}
      <div class="row" class:me={m.fromUser}>
        {#if !m.fromUser}<span class="avatar" aria-hidden="true"><Icon name="ai" size={15} /></span>{/if}
        <p class="bubble" class:me={m.fromUser}>{m.text}</p>
      </div>
    {/each}
    {#if party.thinking}
      <div class="row">
        <span class="avatar" aria-hidden="true"><Icon name="ai" size={15} /></span>
        <div class="bubble typing" role="status" aria-label="Thinking" in:fade><i></i><i></i><i></i></div>
      </div>
    {/if}
    {#if party.quickReplies.length && !busy && !closed}
      <div class="replies">
        {#each party.quickReplies as q (q)}<button type="button" class="chip" onclick={() => party.send(q)}>{q}</button>{/each}
      </div>
    {/if}
  </div>

  {#if closed}
    <p class="muted closing">Build the playlist when you're ready.</p>
  {:else}
    <form class="field" onsubmit={send}>
      <input bind:this={input} bind:value={text} maxlength="500" placeholder="Type an answer or tap one" aria-label="Your answer" disabled={disabled} autocomplete="off" />
      <button class="send" aria-label="Send" title="Send" disabled={!text.trim() || busy}><Icon name="send" size={16} /></button>
    </form>
    <span class="muted count">{party.userTurns} of {party.maxTurns} answers</span>
  {/if}
</section>

<style>
  .chat {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .log {
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-height: 360px;
    overflow-y: auto;
    padding: 12px;
    border-radius: min(16px, var(--r-max));
    background: var(--surface-2);
    border: 1px solid var(--border);
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
    border-radius: min(16px, var(--r-max)) min(16px, var(--r-max)) min(16px, var(--r-max)) min(4px, var(--r-max));
    background: var(--surface);
    border: 1px solid var(--border);
    font-size: 13.5px;
    line-height: 1.45;
    white-space: pre-wrap;
    user-select: text;
  }
  .bubble.me {
    border-radius: min(16px, var(--r-max)) min(16px, var(--r-max)) min(4px, var(--r-max)) min(16px, var(--r-max));
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
    border-radius: min(999px, var(--r-max));
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
    padding: 5px 5px 5px 14px;
    border-radius: min(999px, var(--r-max));
    background: var(--surface-2);
    border: 1.5px solid color-mix(in srgb, var(--primary) 70%, var(--border));
  }
  .field:focus-within {
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
    border-radius: 50%;
    background: var(--primary);
    color: var(--on-primary);
  }
  .send:disabled {
    opacity: 0.45;
  }
  .count {
    font-size: 11.5px;
    opacity: 0.8;
    padding-left: 6px;
  }
  .closing {
    margin: 0;
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
