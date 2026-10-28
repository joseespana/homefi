<script lang="ts">
  import { nearEnd, type Paged } from "../lib/paged.svelte";
  import Icon from "./Icon.svelte";

  /** End of a paged list: loads the next page when it scrolls into view; a spinner while it comes, retry on failure. */
  let { list }: { list: Paged } = $props();
</script>

{#if list.error && !list.loading}
  <div class="more error" role="alert">
    <span>Couldn't load more: {list.error}</span>
    <button class="btn tonal" onclick={() => list.more()}><Icon name="refresh" size={14} /> Retry</button>
  </div>
{:else if !list.done}
  <!-- re-created after every page, so a sentinel still on screen asks for the next one -->
  {#key list.items.length}
    <div class="more" use:nearEnd={() => list.more()}>
      {#if list.loading}<span class="spin" aria-hidden="true"></span> Loading more…{:else}<span class="muted">{list.items.length} of {list.total}</span>{/if}
    </div>
  {/key}
{/if}

<style>
  .more {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    min-height: 48px;
    font-size: 13px;
    color: var(--muted);
    grid-column: 1 / -1;
  }
  .error {
    color: var(--danger);
  }
  .spin {
    width: 14px;
    height: 14px;
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
</style>
