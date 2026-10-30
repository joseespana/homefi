<script lang="ts">
  // The search found nothing: say so and offer the way out.
  import Icon from "../Icon.svelte";
  import { settingsSearch } from "./search.svelte";
</script>

<div class="empty" role="status">
  <Icon name="search" size={28} />
  <p>Nothing matches “{settingsSearch.query.trim()}”</p>
  <button type="button" class="btn tonal" onclick={() => settingsSearch.clear()}>Clear search</button>
</div>

<style>
  .empty {
    grid-column: 1 / -1;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--s-3);
    padding: var(--s-7) var(--s-4);
    color: var(--muted);
    text-align: center;
  }
  p {
    margin: 0;
    font: var(--set-label-font);
    overflow-wrap: anywhere;
  }
</style>
