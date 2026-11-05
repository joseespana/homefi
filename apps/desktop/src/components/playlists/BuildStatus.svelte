<script lang="ts">
  // What a running build is doing ("Finding the songs: 9 of 21"), with a bar while it matches
  // songs, and Cancel. Shown wherever a build or a rearrange runs. By default it follows the
  // concert store; the party screen passes its own `active`, `text`, `fraction` and `oncancel`.
  import { playlists } from "../../lib/playlists.svelte";

  let {
    active,
    text: given,
    fraction: givenFraction,
    oncancel,
  }: {
    active?: boolean;
    text?: string;
    /** 0..1, or null for the indeterminate bar. */
    fraction?: number | null;
    oncancel?: () => void;
  } = $props();

  const p = $derived(playlists.progress);
  const provider = $derived(playlists.sources?.ai?.provider_name ?? "your AI");
  const concertText = $derived(
    !p
      ? "Starting…"
      : p.kind === "searching"
        ? p.with_ai
          ? `Looking up recent setlists with ${provider}…`
          : "Getting the most played on Tidal…"
        : p.kind === "shows_found"
          ? `Found ${p.count} shows`
          : `Finding the songs: ${p.done} of ${p.total}`,
  );
  const concertFraction = $derived(p?.kind === "resolving" && p.total > 0 ? p.done / p.total : null);
  const shown = $derived(active ?? playlists.building);
  const text = $derived(given ?? concertText);
  const fraction = $derived(givenFraction !== undefined ? givenFraction : concertFraction);
</script>

{#if shown}
  <div class="status" role="status" aria-live="polite">
    <div class="line">
      <span>{text}</span>
      <button type="button" class="btn tonal" onclick={() => (oncancel ?? (() => playlists.cancel()))()}>Cancel</button>
    </div>
    {#if fraction !== null}
      <div class="bar" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(fraction * 100)}><i style:width="{fraction * 100}%"></i></div>
    {:else}
      <div class="bar indeterminate" aria-hidden="true"><i></i></div>
    {/if}
  </div>
{/if}

<style>
  .status {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 16px;
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    background: var(--surface-2);
  }
  .line {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .bar {
    height: 6px;
    overflow: hidden;
    border-radius: var(--r-pill);
    background: var(--surface-3);
  }
  .bar i {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--primary-strong);
    transition: width 0.2s ease;
  }
  .indeterminate i {
    width: 35%;
    animation: slide 1.4s ease-in-out infinite;
  }
  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(300%);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .indeterminate i {
      animation: none;
      width: 100%;
    }
  }
</style>
