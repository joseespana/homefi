<script lang="ts">
  import type { RowIds } from "./types";
  // One setting: the label (and a hint) on the left, its control on the right.
  // It stacks (label above, control full width) in a narrow group, for wide
  // controls such as sliders and galleries, and hides itself when the search
  // doesn't match its words.
  import type { Snippet } from "svelte";
  import { useRowSearch } from "./rowSearch.svelte";

  let {
    label,
    hint,
    value,
    stack = "auto",
    disabled = false,
    disabledReason,
    labelFor = false,
    asLabel = false,
    children,
  }: {
    label: string;
    hint?: string;
    /** A readout at the end of the label line (sliders). */
    value?: string;
    /** `auto`: stacks in a narrow group; `always`: wide controls; `never`: switches. */
    stack?: "auto" | "always" | "never";
    disabled?: boolean;
    /** Said under the hint while it is disabled. */
    disabledReason?: string;
    /** The label is a `<label for>` of the control (inputs, selects). */
    labelFor?: boolean;
    /** The whole row is one `<label>`: a click anywhere works the switch. */
    asLabel?: boolean;
    children?: Snippet<[RowIds]>;
  } = $props();

  const uid = $props.id();
  const ids: RowIds = { control: `${uid}-c`, label: `${uid}-l`, hint: `${uid}-h` };

  const search = useRowSearch(() => `${label} ${hint ?? ""}`);
</script>

<svelte:element this={asLabel ? "label" : "div"} class="row set-row" class:disabled data-stack={stack} hidden={!search.visible} aria-disabled={disabled || undefined}>
  <div class="text">
    <div class="head">
      {#if labelFor}
        <label class="label" id={ids.label} for={ids.control}>{label}</label>
      {:else}
        <span class="label" id={ids.label}>{label}</span>
      {/if}
      {#if value !== undefined}<span class="value">{value}</span>{/if}
    </div>
    {#if hint}<p class="hint" id={ids.hint}>{hint}</p>{/if}
    {#if disabled && disabledReason}<p class="hint why">{disabledReason}</p>{/if}
  </div>
  {#if children}<div class="control">{@render children(ids)}</div>{/if}
</svelte:element>

<style>
  .row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    column-gap: var(--s-4);
    row-gap: var(--s-2);
    min-height: var(--set-row-min);
    padding: var(--s-3) var(--s-4);
  }
  .row[hidden] {
    display: none;
  }
  .text {
    min-width: 0;
  }
  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--s-3);
  }
  .label {
    font: var(--set-label-font);
    color: var(--text);
  }
  .value {
    flex: none;
    font: var(--set-label-font);
    font-variant-numeric: tabular-nums;
    color: var(--muted);
  }
  .hint {
    margin: 2px 0 0;
    max-width: 60ch;
    font: var(--set-hint-font);
    color: var(--muted);
    overflow-wrap: anywhere;
  }
  .hint.why {
    color: var(--text);
    opacity: 0.8;
  }
  .control {
    min-width: 0;
    justify-self: end;
    display: flex;
    align-items: center;
    gap: var(--s-2);
  }
  .row[data-stack="always"] {
    grid-template-columns: minmax(0, 1fr);
  }
  .row[data-stack="always"] .control {
    display: block;
    justify-self: stretch;
  }
  .disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .disabled .control {
    pointer-events: none;
  }
  /* a narrow group: the control drops under the label */
  @container group (max-width: 560px) {
    .row[data-stack="auto"] {
      grid-template-columns: minmax(0, 1fr);
    }
    .row[data-stack="auto"] .control {
      justify-self: stretch;
    }
    .row[data-stack="auto"] .control > :global(*) {
      flex: 1 1 auto;
      min-width: 0;
    }
  }
</style>
