<script lang="ts">
  // Release notes, markdown-lite (headings, bullets, **bold**, `code`), as plain text.
  import type { Block } from "../lib/notes";

  let { blocks }: { blocks: Block[] } = $props();
</script>

<div class="notes">
  {#each blocks as b, i (i)}
    {#if b.kind === "h"}
      <h4>{#each b.parts as p}{#if p.bold}<b>{p.text}</b>{:else if p.code}<code>{p.text}</code>{:else}{p.text}{/if}{/each}</h4>
    {:else if b.kind === "li"}
      <p class="li">{#each b.parts as p}{#if p.bold}<b>{p.text}</b>{:else if p.code}<code>{p.text}</code>{:else}{p.text}{/if}{/each}</p>
    {:else}
      <p>{#each b.parts as p}{#if p.bold}<b>{p.text}</b>{:else if p.code}<code>{p.text}</code>{:else}{p.text}{/if}{/each}</p>
    {/if}
  {/each}
</div>

<style>
  .notes {
    font-size: 12.5px;
    line-height: 1.5;
  }
  h4 {
    margin: 8px 0 2px;
    font-size: 12.5px;
  }
  h4:first-child {
    margin-top: 0;
  }
  p {
    margin: 2px 0;
  }
  .li {
    position: relative;
    padding-left: 14px;
  }
  .li::before {
    content: "•";
    position: absolute;
    left: 2px;
    color: var(--primary);
  }
  code {
    font: 12px var(--font-mono);
    padding: 0 3px;
    border-radius: 4px;
    background: var(--surface);
  }
</style>
