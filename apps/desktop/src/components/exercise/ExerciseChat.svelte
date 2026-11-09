<script lang="ts">
  // "Talk it through with the AI": a short conversation that shapes the request (style, length, genres).
  // The AI's words and quick replies come from the core; every turn hands back the request as it now
  // understands it, and the setup behind this panel shows it.
  import { tick } from "svelte";
  import { exercise } from "../../lib/exercise.svelte";
  import Icon from "../Icon.svelte";

  let text = $state("");
  let list = $state<HTMLElement>();

  $effect(() => {
    exercise.chat.length;
    exercise.chatBusy;
    tick().then(() => list?.scrollTo({ top: list.scrollHeight }));
  });
  function send() {
    const said = text;
    text = "";
    exercise.say(said);
  }
</script>

<div class="chat" role="region" aria-label="Talk it through with the AI">
  <div class="msgs" bind:this={list}>
    {#each exercise.chat as m}
      <p class="msg" class:me={m.who === "me"}>{m.text}</p>
    {/each}
    {#if exercise.chatBusy}<p class="msg busy" aria-label="The AI is thinking">…</p>{/if}
  </div>
  {#if exercise.chatError}<p class="err" role="alert">{exercise.chatError}</p>{/if}
  {#if exercise.chatQuick.length && !exercise.chatBusy}
    <div class="quick">
      {#each exercise.chatQuick as q}<button type="button" class="chip" onclick={() => exercise.say(q)}>{q}</button>{/each}
    </div>
  {/if}
  <form class="say" onsubmit={(e) => (e.preventDefault(), send())}>
    <input bind:value={text} placeholder="Say it in your own words" aria-label="Your answer" disabled={exercise.chatBusy} autocomplete="off" />
    <button type="submit" class="btn primary" disabled={exercise.chatBusy || !text.trim()} aria-label="Send"><Icon name="send" size={16} /></button>
    <button type="button" class="btn tonal" onclick={() => exercise.closeChat()}>Done</button>
  </form>
</div>

<style>
  .chat {
    margin-top: 10px;
    padding: 12px;
    border-radius: var(--r-md);
    border: 1px solid var(--border);
    background: var(--surface-2);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .msgs {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-height: 200px;
    overflow-y: auto;
  }
  .msg {
    margin: 0;
    max-width: 85%;
    padding: 8px 12px;
    border-radius: var(--r-md);
    background: var(--surface-3);
    line-height: 1.4;
    user-select: text;
  }
  .msg.me {
    align-self: flex-end;
    background: var(--primary);
    color: var(--on-primary);
  }
  .busy {
    color: var(--muted);
  }
  .err {
    margin: 0;
    color: var(--danger);
    font-size: 13px;
  }
  .quick {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .chip {
    min-height: 36px;
    padding: 0 14px;
    border-radius: var(--r-pill);
    border: 1px solid var(--border);
    background: var(--surface);
    font-weight: 600;
  }
  .chip:hover {
    background: var(--surface-3);
  }
  .say {
    display: flex;
    gap: 8px;
  }
  .say input {
    flex: 1;
    min-width: 0;
    height: 44px;
    user-select: text;
  }
  .btn {
    height: 44px;
    min-width: 44px;
    justify-content: center;
  }
</style>
