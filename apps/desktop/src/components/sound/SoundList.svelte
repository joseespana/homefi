<script lang="ts">
  // Your sounds: one row for each sound (what it is for, who made it, whether
  // it plays now), with Try, Edit and delete, and the output's own curve last.
  import { item } from "../../lib/motion";
  import { correctionName, mainKind, outputsLabel, sound, targetsSummary } from "../../lib/sound.svelte";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";
  import AiTag from "./AiTag.svelte";
  import KindBadge from "./KindBadge.svelte";

  let { onoutput }: { onoutput: () => void } = $props();

  const rules = $derived(sound.rules);
  const output = $derived(sound.output);
  const playingId = $derived(sound.now?.rule?.id ?? null);
  const device = $derived(sound.profileFor(output));

  async function add() {
    const saved = await sound.newSound();
    if (saved) app.view = { name: "sound", edit: saved.rule.id };
  }
</script>

<section class="card" aria-label="Your sounds">
  <header>
    <h3 class="cap">Your sounds{output ? ` · on ${output}` : ""}</h3>
    <button class="btn tonal" onclick={add}><Icon name="plus" size={14} /> New</button>
  </header>

  <ul>
    {#each rules as r (r.id)}
      {@const playing = r.id === playingId}
      <li class:playing in:item={{ step: 18 }}>
        <KindBadge kind={mainKind(r)} />
        <div class="what">
          <div class="l1">
            <b class="ellipsis">{r.label}</b>
            <AiTag by={r.createdBy} />
          </div>
          <div class="l2 ellipsis muted" title={targetsSummary(r)}>
            {targetsSummary(r)}{#if playing}<span class="now">&nbsp;· playing now</span>{/if}{#if r.outputs?.length}<span>&nbsp;· {outputsLabel(r)}</span>{/if}
          </div>
        </div>
        <div class="acts">
          <button class="btn tonal small" onclick={() => sound.tryNow(r)} title="Hear it on this song, nothing saved">Try</button>
          <button class="btn tonal small" onclick={() => (app.view = { name: "sound", edit: r.id })}>Edit</button>
          <button class="icon-btn mini" aria-label="Delete {r.label}" title="Delete sound" onclick={() => sound.remove(r)}><Icon name="x" size={14} /></button>
        </div>
      </li>
    {/each}
    <li class="else">
      <KindBadge all />
      <div class="what">
        <div class="l1"><b>Everything else</b></div>
        <div class="l2 ellipsis muted">{device ? `${correctionName(device)} + correction` : "Flat"}</div>
      </div>
      <div class="acts"><button class="btn tonal small" onclick={onoutput}>Edit</button></div>
    </li>
  </ul>
  {#if !rules.length}<p class="empty muted">No sounds yet. Use “+ New”, or tap “New sound from this song” while music plays.</p>{/if}
  <p class="foot muted">Precedence: song &gt; album &gt; artist &gt; playlist &gt; genre &gt; everything else</p>
</section>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 14px 16px;
    border-radius: var(--r-lg);
    background: var(--glass);
    border: 1px solid var(--glass-border);
    backdrop-filter: blur(18px) saturate(1.2);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .cap {
    margin: 0;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--faint);
  }
  header .btn {
    height: 30px;
    padding: 0 14px;
    font-size: 12.5px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
  }
  li {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    padding: 9px 8px;
    border-radius: var(--r-sm);
    border-bottom: 1px solid color-mix(in srgb, var(--border) 55%, transparent);
  }
  li.playing {
    background: color-mix(in srgb, var(--primary) 10%, transparent);
    border-bottom-color: transparent;
  }
  li.else {
    border-bottom: 0;
  }
  .what {
    min-width: 0;
  }
  .l1 {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13.5px;
  }
  .l2 {
    margin-top: 1px;
    font-size: 12px;
  }
  .now {
    color: var(--accent-text);
  }
  .acts {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .small {
    height: 28px;
    padding: 0 12px;
    font-size: 12px;
  }
  .icon-btn.mini {
    width: 28px;
    height: 28px;
  }
  .empty,
  .foot {
    margin: 0;
    font-size: 12px;
  }
</style>
