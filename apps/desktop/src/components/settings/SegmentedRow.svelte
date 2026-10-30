<script lang="ts" generics="T extends string">
  // A setting whose control is a Segmented strip.
  import SettingRow from "./SettingRow.svelte";
  import Segmented from "./Segmented.svelte";
  import type { SegmentOption } from "./types";

  let {
    label,
    hint,
    options,
    value,
    onchange,
    disabled = false,
    disabledReason,
  }: {
    label: string;
    hint?: string;
    options: readonly SegmentOption<T>[];
    value: T;
    onchange: (id: T) => void;
    disabled?: boolean;
    disabledReason?: string;
  } = $props();
</script>

<SettingRow {label} {hint} {disabled} {disabledReason}>
  {#snippet children(ids)}
    <Segmented {options} {value} {onchange} labelledby={ids.label} />
  {/snippet}
</SettingRow>
