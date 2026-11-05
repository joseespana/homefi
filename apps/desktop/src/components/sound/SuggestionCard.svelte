<script lang="ts">
  // A proactive suggestion: one sound for what you listen to most (the AI's
  // words when set up, else the guided offer). The card only appears once the
  // suggestion is ready, and goes away if it takes longer than WAIT_MS.
  // Tapping an answer goes on in the assistant; nothing changes before a yes.
  import { onMount } from "svelte";
  import { api, type AssistantTurn } from "../../lib/api";
  import { assistant } from "../../lib/assistant.svelte";
  import { uiLanguage } from "../../lib/eq.svelte";
  import { rise } from "../../lib/motion";
  import { sound } from "../../lib/sound.svelte";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";

  /** How long a suggestion may take before the card gives up. */
  const WAIT_MS = 25_000;
  const HIDDEN_KEY = "homefi.suggestion.hidden";

  let turn = $state.raw<AssistantTurn | null>(null);
  let hidden = $state(sessionStorage.getItem(HIDDEN_KEY) === "1");

  onMount(() => {
    if (hidden || !sound.ui.suggestions) return;
    let alive = true;
    const giveUp = setTimeout(() => (alive = false), WAIT_MS);
    api
      .tasteSuggest(uiLanguage())
      .then((t) => alive && (turn = t))
      .catch(() => (alive = false))
      .finally(() => clearTimeout(giveUp));
    return () => {
      alive = false;
      clearTimeout(giveUp);
    };
  });

  function answer(reply: string) {
    if (!turn) return;
    assistant.resume(turn, reply);
    app.view = { name: "assistant" };
  }
  function hide() {
    hidden = true;
    sessionStorage.setItem(HIDDEN_KEY, "1");
  }
</script>

{#if sound.ui.suggestions && !hidden && turn}
  <section class="card" in:rise aria-live="polite">
    <span class="mark" aria-hidden="true"><Icon name="ai" size={16} /></span>
    <div class="body">
      <span class="cap">Suggestion</span>
      <p>{turn.text}</p>
      {#if !turn.done && turn.quick_replies.length}
        <div class="replies">
          {#each turn.quick_replies as q, i (q)}
            <button class="btn {i === 0 ? 'primary' : 'tonal'} small" onclick={() => answer(q)}>{q}</button>
          {/each}
        </div>
      {/if}
    </div>
    <button class="icon-btn mini" title="Not now" aria-label="Hide the suggestion" onclick={hide}><Icon name="x" size={14} /></button>
  </section>
{/if}

<style>
  .card {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 14px 16px;
    border-radius: var(--r-md);
    background: linear-gradient(135deg, color-mix(in srgb, var(--primary) 14%, var(--surface)), var(--glass));
    border: 1px solid color-mix(in srgb, var(--primary) 35%, var(--glass-border));
  }
  .mark {
    flex: none;
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    color: var(--accent-text);
    background: color-mix(in srgb, var(--primary) 18%, transparent);
  }
  .body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .cap {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--faint);
  }
  p {
    margin: 0;
    font-size: 13.5px;
    line-height: 1.45;
  }
  .replies {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .btn.small {
    height: 30px;
    padding: 0 12px;
  }
  .icon-btn.mini {
    width: 26px;
    height: 26px;
  }
</style>
