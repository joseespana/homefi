<script lang="ts">
  // Bass / treble / clarity of a taste rule, ±6 dB in half steps.
  import type { Tilt } from "../../lib/api";
  import { formatDb } from "../../lib/eq.svelte";
  import { MAX_TILT_DB } from "../../lib/sound.svelte";

  let { tilt = $bindable(), compact = false }: { tilt: Tilt; compact?: boolean } = $props();

  const ROWS: { key: keyof Tilt; label: string; hint: string }[] = [
    { key: "bassDb", label: "Bass", hint: "Warmth and punch" },
    { key: "trebleDb", label: "Treble", hint: "Air and sparkle" },
    { key: "clarityDb", label: "Clarity", hint: "Voices and detail" },
  ];
  const set = (key: keyof Tilt, v: number) => (tilt = { ...tilt, [key]: Math.round(v * 2) / 2 });
</script>

<div class="tilt" class:compact>
  {#each ROWS as r (r.key)}
    {@const v = tilt[r.key] ?? 0}
    <label>
      <span class="top"><b>{r.label}</b> <span class="mono" class:pos={v > 0} class:neg={v < 0}>{formatDb(v)}</span></span>
      <input type="range" min={-MAX_TILT_DB} max={MAX_TILT_DB} step="0.5" value={v} aria-label={r.label} oninput={(e) => set(r.key, Number(e.currentTarget.value))} />
      {#if !compact}<small class="muted">{r.hint}</small>{/if}
    </label>
  {/each}
</div>

<style>
  .tilt {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 14px;
  }
  .tilt.compact {
    gap: 10px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 13px;
  }
  .top {
    display: flex;
    justify-content: space-between;
  }
  input {
    width: 100%;
    accent-color: var(--primary-strong);
  }
  .mono {
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
    color: var(--muted);
  }
  .pos {
    color: var(--accent-text);
  }
  .neg {
    color: #d97706;
  }
  small {
    font-size: 11px;
  }
  @container (max-width: 520px) {
    .tilt {
      grid-template-columns: 1fr;
    }
  }
</style>
