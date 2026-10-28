<script lang="ts">
  // Floating notes, bottom-right above the deck: loading, done, errors.
  import { reflow, toast } from "../lib/motion";
  import { app } from "../lib/state.svelte";
  import Icon from "./Icon.svelte";

  const icons = { info: "music", success: "check", error: "alert", loading: "" } as const;
</script>

<div class="stack" aria-live="polite">
  {#each app.notes as n (n.id)}
    <div class="note {n.kind}" style:border-left-color={n.color} in:toast out:toast={{ x: 60 }} animate:reflow role={n.kind === "error" ? "alert" : "status"}>
      <span class="icon">
        {#if n.kind === "loading"}<span class="spin" aria-hidden="true"></span>{:else if n.color}<span class="who" style:background={n.color}></span>{:else}<Icon name={icons[n.kind]} size={15} />{/if}
      </span>
      <span class="text">{n.text}</span>
      <button class="close" aria-label="Dismiss" onclick={() => app.dismiss(n.id)}><Icon name="x" size={14} /></button>
    </div>
  {/each}
</div>

<style>
  .stack {
    position: fixed;
    right: 18px;
    bottom: 114px;
    z-index: 60;
    width: min(360px, calc(100vw - 36px));
    display: flex;
    flex-direction: column;
    gap: 8px;
    pointer-events: none;
  }
  .note {
    pointer-events: auto;
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 10px 10px 10px 12px;
    border-radius: min(14px, var(--r-max));
    background: color-mix(in srgb, var(--surface) 88%, transparent);
    border: 1px solid var(--border);
    border-left: 3px solid var(--room-accent);
    box-shadow: var(--shadow);
    backdrop-filter: blur(18px) saturate(1.3);
    font-size: 13px;
    line-height: 1.35;
    animation: slide-in 0.22s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .note.success {
    border-left-color: var(--ok);
  }
  .note.error {
    border-left-color: var(--danger);
  }
  .icon {
    flex: none;
    width: 18px;
    height: 18px;
    display: grid;
    place-items: center;
    margin-top: 1px;
    color: var(--accent-text);
  }
  .success .icon {
    color: var(--ok);
  }
  .error .icon {
    color: var(--danger);
  }
  .text {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
    /* long errors: three lines at most */
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .close {
    flex: none;
    width: 22px;
    height: 22px;
    display: grid;
    place-items: center;
    border-radius: min(6px, var(--r-max));
    color: var(--muted);
  }
  .close:hover {
    color: var(--text);
    background: var(--glass-strong);
  }
  /* someone's colour (shared control notices) */
  .who {
    width: 10px;
    height: 10px;
    border-radius: 50%;
  }
  .spin {
    width: 13px;
    height: 13px;
    border-radius: 50%;
    border: 2px solid color-mix(in srgb, currentColor 30%, transparent);
    border-top-color: currentColor;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @keyframes slide-in {
    from {
      opacity: 0;
      transform: translateX(16px);
    }
  }
</style>
