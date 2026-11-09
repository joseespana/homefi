<script lang="ts">
  // The "not medical advice" notice, once before the first ride (when Build or Start is pressed).
  // Esc = not now (nothing is built); "I understand" remembers it.
  import { dialog, fade } from "../../lib/motion";
  import { exercise } from "../../lib/exercise.svelte";
  import Icon from "../Icon.svelte";

  let ok = $state<HTMLButtonElement>();
  $effect(() => {
    const before = document.activeElement as HTMLElement | null;
    ok?.focus();
    return () => before?.focus?.();
  });
  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      exercise.dismissNotice();
    }
  }
</script>

<svelte:window onkeydowncapture={onKey} />

<div class="backdrop" role="presentation" transition:fade onclick={(e) => e.target === e.currentTarget && exercise.dismissNotice()}>
  <div class="dialog" transition:dialog role="alertdialog" aria-modal="true" aria-labelledby="ex-health-title" aria-describedby="ex-health-text">
    <span class="mark" aria-hidden="true"><Icon name="shield" size={22} /></span>
    <h2 id="ex-health-title">Before your first ride</h2>
    <p id="ex-health-text">{exercise.noticeText}</p>
    <div class="actions">
      <button class="btn tonal" type="button" onclick={() => exercise.dismissNotice()}>Not now</button>
      <button class="btn primary" type="button" bind:this={ok} onclick={() => exercise.acceptNotice()}>I understand</button>
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
    background: rgb(5 10 18 / 0.6);
  }
  .dialog {
    width: min(460px, calc(100vw - 48px));
    padding: 24px;
    border-radius: var(--r-lg);
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
    color: var(--text);
  }
  .mark {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: 50%;
    background: var(--surface-3);
    color: var(--primary);
  }
  h2 {
    margin: 12px 0 8px;
  }
  p {
    margin: 0 0 20px;
    color: var(--muted);
    line-height: 1.55;
    font-size: 15px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }
  .btn {
    height: 44px;
    padding: 0 22px;
  }
</style>
