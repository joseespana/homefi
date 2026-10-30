<script lang="ts">
  // On / off setting: label and hint on the left, a switch on the right. The
  // whole row is the click target (the switch itself has a 44 px hit area).
  import SettingRow from "./SettingRow.svelte";

  let {
    label,
    hint,
    checked,
    onchange,
    disabled = false,
    disabledReason,
  }: {
    label: string;
    hint?: string;
    checked: boolean;
    onchange: (on: boolean) => void;
    disabled?: boolean;
    disabledReason?: string;
  } = $props();
</script>

<SettingRow {label} {hint} {disabled} {disabledReason} stack="never" asLabel>
  {#snippet children(ids)}
    <button type="button" class="switch" role="switch" aria-checked={checked} aria-labelledby={ids.label} aria-describedby={hint ? ids.hint : undefined} {disabled} class:on={checked} onclick={() => onchange(!checked)}>
      <span class="track"><span class="knob"></span></span>
    </button>
  {/snippet}
</SettingRow>

<style>
  .switch {
    display: grid;
    place-items: center;
    min-width: var(--set-hit);
    height: var(--set-hit);
    padding: 0;
  }
  .track {
    position: relative;
    width: var(--switch-w);
    height: var(--switch-h);
    border-radius: var(--switch-r);
    border: 1px solid var(--switch-border);
    background: var(--switch-off);
    transition: background 0.2s, box-shadow 0.2s;
  }
  .on .track {
    background: var(--switch-on);
    box-shadow: var(--switch-glow);
  }
  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: calc(var(--switch-h) - 4px - 2px);
    height: calc(var(--switch-h) - 4px - 2px);
    border-radius: var(--switch-knob-r);
    background: var(--switch-knob);
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.35);
    transition: transform calc(0.18s * var(--motion-speed)) var(--ease-out);
  }
  .on .knob {
    background: var(--switch-knob-on);
    transform: translateX(calc(var(--switch-w) - var(--switch-h)));
  }
  .switch:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: -2px;
  }
</style>
