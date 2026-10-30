<script lang="ts" generics="T extends string">
  import type { Choice } from "./types";
  // Pick one of a few options when each needs a sentence of explanation: radio
  // rows inside the group (label and hint on the left, the radio on the right).
  import { roving } from "./roving";
  import { useRowSearch } from "./rowSearch.svelte";

  let { label, options, value, onchange }: { label: string; options: readonly Choice<T>[]; value: T; onchange: (id: T) => void } = $props();

  const search = useRowSearch(() => `${label} ${options.map((o) => `${o.label} ${o.hint ?? ""}`).join(" ")}`);
</script>

<div class="choices" role="radiogroup" aria-label={label} hidden={!search.visible} use:roving={{ selector: "[role=radio]:not(:disabled)", vertical: true }}>
  {#each options as o (o.id)}
    <button type="button" role="radio" class:selected={value === o.id} aria-checked={value === o.id} tabindex={value === o.id ? 0 : -1} disabled={o.disabled} onclick={() => onchange(o.id)}>
      <span class="text">
        <span class="label">{o.label}</span>
        {#if o.hint}<span class="hint">{o.hint}</span>{/if}
        {#if o.disabled && o.disabledReason}<span class="hint why">{o.disabledReason}</span>{/if}
      </span>
      <span class="radio" aria-hidden="true"></span>
    </button>
  {/each}
</div>

<style>
  .choices[hidden] {
    display: none;
  }
  button {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    column-gap: var(--s-4);
    width: 100%;
    min-height: var(--set-row-min);
    padding: var(--s-3) var(--s-4);
    text-align: left;
    transition: background 0.15s;
  }
  button + button {
    border-top: 1px solid var(--set-divider);
  }
  button:hover:not(:disabled):not(.selected) {
    background: var(--set-row-hover);
  }
  button.selected {
    background: var(--set-sel-bg);
  }
  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .label {
    font: var(--set-label-font);
    color: var(--text);
  }
  .selected .label {
    color: var(--set-sel-fg);
  }
  .hint {
    max-width: 60ch;
    font: var(--set-hint-font);
    color: var(--muted);
  }
  .radio {
    position: relative;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    border: 2px solid var(--faint);
    transition: border-color 0.15s;
  }
  .selected .radio {
    border-color: var(--set-sel-border);
  }
  .selected .radio::after {
    content: "";
    position: absolute;
    inset: 3px;
    border-radius: 50%;
    background: var(--set-sel-border);
  }
  button:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: -2px;
  }
</style>
