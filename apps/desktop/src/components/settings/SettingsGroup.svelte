<script lang="ts">
  // A card of related rows, with its caption above it. Rows are separated by
  // hairlines; in the two-column flow of very wide windows `span={2}` takes both.
  import { setContext, type Snippet } from "svelte";
  import { GROUP_KEY, settingsSearch, type GroupContext } from "./search.svelte";

  let { title, description, span = 1, children }: { title?: string; description?: string; span?: 1 | 2; children: Snippet } = $props();

  const uid = $props.id();
  const matched = $derived(!!title && settingsSearch.matches(`${title} ${description ?? ""}`));
  const context: GroupContext = {
    get matched() {
      return settingsSearch.active && matched;
    },
  };
  setContext(GROUP_KEY, context);
</script>

<section class="group" class:searching={settingsSearch.active} data-span={span} aria-labelledby={title ? `${uid}-t` : undefined}>
  {#if title}
    <header>
      <h3 id="{uid}-t">{title}</h3>
      {#if description}<p>{description}</p>{/if}
    </header>
  {/if}
  <div class="card">{@render children()}</div>
</section>

<style>
  .group {
    min-width: 0;
    container: group / inline-size;
  }
  .group[data-span="2"] {
    grid-column: 1 / -1;
  }
  /* searching: a group with nothing left to show goes away */
  .group.searching:not(:has(.card > :not([hidden]))) {
    display: none;
  }
  header {
    margin: 0 var(--s-1) var(--s-2);
  }
  h3 {
    display: block;
    margin: 0;
    padding: var(--set-group-bar-pad);
    border-bottom: var(--set-group-rule);
    width: fit-content;
    max-width: 100%;
    background: var(--set-group-bar);
    font: var(--set-group-font);
    letter-spacing: var(--set-group-spacing);
    text-transform: var(--set-group-transform);
    color: var(--set-group-color);
  }
  header p {
    margin: var(--s-1) 0 0;
    max-width: 65ch;
    font: var(--set-hint-font);
    color: var(--muted);
  }
  .card {
    background: var(--set-card-bg);
    border: 1px solid var(--set-card-border);
    border-radius: var(--set-card-r);
    box-shadow: var(--set-card-shadow);
    backdrop-filter: var(--set-card-blur);
    overflow: clip;
  }
  .card > :global([hidden]) {
    display: none;
  }
  .card > :global(:not([hidden]) ~ :not([hidden])) {
    border-top: 1px solid var(--set-divider);
  }
</style>
