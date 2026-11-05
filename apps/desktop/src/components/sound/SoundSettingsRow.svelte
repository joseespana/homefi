<script lang="ts">
  // "Intensity Balanced · Loudness Auto · Suggestions on ▾": the sound
  // settings in one line that opens to change them (the same settings as
  // Settings → Sound).
  import { INTENSITIES, LOUDNESS_MODES, sound } from "../../lib/sound.svelte";
  import { rise } from "../../lib/motion";
  import Icon from "../Icon.svelte";

  let open = $state(false);
  const taste = $derived(sound.taste);
  const loud = $derived(sound.loudness);
  const intensity = $derived(INTENSITIES.find((i) => i.id === taste.intensity)?.label ?? "");
  const loudness = $derived(LOUDNESS_MODES.find((m) => m.id === loud.mode)?.label ?? "");
</script>

<section class="card">
  <button class="head" aria-expanded={open} aria-controls="sound-settings" onclick={() => (open = !open)}>
    <span class="cap">Sound settings</span>
    <span class="sum">Intensity {intensity} · Loudness {loudness} · Suggestions {sound.ui.suggestions ? "on" : "off"} <Icon name={open ? "up" : "down"} size={12} /></span>
  </button>
  {#if open}
    <div id="sound-settings" class="body" in:rise={{ y: 6, duration: 180 }}>
      <div class="row">
        <span>Intensity <small class="muted">the size of each step the AI and “How does it sound?” take</small></span>
        <div class="seg" role="radiogroup" aria-label="Intensity">
          {#each INTENSITIES as i (i.id)}
            <button role="radio" aria-checked={taste.intensity === i.id} class:on={taste.intensity === i.id} title={i.hint} onclick={() => sound.saveTaste({ ...taste, intensity: i.id })}>{i.label}</button>
          {/each}
        </div>
      </div>
      <div class="row">
        <span>Loudness <small class="muted">evens out songs from their ReplayGain</small></span>
        <div class="seg" role="radiogroup" aria-label="Loudness normalization">
          {#each LOUDNESS_MODES as m (m.id)}
            <button role="radio" aria-checked={loud.mode === m.id} class:on={loud.mode === m.id} title={m.hint} onclick={() => sound.saveLoudness({ ...loud, mode: m.id })}>{m.label}</button>
          {/each}
        </div>
      </div>
      <label class="row">
        <span>Suggestions <small class="muted">a sound for what you listen to most</small></span>
        <input type="checkbox" role="switch" checked={sound.ui.suggestions} onchange={(e) => sound.saveUi({ suggestions: e.currentTarget.checked })} />
      </label>
    </div>
  {/if}
</section>

<style>
  .card {
    padding: 4px 16px;
    border-radius: var(--r-lg);
    background: var(--glass);
    border: 1px solid var(--glass-border);
  }
  .head {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 44px;
    text-align: left;
  }
  .cap {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--faint);
  }
  .sum {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    color: var(--muted);
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 4px 0 12px;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    font-size: 13px;
  }
  small {
    display: block;
    font-size: 11.5px;
  }
  .seg {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
    justify-content: flex-end;
  }
  .seg button {
    height: 28px;
    padding: 0 12px;
    border-radius: var(--r-pill);
    font-size: 12px;
    color: var(--muted);
    background: var(--glass-strong);
    border: 1px solid var(--glass-border);
  }
  .seg button.on {
    color: var(--text);
    border-color: var(--primary);
    box-shadow: inset 0 0 0 1px var(--primary);
    background: color-mix(in srgb, var(--primary) 14%, var(--surface));
  }
  input[type="checkbox"] {
    width: 18px;
    height: 18px;
    accent-color: var(--primary-strong);
  }
</style>
