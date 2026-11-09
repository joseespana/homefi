<script lang="ts">
  // Eight synthesized one-shots (made by HomeFi, no samples shipped); which sound sits on each pad is the
  // layout's (Edit layout). Overlapping presses mix.
  import { dj } from "../../lib/dj.svelte";
  import { SFX } from "../../lib/dj-api";
  import Icon from "../Icon.svelte";
  import DjKnob from "./DjKnob.svelte";

  /** The pads in the layout's order, each with the sound the rider gave it. */
  const pads = $derived(dj.layout.pads.map((sound) => SFX[sound]));
  const active = $derived(dj.snap?.pads_active ?? []);
  const keyOf = (n: number) => (dj.settings.keys_enabled ? dj.keyFor(`sampler.${n + 1}`) : null);
</script>

<section class="sampler" aria-label="Sampler">
  <h2>Sampler</h2>
  <div class="sp8">
    {#each pads as pad, n (n)}
      {@const key = keyOf(n)}
      <button type="button" class:play={!!active[n]} title="Plays a sound effect{key ? ` [${key}]` : ''}" onpointerdown={(e) => e.button === 0 && dj.send({ type: "sampler", pad: n })} onkeydown={(e) => (e.key === " " || e.key === "Enter") && (e.preventDefault(), dj.send({ type: "sampler", pad: n }))}>
        {#if key}<kbd>{key}</kbd>{/if}
        <span aria-hidden="true"><Icon name={pad.icon} size={20} /></span>{pad.name}
      </button>
    {/each}
  </div>
  <div class="vol">
    <DjKnob
      value={dj.snap?.sampler_volume ?? dj.settings.sampler_volume}
      detent={0.8}
      size={40}
      label="Sampler volume"
      tip="Volume of the sound effects"
      valueText={(v) => `${Math.round(v * 100)} %`}
      onchange={(value) => dj.send({ type: "sampler_volume", value })}
    />
  </div>
</section>

<style>
  .sampler {
    container: sampler / inline-size;
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    padding: 10px 12px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
  }
  h2 {
    flex: none;
    width: 62px;
    margin: 0;
    font-size: 11px;
    color: var(--faint);
    text-transform: uppercase;
    letter-spacing: 0.1em;
  }
  .sp8 {
    flex: 1;
    display: grid;
    grid-template-columns: repeat(8, minmax(0, 1fr));
    gap: 8px;
  }
  /* the pads wrap into two rows of four when the sampler is narrow, and into two columns when it is tiny */
  @container sampler (max-width: 719px) {
    .sp8 {
      grid-template-columns: repeat(4, minmax(0, 1fr));
    }
  }
  @container sampler (max-width: 359px) {
    .sp8 {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  .sp8 button {
    position: relative;
    height: 60px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    overflow: hidden;
    border-radius: var(--r-md);
    border: 1px solid var(--border);
    background: linear-gradient(var(--hw-top), var(--hw-bottom));
    font-weight: 600;
    font-size: 12px;
  }
  .sp8 button:hover {
    border-color: var(--primary);
  }
  .sp8 span {
    margin-top: 8px;
    display: grid;
    place-items: center;
    line-height: 1;
  }
  kbd {
    position: absolute;
    top: 3px;
    left: 6px;
    font: 500 9px var(--font-mono);
    color: var(--faint);
  }
  @container sampler (max-width: 899px) {
    kbd {
      display: none;
    }
  }
  /* playing: one ring pulse (full motion), a short flash (subtle), the colour only (off) */
  .sp8 button.play {
    border-color: var(--primary);
    background: color-mix(in srgb, var(--primary) 22%, var(--surface-2));
    animation: pad calc(0.6s * var(--motion-speed, 1)) var(--ease-out, ease-out);
  }
  @keyframes pad {
    from {
      box-shadow: 0 0 0 0 color-mix(in srgb, var(--primary) 60%, transparent);
      background: var(--primary);
      color: var(--on-primary);
    }
    to {
      box-shadow: 0 0 0 14px transparent;
    }
  }
  :global(:root[data-motion="subtle"]) .sp8 button.play {
    animation-duration: 0.12s;
  }
  :global(:root[data-motion="off"]) .sp8 button.play {
    animation: none;
  }
  .vol {
    flex: none;
  }
</style>
