<script lang="ts">
  // One card of an OptionGallery: a preview (a snippet), the label under it, a check
  // when chosen (so the choice isn't told by colour alone).
  import type { Snippet } from "svelte";
  import Icon from "../Icon.svelte";

  let { label, hint, selected, disabled = false, onclick, preview, extra }: { label: string; hint?: string; selected: boolean; disabled?: boolean; onclick: () => void; preview?: Snippet; extra?: Snippet } = $props();
</script>

<button type="button" role="radio" class:selected aria-checked={selected} tabindex={selected ? 0 : -1} {disabled} {onclick}>
  {#if preview}<span class="preview">{@render preview()}</span>{/if}
  <span class="label">{label}</span>
  {#if hint}<span class="hint">{hint}</span>{/if}
  {#if extra}{@render extra()}{/if}
  {#if selected}<span class="check" aria-hidden="true"><Icon name="check" size={12} /></span>{/if}
</button>

<style>
  button {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: var(--s-1);
    padding: var(--s-3);
    min-width: 0;
    text-align: left;
    border: 1px solid var(--set-ctl-border);
    border-radius: var(--set-ctl-r);
    background: var(--set-ctl-bg);
    box-shadow: var(--set-ctl-shadow);
    transition: background 0.2s, border-color 0.2s, box-shadow 0.2s;
  }
  button:hover:not(.selected):not(:disabled) {
    border-color: var(--set-ctl-hover);
  }
  button.selected {
    background: var(--set-sel-bg);
    border-color: var(--set-sel-border);
    box-shadow: var(--set-sel-shadow);
    animation: hf-pick calc(0.32s * var(--motion-speed)) var(--ease-pop);
  }
  .preview:empty {
    display: none;
  }
  .preview {
    display: block;
    margin-bottom: var(--s-1);
    min-width: 0;
  }
  .label {
    font: var(--set-label-font);
    padding-right: var(--s-5);
  }
  .selected .label {
    color: var(--set-sel-fg);
    font-weight: 600;
  }
  .hint {
    font: var(--set-hint-font);
    color: var(--muted);
  }
  .check {
    position: absolute;
    top: var(--s-2);
    right: var(--s-2);
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border-radius: min(999px, var(--r-max));
    background: var(--set-sel-border);
    color: var(--on-primary, #fff);
  }
  button:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: -2px;
  }
</style>
