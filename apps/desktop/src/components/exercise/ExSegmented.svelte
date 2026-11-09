<script lang="ts" generics="T extends string">
  // A pill strip of 2-4 exclusive options, 44 px high (a radiogroup: arrows move and choose).
  import { roving } from "../settings/roving";

  let {
    options,
    value,
    onchange,
    label,
  }: {
    options: readonly { id: T; label: string; hint?: string; disabled?: boolean }[];
    value: T;
    onchange: (id: T) => void;
    label: string;
  } = $props();
</script>

<div class="seg" role="radiogroup" aria-label={label} use:roving={{ selector: "[role=radio]:not(:disabled)", vertical: false }}>
  {#each options as o (o.id)}
    <button type="button" role="radio" aria-checked={value === o.id} tabindex={value === o.id ? 0 : -1} title={o.hint} disabled={o.disabled} onclick={() => onchange(o.id)}>{o.label}</button>
  {/each}
</div>

<style>
  .seg {
    display: inline-flex;
    max-width: 100%;
    border: 1px solid var(--border);
    border-radius: var(--r-pill);
    background: var(--surface-2);
    overflow: hidden;
  }
  button {
    min-height: var(--ride-hit);
    padding: 0 16px;
    font-weight: 600;
    white-space: nowrap;
    flex: 1 1 auto;
    transition: background 0.15s;
  }
  button:hover:not([aria-checked="true"]):not(:disabled) {
    background: var(--surface-3);
  }
  button[aria-checked="true"] {
    background: var(--primary);
    color: var(--on-primary);
  }
  button:focus-visible {
    outline-offset: -3px;
  }
</style>
