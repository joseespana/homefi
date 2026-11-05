<script lang="ts">
  // Trying a sound: it plays on this song at once and nothing is saved. Keep
  // saves it for the kind it was made for (or another), Undo stops it. It ends
  // by itself when the song changes. Slides in (instantly with motion off).
  import { rise } from "../../lib/motion";
  import { kindLabel, sound } from "../../lib/sound.svelte";

  const rule = $derived(sound.trying ?? (sound.now?.auditioning ? sound.now.rule : null));
  const keeps = $derived(sound.keepScopes(rule));
  const onlyOn = $derived(sound.onlyHere ? sound.output : null);
</script>

{#if rule}
  <section class="try" in:rise={{ y: -8, duration: 150 }} out:rise={{ y: -8, duration: 150 }} aria-live="polite">
    <p class="title">Trying “{rule.label}”</p>
    <div class="acts">
      {#if keeps.primary}
        <button class="btn primary" onclick={() => sound.keep(rule, keeps.primary!, onlyOn)}>
          Keep for {kindLabel(keeps.primary.kind).toLowerCase()}
        </button>
        {#each keeps.others as s (s.kind + s.name)}
          <button class="btn tonal" onclick={() => sound.keep(rule, s, onlyOn)}>{kindLabel(s.kind)}</button>
        {/each}
      {/if}
      <button class="btn plain" onclick={() => sound.stopTrying()}>Undo</button>
    </div>
    <p class="note muted">Plays on this song at once, nothing saved. Leaving the song ends the try. Hear without compares.</p>
  </section>
{/if}

<style>
  .try {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 6px 14px;
    padding: 12px 14px;
    border-radius: var(--r-md);
    background: color-mix(in srgb, var(--primary) 9%, var(--surface));
    border: 1.5px solid var(--primary);
  }
  .title {
    margin: 0;
    font-size: 14px;
    font-weight: 700;
  }
  .acts {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 6px;
  }
  .acts .btn {
    height: 32px;
    padding: 0 16px;
    font-size: 13px;
  }
  .btn.plain {
    background: transparent;
    color: var(--muted);
  }
  .note {
    grid-column: 1 / -1;
    margin: 0;
    font-size: 12px;
  }
</style>
