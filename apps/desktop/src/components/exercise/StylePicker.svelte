<script lang="ts">
  // The four ride styles as radio cards, each with a small drawing of its course at the chosen length.
  import { PRESET_IDS, STYLE_INFO, sketchOf, type PresetId } from "../../lib/exercise-api";
  import { roving } from "../settings/roving";
  import Mountain from "./Mountain.svelte";

  let {
    value,
    minutes,
    onchange,
    recommended = null,
    disabled = false,
  }: { value: PresetId; minutes: number; onchange: (id: PresetId) => void; recommended?: PresetId | null; disabled?: boolean } = $props();
</script>

<div class="styles" role="radiogroup" aria-label="Ride style" use:roving={{ selector: "[role=radio]:not(:disabled)", vertical: false }}>
  {#each PRESET_IDS as id (id)}
    {@const sketch = sketchOf(id, minutes)}
    <button type="button" role="radio" class="style" aria-checked={value === id} tabindex={value === id ? 0 : -1} {disabled} onclick={() => onchange(id)}>
      <span class="draw"><Mountain parts={sketch.parts} profile={sketch.profile} mode="summary" height={44} label="" /></span>
      <b>{STYLE_INFO[id].name}</b>
      <span class="line">{STYLE_INFO[id].line}</span>
      {#if recommended === id}<span class="tag">Good for your first time</span>{/if}
    </button>
  {/each}
</div>

<style>
  .styles {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 10px;
  }
  .style {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 10px;
    text-align: left;
    border: 2px solid var(--border);
    border-radius: var(--r-md);
    background: var(--surface-2);
    transition: background 0.15s, border-color 0.15s;
    min-width: 0;
  }
  .style:hover:not(:disabled):not([aria-checked="true"]) {
    background: var(--surface-3);
  }
  .style[aria-checked="true"] {
    border-color: var(--primary);
    background: var(--surface-3);
  }
  .style:disabled {
    opacity: 0.5;
  }
  .draw {
    display: block;
    height: 44px;
    margin-bottom: 2px;
    pointer-events: none;
  }
  b {
    font-size: 15px;
  }
  .line {
    font-size: 12px;
    color: var(--muted);
  }
  .tag {
    align-self: flex-start;
    margin-top: 2px;
    padding: 1px 8px;
    border-radius: var(--r-pill);
    background: rgb(74 222 128 / 0.16);
    color: var(--ok);
    font-size: 11px;
    font-weight: 700;
  }
</style>
