<script lang="ts" generics="T extends string">
  import type { SegmentOption } from "./types";
  // 2-4 short, mutually exclusive options as one pill strip (a radiogroup:
  // arrows move and choose, Tab enters on the selected one).
  import { roving } from "./roving";

  let {
    options,
    value,
    onchange,
    labelledby,
    label,
  }: {
    options: readonly SegmentOption<T>[];
    value: T;
    onchange: (id: T) => void;
    labelledby?: string;
    label?: string;
  } = $props();
</script>

<div class="seg" role="radiogroup" aria-labelledby={labelledby} aria-label={labelledby ? undefined : label} use:roving={{ selector: "[role=radio]:not(:disabled)", vertical: false }}>
  {#each options as o (o.id)}
    <button
      type="button"
      role="radio"
      class:selected={value === o.id}
      aria-checked={value === o.id}
      tabindex={value === o.id ? 0 : -1}
      title={o.hint}
      disabled={o.disabled}
      onclick={() => onchange(o.id)}
    >{o.label}</button>
  {/each}
</div>

<style>
  .seg {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-1);
    width: 100%;
    justify-content: flex-end;
  }
  button {
    flex: 1 1 auto;
    min-height: 36px;
    padding: 0 var(--s-3);
    border: 1px solid var(--set-ctl-border);
    border-radius: var(--set-ctl-r);
    background: var(--set-ctl-bg);
    box-shadow: var(--set-ctl-shadow);
    font: var(--set-label-font);
    white-space: nowrap;
    transition: background 0.2s, border-color 0.2s, box-shadow 0.2s;
  }
  button:hover:not(.selected):not(:disabled) {
    border-color: var(--set-ctl-hover);
  }
  button.selected {
    background: var(--set-sel-bg);
    border-color: var(--set-sel-border);
    box-shadow: var(--set-sel-shadow);
    color: var(--set-sel-fg);
    font-weight: 600;
  }
  button:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 2px;
  }
</style>
