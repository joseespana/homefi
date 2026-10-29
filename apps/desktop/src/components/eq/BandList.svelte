<script lang="ts">
  // The bands of a curve as rows: on / type / frequency / gain / Q / remove.
  // Used by the equalizer and by a taste rule's advanced editor (with the
  // assistant's one-line note under each band).
  import { EQ_LIMITS, type EqBandKind, type TasteBand } from "../../lib/api";
  import { BAND_KINDS, hasGain } from "../../lib/eq.svelte";
  import { item } from "../../lib/motion";
  import Icon from "../Icon.svelte";

  type Patch = Partial<{ freqHz: number; gainDb: number; q: number; kind: EqBandKind; enabled: boolean }>;
  let {
    bands,
    selected = $bindable(null),
    gainDb = EQ_LIMITS.gainDb,
    q = EQ_LIMITS.q,
    notes = false,
    onedit,
    onremove,
  }: {
    bands: TasteBand[];
    selected?: number | null;
    gainDb?: readonly [number, number];
    q?: readonly [number, number];
    /** Show each band's note under it. */
    notes?: boolean;
    onedit: (index: number, patch: Patch) => void;
    onremove: (index: number) => void;
  } = $props();
</script>

<div class="bands" role="list">
  <div class="brow headrow" aria-hidden="true"><span>#</span><span>On</span><span>Type</span><span>Frequency</span><span>Gain</span><span>Q</span><span></span></div>
  {#each bands as b, i (i)}
    <div class="brow" role="listitem" class:selected={selected === i} class:off={!b.enabled} in:item onpointerdown={() => (selected = i)}>
      <span class="n">{i + 1}</span>
      <input type="checkbox" checked={b.enabled} aria-label="Band {i + 1} on" onchange={(e) => onedit(i, { enabled: e.currentTarget.checked })} />
      <select value={b.kind} aria-label="Band {i + 1} type" onchange={(e) => onedit(i, { kind: e.currentTarget.value as EqBandKind })}>
        {#each BAND_KINDS as k}<option value={k.id}>{k.label}</option>{/each}
      </select>
      <label class="unit"><input type="number" min="20" max="20000" step="1" value={b.freqHz} aria-label="Band {i + 1} frequency" onchange={(e) => onedit(i, { freqHz: Number(e.currentTarget.value) })} /><span>Hz</span></label>
      <label class="unit"><input type="number" min={gainDb[0]} max={gainDb[1]} step="0.1" value={b.gainDb} disabled={!hasGain(b.kind)} aria-label="Band {i + 1} gain" onchange={(e) => onedit(i, { gainDb: Number(e.currentTarget.value) })} /><span>dB</span></label>
      <input type="number" min={q[0]} max={q[1]} step="0.05" value={b.q} aria-label="Band {i + 1} Q" onchange={(e) => onedit(i, { q: Number(e.currentTarget.value) })} />
      <button class="icon-btn mini" title="Remove band" aria-label="Remove band {i + 1}" onclick={() => onremove(i)}><Icon name="x" size={14} /></button>
      {#if notes && b.note}<p class="note"><Icon name="ai" size={11} /> {b.note}</p>{/if}
    </div>
  {/each}
</div>

<style>
  .bands {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .brow {
    display: grid;
    grid-template-columns: 26px 26px minmax(110px, 1.1fr) minmax(100px, 1fr) minmax(90px, 1fr) minmax(70px, 0.7fr) 30px;
    align-items: center;
    gap: 8px;
    padding: 4px 6px;
    border-radius: min(10px, var(--r-max));
    border: 1px solid transparent;
  }
  .brow.selected {
    border-color: color-mix(in srgb, var(--primary) 55%, transparent);
    background: color-mix(in srgb, var(--primary) 9%, transparent);
  }
  .brow.off > :not(input[type="checkbox"]):not(button) {
    opacity: 0.5;
  }
  .headrow {
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--faint);
  }
  .n {
    font: 700 12px var(--font-mono);
    color: var(--accent-text);
    text-align: center;
  }
  .brow input[type="checkbox"] {
    width: 16px;
    height: 16px;
    accent-color: var(--primary-strong);
  }
  .brow select,
  .brow input[type="number"] {
    width: 100%;
    min-width: 0;
    padding: 5px 8px;
    font-size: 13px;
    font-variant-numeric: tabular-nums;
  }
  .unit {
    position: relative;
    display: flex;
    align-items: center;
  }
  .unit span {
    position: absolute;
    right: 8px;
    font-size: 11px;
    color: var(--faint);
    pointer-events: none;
  }
  .unit input {
    padding-right: 30px !important;
  }
  .unit input::-webkit-inner-spin-button {
    display: none;
  }
  .note {
    grid-column: 3 / -1;
    display: flex;
    align-items: center;
    gap: 5px;
    margin: -2px 0 2px;
    font-size: 11.5px;
    color: var(--muted);
  }
  @container (max-width: 640px) {
    .brow {
      grid-template-columns: 20px 20px minmax(0, 1.2fr) minmax(0, 1fr) minmax(0, 1fr) minmax(0, 0.8fr) 28px;
      gap: 4px;
    }
  }
</style>
