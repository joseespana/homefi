<script lang="ts">
  // A sound's curve, two ways: "Simple" (bass / treble / clarity sliders; only
  // when Settings → Sound keeps the tilt sliders on) or "Advanced" (the
  // parametric curve: drag a band, double-click to add one, wheel or pinch for
  // its width, the band list below). The sound's own curve is drawn over the
  // output's profile (faint), with what plays, both together, dashed. The
  // assistant's note for each band shows on hover and under it.
  import { TASTE_LIMITS, type EqBandKind, type EqProfile, type TasteBand, type TasteRule, type Tilt } from "../../lib/api";
  import { clamp, formatHz, newBand } from "../../lib/eq.svelte";
  import { usePreview } from "../../lib/preview.svelte";
  import { neutralRule, sound, tiltWords } from "../../lib/sound.svelte";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";
  import BandList from "../eq/BandList.svelte";
  import EqGraph from "../eq/EqGraph.svelte";
  import TiltEditor from "./TiltEditor.svelte";

  let {
    tilt = $bindable(),
    bands = $bindable(),
    rule = null,
    height = 210,
    onrefine = null,
  }: {
    tilt: Tilt;
    bands: TasteBand[];
    /** The sound being edited (for the preview on its outputs). */
    rule?: TasteRule | null;
    height?: number;
    /** "Ask the AI to refine" (hidden without it). */
    onrefine?: (() => void) | null;
  } = $props();

  /** Settings → Sound → "Tilt sliders in Edit curve": off leaves only the curve. */
  const sliders = $derived(app.settings?.eq_ui?.taste_sliders ?? true);
  const pref = sound.ui.taste_editor ?? "auto";
  let picked = $state<"simple" | "advanced">(pref === "advanced" || (pref === "auto" && bands.length > 0) ? "advanced" : "simple");
  const mode = $derived(sliders ? picked : "advanced");
  let selected = $state<number | null>(null);

  const preview = usePreview(
    () => ({ ...(rule ?? neutralRule("")), tilt, bands }),
    () => sound.output,
  );

  const handles = $derived<EqProfile>({ name: "", preampDb: 0, bands, source: { kind: "manual" } });
  const full = $derived(bands.length >= TASTE_LIMITS.maxBands);
  const tiltSaid = $derived(tiltWords(tilt));

  type Patch = Partial<{ freqHz: number; gainDb: number; q: number; kind: EqBandKind; enabled: boolean }>;
  function edit(i: number, patch: Patch) {
    bands = bands.map((b, k) => {
      if (k !== i) return b;
      const next = { ...b };
      if (patch.freqHz !== undefined) next.freqHz = Math.round(clamp(patch.freqHz, TASTE_LIMITS.freqHz));
      if (patch.gainDb !== undefined) next.gainDb = Math.round(clamp(patch.gainDb, TASTE_LIMITS.gainDb) * 10) / 10;
      if (patch.q !== undefined) next.q = Math.round(clamp(patch.q, TASTE_LIMITS.q) * 100) / 100;
      if (patch.kind !== undefined) next.kind = patch.kind;
      if (patch.enabled !== undefined) next.enabled = patch.enabled;
      return next;
    });
  }
  function add(freqHz?: number, gainDb = 0) {
    if (full) return;
    bands = [...bands, { ...newBand(bands, freqHz, clamp(gainDb, TASTE_LIMITS.gainDb)), note: "" }];
    selected = bands.length - 1;
  }
  function remove(i: number) {
    bands = bands.filter((_, k) => k !== i);
    selected = null;
  }
  const tip = $derived(selected !== null && bands[selected] ? bands[selected] : null);
</script>

