<script lang="ts">
  // "Hear without": flat, same loudness, nothing saved (it also pauses a Try).
  // Hold the button, or click on / click off, as Settings → Sound says.
  import { onDestroy } from "svelte";
  import { api, errorText, type EqUiSettings } from "../../lib/api";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";

  let { disabled = false, onchange }: { disabled?: boolean; onchange?: (on: boolean) => void } = $props();

  const mode = $derived((app.settings?.eq_ui as EqUiSettings | undefined)?.ab_mode ?? "hold");
  let bypass = $state(false);

  function set(on: boolean) {
    if (bypass === on) return;
    bypass = on;
    onchange?.(on);
    api.eqBypass(on).catch((e) => app.note(errorText(e), "error", "sound-hear"));
  }
  onDestroy(() => {
    if (bypass) api.eqBypass(false).catch(() => {});
  });
</script>

{#if mode === "hold"}
  <button
    class="btn tonal"
    class:pressed={bypass}
    {disabled}
    title="Hold to hear the music without the sound"
    onpointerdown={() => set(true)}
    onpointerup={() => set(false)}
    onpointerleave={() => set(false)}
    onkeydown={(e) => (e.key === " " || e.key === "Enter") && (e.preventDefault(), e.stopPropagation(), set(true))}
    onkeyup={() => set(false)}
    onblur={() => set(false)}
  ><Icon name="ear" size={16} /> Hear without</button>
{:else}
  <button class="btn tonal" class:pressed={bypass} aria-pressed={bypass} {disabled} title="Click to compare with and without the sound" onclick={() => set(!bypass)}><Icon name="ear" size={16} /> {bypass ? "Back to sound" : "Hear without"}</button>
{/if}

<style>
  .pressed {
    background: var(--primary-strong);
    color: var(--on-primary-strong);
    transform: translateY(1px);
  }
</style>
