<script lang="ts">
  // The beginner's button: "Mix to B" (or A) brings the other song in on the beat over N beats,
  // with the crossfader and the bass swap done for you. During the mix it becomes "Stop mix".
  import { dj } from "../../lib/dj.svelte";
  import { DJ_MIX_BEATS } from "../../lib/dj-api";

  const auto = $derived(dj.snap?.auto_mix ?? null);
  const target = $derived(dj.mixTarget);
  const can = $derived(dj.canMix);
  const progress = $derived(Math.round((auto?.progress ?? 0) * 100));
  const label = $derived(auto ? (auto.armed ? "Waiting for the beat… Stop" : "Stop mix") : target === "b" ? "Mix to B ➜" : "⬅ Mix to A");
  const tip = $derived(
    auto ? "Stop the mix where it is (moving the crossfader stops it too)" : can ? `Fade from ${target === "b" ? "A to B" : "B to A"} smoothly, with the beats lined up` : "Load a song on both decks to mix.",
  );
  const key = $derived(dj.settings.keys_enabled ? dj.keyFor(`auto_mix.to_${target}`) : null);
</script>

<div class="mix">
  <button class="mixbtn" class:stop={!!auto} type="button" aria-disabled={!can && !auto} title={key ? `${tip} [${key}]` : tip} onclick={() => dj.mix()}>{label}</button>
  <div class="mixlen">
    <span title="How many beats the fade lasts. Longer is smoother.">Mix length</span>
    <div class="seg" role="radiogroup" aria-label="Mix length in beats">
      {#each DJ_MIX_BEATS as beats (beats)}
        <button type="button" role="radio" aria-checked={dj.beats === beats} class:on={dj.beats === beats} onclick={() => (dj.mixBeats = beats)}>{beats}</button>
      {/each}
    </div>
  </div>
  <div class="mixprog" role="progressbar" aria-label="Mix progress" aria-valuemin={0} aria-valuemax={100} aria-valuenow={progress}>
    <i style:width="{progress}%"></i>
  </div>
</div>

<style>
  .mix {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .mixbtn {
    min-height: 56px;
    border-radius: var(--r-pill);
    background: var(--dj-mix-gradient, var(--primary));
    color: var(--dj-mix-ink);
    font-weight: 800;
    font-size: 15px;
    box-shadow: 0 6px 18px rgb(0 0 0 / 0.3);
  }
  /* skins without gradients (luna, aluminum, web2000) use their own primary button */
  :global(:root:is([data-skin="luna"], [data-skin="aluminum"], [data-skin="web2000"])) .mixbtn {
    background: var(--primary-strong);
    color: var(--on-primary-strong);
  }
  .mixbtn:hover {
    filter: brightness(1.08);
  }
  .mixbtn[aria-disabled="true"] {
    filter: saturate(0.3);
    opacity: 0.6;
    cursor: default;
  }
  .mixbtn.stop {
    background: var(--surface-3);
    color: var(--text);
    box-shadow: inset 0 0 0 2px var(--dj-b);
  }
  .mixlen {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    font-size: 12px;
    color: var(--muted);
  }
  .seg {
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: var(--r-pill);
    overflow: hidden;
  }
  .seg button {
    min-height: 32px;
    min-width: 40px;
    padding: 0 10px;
    font-size: 12px;
    color: var(--muted);
  }
  .seg button.on {
    background: var(--surface-3);
    color: var(--text);
    font-weight: 700;
  }
  .mixprog {
    height: 6px;
    border-radius: var(--r-xs);
    background: var(--dj-well);
    overflow: hidden;
  }
  .mixprog i {
    display: block;
    height: 100%;
    background: var(--dj-mix-gradient, var(--primary));
  }
</style>
