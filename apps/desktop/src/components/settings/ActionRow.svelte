<script lang="ts">
  // A setting that does something: the label (and what it does) on the left, buttons on the right.
  import type { Snippet } from "svelte";
  import SettingRow from "./SettingRow.svelte";

  let { label, hint, disabled = false, disabledReason, children }: { label: string; hint?: string; disabled?: boolean; disabledReason?: string; children: Snippet } = $props();
</script>

<SettingRow {label} {hint} {disabled} {disabledReason}>
  <div class="actions">{@render children()}</div>
</SettingRow>

<style>
  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--s-2);
  }
  /* danger: red text and border */
  .actions :global(.btn.danger) {
    color: var(--danger);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--danger) 55%, transparent);
  }
</style>
