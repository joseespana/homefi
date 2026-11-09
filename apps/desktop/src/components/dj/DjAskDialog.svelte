<script lang="ts">
  // A song picked anywhere in the app while DJ mode is on and "Ask which deck" is the setting.
  import { dj } from "../../lib/dj.svelte";

  const pick = $derived(dj.asking);
  let first = $state<HTMLButtonElement>();
  $effect(() => {
    const before = document.activeElement as HTMLElement | null;
    first?.focus();
    return () => before?.focus?.();
  });
</script>

<svelte:window onkeydowncapture={(e) => e.key === "Escape" && (e.stopPropagation(), dj.answerAsk(null))} />

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && dj.answerAsk(null)}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="dj-ask-title">
    <h2 id="dj-ask-title">Where does “{pick?.title}” go?</h2>
    <p class="muted">DJ mode is on, so songs go to a deck instead of the player.</p>
    <div class="actions">
      <button class="btn deck a" type="button" bind:this={first} onclick={() => dj.answerAsk("a")}>Deck A</button>
      <button class="btn deck b" type="button" onclick={() => dj.answerAsk("b")}>Deck B</button>
      <button class="btn tonal" type="button" onclick={() => dj.answerAsk("crate")}>The crate</button>
      <button class="btn tonal" type="button" onclick={() => dj.answerAsk(null)}>Cancel</button>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: grid;
    place-items: center;
    background: rgb(4 10 18 / 0.6);
  }
  .dialog {
    width: min(440px, calc(100vw - 32px));
    padding: 22px 26px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
    box-shadow: var(--shadow);
  }
  h2,
  p {
    margin: 0;
  }
  h2 {
    font-size: 17px;
    overflow-wrap: anywhere;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 6px;
  }
  .btn {
    height: 40px;
  }
  .deck.a {
    background: var(--dj-a);
    color: var(--dj-a-ink);
  }
  .deck.b {
    background: var(--dj-b);
    color: var(--dj-b-ink);
  }
</style>