<div class="tce">
  <div class="modes" role="tablist" aria-label="Editor">
    {#if sliders}
      <button role="tab" aria-selected={mode === "simple"} class:on={mode === "simple"} onclick={() => (picked = "simple")}>Simple</button>
      <button role="tab" aria-selected={mode === "advanced"} class:on={mode === "advanced"} onclick={() => (picked = "advanced")}>
        Advanced{#if bands.length}<span class="count">{bands.length}</span>{/if}
      </button>
    {/if}
    <span class="grow"></span>
    {#if onrefine}<button class="btn tonal small refine" onclick={onrefine}><Icon name="ai" size={14} /> Ask the AI to refine</button>{/if}
  </div>

  {#if mode === "simple"}
    <TiltEditor bind:tilt compact />
    {#if bands.length}<p class="muted small"><Icon name="info" size={12} /> Plus a curve of {bands.length} {bands.length === 1 ? "band" : "bands"}: see Advanced.</p>{/if}
  {:else}
    <EqGraph
      profile={handles}
      curve={preview.value?.taste ?? null}
      background={preview.value?.device ?? null}
      compare={preview.value?.combined ?? null}
      editable
      bind:selected
      {height}
      range={12}
      gainLimits={TASTE_LIMITS.gainDb}
      notes={bands.map((b) => b.note)}
      onedit={edit}
      onadd={(f, g) => add(f, g)}
      onremove={remove}
    />
    <div class="legend">
      <span><i class="sw layer"></i> This sound</span>
      {#if preview.value?.device}<span title="The output's own profile, under the sound"><i class="sw device"></i> {preview.value.device.name}</span>{/if}
      <span title="Output profile and sound together, as it plays"><i class="sw sum"></i> What plays</span>
      <span class="grow"></span>
      <button class="btn tonal small" disabled={full} title={full ? `At most ${TASTE_LIMITS.maxBands} bands` : "Add a band"} onclick={() => add()}><Icon name="plus" size={14} /> Band</button>
    </div>
    {#if tip?.note}<p class="caption"><Icon name="ai" size={12} /> <b>{formatHz(tip.freqHz)}</b> {tip.note}</p>{/if}
    {#if bands.length}
      <BandList {bands} bind:selected gainDb={TASTE_LIMITS.gainDb} q={TASTE_LIMITS.q} notes onedit={edit} onremove={remove} />
    {:else}
      <p class="muted small">No bands yet: double-click the curve or press “Band”. A narrow cut fixes harshness; a wide lift adds warmth.</p>
    {/if}
    {#if tiltSaid !== "No change"}<p class="muted small">Also from Simple: {tiltSaid}.</p>{/if}
  {/if}
  {#if preview.value?.notes.length}<p class="kept small"><Icon name="shield" size={12} /> Kept safe: {preview.value.notes.join(" · ")}</p>{/if}
</div>

<style>
  .tce {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }
  .modes {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .modes [role="tab"] {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 12px;
    border-radius: var(--r-pill);
    font-size: 12.5px;
    color: var(--muted);
    background: var(--glass-strong);
    border: 1px solid var(--glass-border);
  }
  .modes [role="tab"].on {
    color: var(--text);
    border-color: var(--primary);
    box-shadow: inset 0 0 0 1px var(--primary);
    background: color-mix(in srgb, var(--primary) 14%, var(--surface));
  }
  .count {
    min-width: 16px;
    padding: 0 4px;
    border-radius: var(--r-sm);
    font-size: 10.5px;
    font-weight: 700;
    background: var(--primary-strong);
    color: var(--on-primary-strong);
  }
  .grow {
    flex: 1;
  }
  .btn.small {
    height: 28px;
    padding: 0 10px;
    font-size: 12.5px;
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 14px;
    font-size: 11.5px;
    color: var(--muted);
  }
  .legend span {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .sw {
    display: inline-block;
    width: 16px;
    height: 0;
    border-top: 2.5px solid var(--accent-text);
  }
  .sw.device {
    border-top: 1.5px solid var(--muted);
    opacity: 0.5;
  }
  .sw.sum {
    border-top: 1.6px dashed var(--muted);
  }
  .small {
    margin: 0;
    font-size: 12px;
  }
  .caption {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-size: 12.5px;
    color: var(--accent-text);
  }
  .kept {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    color: var(--warn);
  }
</style>
