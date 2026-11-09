<script lang="ts">
  // Genres for today, several at once. "Any" is what nothing picked means (picking one clears it); "+" adds
  // a genre of your own. A seventh is refused with a note (the core keeps six).
  import { tick } from "svelte";
  import { EX_MAX_GENRES, EX_MAX_GENRE_CHARS, GENRES } from "../../lib/exercise-api";
  import Icon from "../Icon.svelte";

  let { picked, ontoggle, onclear }: { picked: readonly string[]; ontoggle: (genre: string) => void; onclear: () => void } = $props();

  const isOn = (g: string) => picked.some((p) => p.toLowerCase() === g.toLowerCase());
  const own = $derived(picked.filter((p) => !GENRES.some((g) => g.toLowerCase() === p.toLowerCase())));
  const full = $derived(picked.length >= EX_MAX_GENRES);
  let adding = $state(false);
  let draft = $state("");
  let field = $state<HTMLInputElement>();

  async function openAdd() {
    adding = true;
    await tick();
    field?.focus();
  }
  function commit() {
    const g = draft.trim();
    if (g && !isOn(g)) ontoggle(g);
    draft = "";
    adding = false;
  }
</script>

<div class="chips" role="group" aria-label="Genres for today">
  {#each GENRES as g}
    <button type="button" class="chip" aria-pressed={isOn(g)} onclick={() => ontoggle(g)}>{g}</button>
  {/each}
  {#each own as g}
    <button type="button" class="chip" aria-pressed="true" onclick={() => ontoggle(g)}>{g}</button>
  {/each}
  <button type="button" class="chip any" aria-pressed={picked.length === 0} onclick={onclear}>Any</button>
  {#if adding}
    <input bind:this={field} bind:value={draft} class="add" maxlength={EX_MAX_GENRE_CHARS} placeholder="Your genre" aria-label="Another genre" onblur={commit} onkeydown={(e) => (e.key === "Enter" ? commit() : e.key === "Escape" && ((draft = ""), (adding = false)))} />
  {:else}
    <button type="button" class="chip plus" aria-label="Add another genre" title="Add another genre" onclick={openAdd}><Icon name="plus" size={16} /></button>
  {/if}
</div>
{#if full}<p class="limit" role="status">Pick up to {EX_MAX_GENRES}.</p>{/if}

<style>
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .chip {
    min-height: 40px;
    padding: 0 16px;
    border-radius: var(--r-pill);
    border: 1px solid var(--border);
    background: var(--surface-2);
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
    transition: background 0.15s;
  }
  .chip:hover:not([aria-pressed="true"]) {
    background: var(--surface-3);
  }
  .chip[aria-pressed="true"] {
    background: var(--primary);
    color: var(--on-primary);
    border-color: transparent;
  }
  .chip[aria-pressed="true"]::before {
    content: "\2713";
    font-weight: 800;
  }
  .plus {
    width: 40px;
    padding: 0;
    justify-content: center;
  }
  .add {
    height: 40px;
    width: 150px;
    border-radius: var(--r-pill);
    padding: 0 14px;
  }
  .limit {
    margin: 8px 0 0;
    font-size: 12px;
    color: var(--warn);
  }
</style>
