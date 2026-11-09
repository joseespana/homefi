<script lang="ts">
  // A card of the Exercise screens: a small caps title and its content.
  import type { Snippet } from "svelte";
  let { title, children, aside }: { title?: string; children: Snippet; aside?: Snippet } = $props();
</script>

<section class="card">
  {#if title}
    <header>
      <h3>{title}</h3>
      {#if aside}<span class="aside">{@render aside()}</span>{/if}
    </header>
  {/if}
  {@render children()}
</section>

<style>
  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
    padding: 16px;
    min-width: 0;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-bottom: 10px;
  }
  h3 {
    margin: 0;
    font: 700 13px var(--font);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
  }
  .aside {
    font-size: 12px;
    color: var(--muted);
  }
</style>
