<script lang="ts">
  // The output playing: its name, the headphone correction it uses (change it
  // or find your headphones) and the outputs to switch to the curve of.
  import { onMount } from "svelte";
  import { outputIcon } from "../../lib/api";
  import { correctionName, sound } from "../../lib/sound.svelte";
  import Icon from "../Icon.svelte";

  let { onedit }: { onedit: (output: string | null) => void } = $props();

  onMount(() => void sound.loadDevices());
  const output = $derived(sound.output);
  const profile = $derived(sound.profileFor(output));
  const outputs = $derived(sound.outputChoices());
</script>

<section class="card" aria-label="Output">
  <span class="cap">Output</span>
  <h3>{output ?? "System default"}</h3>
  <p class="muted">
    {#if profile}
      Headphone correction: {correctionName(profile)} <span class="ok">✓</span> · <button class="link" onclick={() => onedit(output)}>Change</button> · <button class="link" onclick={() => onedit(output)}>Find my headphones</button>
    {:else}
      No headphone correction · <button class="link" onclick={() => onedit(output)}>Find my headphones</button>
    {/if}
  </p>
  <div class="chips">
    {#each outputs as o (o)}
      <button class="chip" class:on={o === output} aria-pressed={o === output} onclick={() => onedit(o)} title="Edit the curve of {o}">
        <Icon name={outputIcon(o)} size={14} /> <span class="ellipsis">{o}</span>
      </button>
    {/each}
    <button class="chip add" onclick={() => onedit(null)}>+ Add output</button>
  </div>
</section>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 14px 16px;
    border-radius: min(18px, var(--r-max));
    background: var(--glass);
    border: 1px solid var(--glass-border);
    backdrop-filter: blur(18px) saturate(1.2);
  }
  .cap {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--faint);
  }
  h3 {
    margin: 0;
    font-size: 15px;
  }
  p {
    margin: 0;
    font-size: 12.5px;
  }
  .ok {
    color: var(--ok);
  }
  .link {
    color: var(--primary);
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 4px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    max-width: 100%;
    height: 32px;
    padding: 0 14px;
    border-radius: min(999px, var(--r-max));
    font-size: 12.5px;
    color: var(--muted);
    background: var(--glass-strong);
    border: 1px solid var(--glass-border);
  }
  .chip.on {
    color: var(--on-primary-strong);
    background: var(--primary-strong);
    border-color: var(--primary-strong);
  }
</style>
