<script lang="ts">
  // A number on a slider: the label and the live value on one line, the slider
  // across the whole row. The change is saved when the thumb is released.
  import SettingRow from "./SettingRow.svelte";

  let {
    label,
    hint,
    min,
    max,
    step,
    value,
    format = (n: number) => String(n),
    onchange,
    disabled = false,
  }: {
    label: string;
    hint?: string;
    min: number;
    max: number;
    step: number;
    value: number;
    /** The readout next to the label. */
    format?: (n: number) => string;
    onchange: (n: number) => void;
    disabled?: boolean;
  } = $props();

  // while dragging the readout follows the thumb; the setting changes on release
  let dragging = $state<number | null>(null);
</script>

<SettingRow {label} {hint} {disabled} stack="always" value={format(dragging ?? value)}>
  {#snippet children(ids)}
    <input
      type="range"
      {min}
      {max}
      {step}
      {value}
      {disabled}
      aria-labelledby={ids.label}
      aria-describedby={hint ? ids.hint : undefined}
      aria-valuetext={format(dragging ?? value)}
      oninput={(e) => (dragging = Number(e.currentTarget.value))}
      onchange={(e) => {
        dragging = null;
        onchange(Number(e.currentTarget.value));
      }}
    />
  {/snippet}
</SettingRow>

<style>
  input {
    width: 100%;
    height: var(--set-hit);
    margin: 0;
    padding: 0;
    border: 0;
    background: transparent;
    accent-color: var(--primary-strong);
    cursor: pointer;
  }
  input:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 0;
  }
</style>
