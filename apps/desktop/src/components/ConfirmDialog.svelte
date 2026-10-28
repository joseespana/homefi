<script lang="ts">
  // Asks before anything destructive (delete, remove, log out): a stray click must never do it.
  // Esc / backdrop = cancel, Enter = confirm (unless Cancel has the focus).
  import { dialog, fade } from "../lib/motion";
  import { untrack } from "svelte";
  import { app } from "../lib/state.svelte";
  import Icon from "./Icon.svelte";

  const d = $derived(app.confirmDialog!);
  let card = $state<HTMLElement>();
  let cancelBtn = $state<HTMLButtonElement>();
  let okBtn = $state<HTMLButtonElement>();
  let checkBox = $state<HTMLInputElement>();

  // destructive: Cancel takes the focus, so Enter / Space back out; the focus goes back where it was
  $effect(() => {
    const before = document.activeElement as HTMLElement | null;
    untrack(() => (d.danger ? cancelBtn : okBtn))?.focus();
    return () => before?.focus?.();
  });

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      app.answerConfirm(false);
    } else if (e.key === "Enter" && document.activeElement !== cancelBtn && document.activeElement !== checkBox) {
      e.preventDefault();
      e.stopPropagation();
      app.answerConfirm(true);
    } else if (e.key === "Tab" && card) {
      // keep the focus inside the dialog
      const buttons = [checkBox, cancelBtn, okBtn].filter((b) => !!b) as HTMLElement[];
      const i = buttons.indexOf(document.activeElement as HTMLButtonElement);
      e.preventDefault();
      buttons[(i + (e.shiftKey ? buttons.length - 1 : 1) + buttons.length) % buttons.length]?.focus();
    }
  }
</script>

<svelte:window onkeydowncapture={onKey} />

<div class="backdrop" role="presentation" transition:fade onclick={(e) => e.target === e.currentTarget && app.answerConfirm(false)}>
  <div class="dialog" transition:dialog bind:this={card} role="alertdialog" aria-modal="true" aria-labelledby="confirm-title" aria-describedby={d.text ? "confirm-text" : undefined}>
    {#if d.danger}<span class="mark" aria-hidden="true"><Icon name="alert" size={20} /></span>{/if}
    <h2 id="confirm-title">{d.title}</h2>
    {#if d.text}<p id="confirm-text" class="muted">{d.text}</p>{/if}
    {#if d.check && app.confirmDialog}
      <label class="check"><input type="checkbox" bind:this={checkBox} bind:checked={app.confirmDialog.checked} /> {d.check}</label>
    {/if}
    <div class="actions">
      <button type="button" class="btn tonal" bind:this={cancelBtn} onclick={() => app.answerConfirm(false)}>{d.cancelLabel ?? "Cancel"}</button>
      <button type="button" class="btn" class:primary={!d.danger} class:danger={d.danger} bind:this={okBtn} onclick={() => app.answerConfirm(true)}>
        {d.confirmLabel ?? "Confirm"}
      </button>
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
    backdrop-filter: blur(6px);
    animation: fade 0.15s ease;
  }
  .dialog {
    position: relative;
    width: min(400px, calc(100vw - 32px));
    padding: 24px 28px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: min(20px, var(--r-max));
    box-shadow: var(--shadow);
    animation: pop 0.18s cubic-bezier(0.2, 0.9, 0.3, 1.2);
  }
  .mark {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: 50%;
    color: var(--danger);
    background: color-mix(in srgb, var(--danger) 14%, transparent);
  }
  h2 {
    margin: 0;
    font-size: 18px;
    overflow-wrap: anywhere;
  }
  p {
    margin: 0;
    line-height: 1.45;
    overflow-wrap: anywhere;
  }
  .check {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    margin-top: 4px;
    font-size: 14px;
    line-height: 1.4;
  }
  .check input {
    flex: none;
    width: 16px;
    height: 16px;
    margin: 2px 0 0;
    accent-color: var(--primary-strong);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 10px;
  }
  .btn.danger {
    /* darkened: white text stays readable on the light red of the dark theme */
    background: color-mix(in srgb, var(--danger) 78%, black);
    color: #fff;
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: scale(0.96);
    }
  }
</style>
