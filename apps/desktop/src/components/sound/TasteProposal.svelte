<script lang="ts">
  // A taste layer the assistant put on the table (not saved yet): its curve
  // over the output's own profile, what plays with both (dashed), each band
  // with the assistant's one-line reason, and Apply / Keep talking.
  import { api, type TastePreview, type TasteRule } from "../../lib/api";
  import { formatDb, formatHz, hasGain } from "../../lib/eq.svelte";
  import { ruleWords, scopePhrase } from "../../lib/sound.svelte";
  import Icon from "../Icon.svelte";
  import EqGraph from "../eq/EqGraph.svelte";

  let { rule, open = true, busy = false, onapply, onkeep }: { rule: TasteRule; open?: boolean; busy?: boolean; onapply: () => void; onkeep: () => void } = $props();

  let preview = $state<TastePreview | null>(null);
  $effect(() => {
    api
      .tastePreview(JSON.parse(JSON.stringify(rule)))
      .then((p) => (preview = p))
      .catch(() => (preview = null));
  });
  const bands = $derived(rule.bands ?? []);
  let selected = $state<number | null>(null);
</script>

<div class="tp">
  <div class="ptitle">
    <Icon name="layers" size={14} />
    <b class="ellipsis">{rule.label}</b>
    <span class="muted">for {scopePhrase(rule.scope)}</span>
  </div>
  {#if preview}
    <EqGraph profile={{ name: "", preampDb: 0, bands, source: { kind: "manual" } }} curve={preview.taste} background={preview.device} compare={preview.combined} notes={bands.map((b) => b.note)} bind:selected height={160} range={12} />
  {/if}
  <div class="legend">
    <span><i class="sw layer"></i> This layer</span>
    {#if preview?.device}<span><i class="sw device"></i> {preview.device.name}</span>{/if}
    <span><i class="sw sum"></i> What plays</span>
    <span class="grow"></span>
    <span class="mono muted">{ruleWords(rule)}</span>
  </div>
  {#if bands.length}
    <ol class="blist">
      {#each bands as b, i (i)}
        <li class:sel={selected === i}>
          <span class="n">{i + 1}</span>
          <span class="mono">{hasGain(b.kind) ? formatDb(b.gainDb) : b.kind === "highPass" ? "High-pass" : "Low-pass"} · {formatHz(b.freqHz)} · Q {b.q}</span>
          {#if b.note}<span class="why">{b.note}</span>{/if}
        </li>
      {/each}
    </ol>
  {/if}
  {#if rule.explanation}<p class="explain">{rule.explanation}</p>{/if}
  {#if preview?.notes.length}<p class="kept"><Icon name="shield" size={12} /> Kept safe: {preview.notes.join(" · ")}</p>{/if}
  {#if open}
    <div class="pbuttons">
      <button class="btn primary" disabled={busy} onclick={onapply}><Icon name="check" size={16} /> Apply</button>
      <button class="btn tonal" disabled={busy} onclick={onkeep}>Keep talking</button>
    </div>
  {/if}
</div>

<style>
  .tp {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 14px;
    border-radius: min(16px, var(--r-max));
    background: var(--glass);
    border: 1px solid var(--glass-border);
  }
  .ptitle {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    font-size: 13.5px;
    color: var(--accent-text);
  }
  .ptitle b {
    color: var(--text);
  }
  .ptitle .muted {
    font-size: 12px;
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 12px;
    font-size: 11.5px;
    color: var(--muted);
  }
  .legend span {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .grow {
    flex: 1;
  }
  .sw {
    display: inline-block;
    width: 16px;
    border-top: 2.5px solid var(--accent-text);
  }
  .sw.device {
    border-top: 1.5px solid var(--muted);
    opacity: 0.5;
  }
  .sw.sum {
    border-top: 1.6px dashed var(--muted);
  }
  .mono {
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }
  .blist {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .blist li {
    display: grid;
    grid-template-columns: 20px auto 1fr;
    align-items: baseline;
    gap: 8px;
    padding: 3px 6px;
    border-radius: min(8px, var(--r-max));
    font-size: 12.5px;
  }
  .blist li.sel {
    background: color-mix(in srgb, var(--primary) 10%, transparent);
  }
  .n {
    font: 700 11px var(--font-mono);
    color: var(--accent-text);
    text-align: center;
  }
  .why {
    color: var(--muted);
  }
  .explain {
    margin: 0;
    font-size: 12.5px;
    color: var(--muted);
  }
  .kept {
    display: flex;
    gap: 6px;
    margin: 0;
    font-size: 11.5px;
    color: #d97706;
  }
  .pbuttons {
    display: flex;
    gap: 8px;
  }
</style>
