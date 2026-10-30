<script lang="ts">
  // A text or number setting: the label on the left, the box on the right
  // (under the label in a narrow group). The value is saved on change (blur / Enter).
  import SettingRow from "./SettingRow.svelte";

  let {
    label,
    hint,
    value,
    type = "text",
    placeholder,
    maxlength,
    min,
    max,
    inputmode,
    mono = false,
    width,
    onchange,
    disabled = false,
  }: {
    label: string;
    hint?: string;
    value: string | number;
    type?: "text" | "number" | "password";
    placeholder?: string;
    maxlength?: number;
    min?: number;
    max?: number;
    inputmode?: "numeric" | "text";
    mono?: boolean;
    /** A fixed width (a port, a code); the default is 220-320 px. */
    width?: string;
    onchange: (value: string) => void;
    disabled?: boolean;
  } = $props();
</script>

<SettingRow {label} {hint} {disabled} labelFor>
  {#snippet children(ids)}
    <input
      id={ids.control}
      class:mono
      class:fixed={!!width}
      style:--field-w={width}
      {type}
      {value}
      {placeholder}
      {maxlength}
      {min}
      {max}
      {inputmode}
      {disabled}
      autocomplete="off"
      spellcheck="false"
      aria-describedby={hint ? ids.hint : undefined}
      onchange={(e) => onchange(e.currentTarget.value)}
    />
  {/snippet}
</SettingRow>

<style>
  input {
    width: 280px;
    max-width: 100%;
    min-height: 36px;
    background: var(--set-field-bg);
    border: 1px solid var(--set-field-border);
    border-radius: var(--set-field-r);
    box-shadow: var(--set-field-shadow);
    color: var(--set-field-fg);
    font-size: 14px;
  }
  input.fixed {
    width: var(--field-w);
  }
  input.mono {
    font-family: var(--font-mono);
  }
  input:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 0;
  }
  @container group (max-width: 560px) {
    input:not(.fixed) {
      width: 100%;
    }
  }
</style>
