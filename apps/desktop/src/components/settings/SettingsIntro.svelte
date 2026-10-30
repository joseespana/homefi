<script lang="ts">
  // What a page is about, in a sentence or two, above its groups.
  import type { Snippet } from "svelte";

  let { children }: { children: Snippet } = $props();
</script>

<p class="intro">{@render children()}</p>

<style>
  .intro {
    grid-column: 1 / -1;
    margin: 0;
    max-width: 65ch;
    font: 400 14px/1.5 var(--font);
    color: var(--muted);
  }
</style>
