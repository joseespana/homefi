<script lang="ts">
  // The equalizer's "Taste rules" tab: every taste layer (scope, tilt, who made
  // it, on / off, edit, delete), what plays now, a suggestion and loudness.
  import type { TasteBand, TasteRule, Tilt } from "../../lib/api";
  import { item, rise } from "../../lib/motion";
  import { LOUDNESS_MODES, SCOPE_KINDS, creatorLabel, ruleWords, scopeIcon, sound } from "../../lib/sound.svelte";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";
  import SuggestionCard from "./SuggestionCard.svelte";
  import TasteCurveEditor from "./TasteCurveEditor.svelte";

  // the playing rule follows the music (and the rules)
  const playingKey = $derived(`${app.player?.current}|${app.player?.taste_rule}|${sound.taste.rules.length}`);
  $effect(() => {
    playingKey;
    sound.refreshNow();
  });

  const taste = $derived(sound.taste);
  const loud = $derived(sound.loudness);
  const rank = (r: TasteRule) => SCOPE_KINDS.findIndex((k) => k.id === r.scope.kind);
  const rules = $derived([...taste.rules].sort((a, b) => rank(a) - rank(b) || a.scope.name.localeCompare(b.scope.name)));
  const now = $derived(sound.now);

  let editing = $state<{ id: string; tilt: Tilt; bands: TasteBand[]; label: string } | null>(null);
  function edit(r: TasteRule) {
    editing = { id: r.id, tilt: { bassDb: r.tilt.bassDb, trebleDb: r.tilt.trebleDb, clarityDb: r.tilt.clarityDb ?? 0 }, bands: JSON.parse(JSON.stringify(r.bands ?? [])), label: r.label };
  }
  async function saveEdit() {
    if (!editing) return;
    const { id, tilt, bands, label } = editing;
    editing = null;
    await sound.patchRule(id, { tilt, bands, label: label.trim() || undefined });
  }
  /** Saves what's in the editor, then the assistant goes on from that curve. */
  async function refine() {
    if (!editing) return;
    const id = editing.id;
    await saveEdit();
    const r = sound.taste.rules.find((x) => x.id === id);
    if (r) sound.refine(r);
  }
  const aiOn = $derived(!!app.settings?.ai?.provider);
  async function remove(r: TasteRule) {
    const ok = await app.confirm({ title: `Remove “${r.label}”?`, text: `The ${r.scope.kind} ${r.scope.name} goes back to the output's own sound.`, confirmLabel: "Remove", danger: true });
    if (ok) await sound.removeRule(r.id);
  }
  const loudnessGain = $derived(app.player?.loudness_db ?? 0);
</script>

<div class="taste">
  <div class="main">
    <section class="card head" in:rise>
      <div class="htext">
        <h3><Icon name="layers" size={16} /> Taste layers</h3>
        <p class="muted small">
          A little more bass for your electronic music, softer highs for one loud album… Each layer changes only its music, on top of your
          output's profile. When several match, the most specific plays: track › album › playlist › artist › genre.
        </p>
      </div>
      <label class="switch" title={taste.enabled ? "Turn taste layers off" : "Turn taste layers on"}>
        <input type="checkbox" role="switch" checked={taste.enabled} onchange={(e) => sound.saveTaste({ ...taste, enabled: e.currentTarget.checked })} />
        <span class="track"><span class="thumb"></span></span>
        <b>{taste.enabled ? "On" : "Off"}</b>
      </label>
    </section>

    <SuggestionCard />

    <section class="card">
      <h3>Rules <small class="muted">{rules.length}</small></h3>
      {#if !rules.length}
        <div class="empty">
          <Icon name="soundwave" size={22} />
          <p>No taste layers yet. While music plays, tap <b>How does it sound?</b> in the player, or right-click an album, playlist or artist and pick <b>Sound for this…</b></p>
        </div>
      {:else}
        <div class="rules" role="list">
          {#each rules as r (r.id)}
            {@const playingNow = now?.rule?.id === r.id}
            <div class="rule" role="listitem" class:off={!r.enabled || !taste.enabled} class:now={playingNow} in:item>
              <span class="kind" title={r.scope.kind}><Icon name={scopeIcon(r.scope.kind)} size={15} /></span>
              <div class="what">
                <div class="line1">
                  <b class="ellipsis">{r.label}</b>
                  {#if playingNow}<span class="live" title="Playing now"><span class="led"></span> Now</span>{/if}
                </div>
                <div class="line2 muted">
                  <span class="scope">{r.scope.kind} · {r.scope.name || "untitled"}</span>
                  <span class="mono" title={(r.bands ?? []).map((b) => b.note).filter(Boolean).join("\n") || undefined}>{ruleWords(r)}</span>
                  <span class="badge {r.createdBy}">{creatorLabel(r.createdBy)}</span>
                </div>
                {#if r.explanation && editing?.id !== r.id}<p class="why muted">{r.explanation}</p>{/if}
                {#if editing?.id === r.id}
                  <div class="editor" in:rise={{ y: 6, duration: 200 }}>
                    {#if r.explanation}<p class="why muted">{r.explanation}</p>{/if}
                    <TasteCurveEditor bind:tilt={editing.tilt} bind:bands={editing.bands} rule={r} onrefine={aiOn ? refine : null} />
                    <div class="erow">
                      <input bind:value={editing.label} maxlength="60" aria-label="Rule name" />
                      <button class="btn tonal small" onclick={() => (editing = null)}>Cancel</button>
                      <button class="btn primary small" onclick={saveEdit}>Save</button>
                    </div>
                  </div>
                {/if}
              </div>
              <div class="acts">
                <input type="checkbox" role="switch" class="mini-switch" checked={r.enabled} aria-label="{r.label} on" onchange={(e) => sound.patchRule(r.id, { enabled: e.currentTarget.checked })} />
                <button class="icon-btn mini" title="Edit" aria-label="Edit {r.label}" onclick={() => (editing?.id === r.id ? (editing = null) : edit(r))}><Icon name="edit" size={14} /></button>
                <button class="icon-btn mini" title="Remove" aria-label="Remove {r.label}" onclick={() => remove(r)}><Icon name="trash" size={14} /></button>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </section>
  </div>

  <aside class="side">
    <section class="card">
      <h3><Icon name="music" size={16} /> Playing now</h3>
      {#if now?.context}
        <p class="small"><b>{now.context.title ?? "This track"}</b>{#if now.context.genres.length}<span class="muted"> · {now.context.genres.join(", ")}</span>{/if}</p>
        <p class="small {now.rule ? 'accent' : 'muted'}">{now.rule ? `Layer: ${now.rule.label} (${ruleWords(now.rule)})` : "No taste layer for it: the output's profile alone."}</p>
        <div class="scopes">
          {#each now.scopes as s (s.kind + s.name)}
            <button class="chip" onclick={() => (sound.forScope = { scope: s, label: s.name || s.kind })}><Icon name={scopeIcon(s.kind)} size={12} /> <span class="ellipsis">{s.kind === "genre" ? s.name : `This ${s.kind}`}</span></button>
          {/each}
        </div>
      {:else}
        <p class="muted small">Play something to set a layer for it.</p>
      {/if}
    </section>

    <section class="card">
      <h3><Icon name="gauge" size={16} /> Loudness</h3>
      <p class="muted small">Evens out songs that are much louder or quieter than the rest, from their ReplayGain.</p>
      <div class="modes" role="radiogroup" aria-label="Loudness normalization">
        {#each LOUDNESS_MODES as m (m.id)}
          <button role="radio" aria-checked={loud.mode === m.id} class:on={loud.mode === m.id} title={m.hint} onclick={() => sound.saveLoudness({ ...loud, mode: m.id })}>{m.label}</button>
        {/each}
      </div>
      <p class="small muted">{LOUDNESS_MODES.find((m) => m.id === loud.mode)?.hint}</p>
      {#if loud.mode !== "off"}
        <label class="target">
          <span>Target <b class="mono">{loud.targetLufs} LUFS</b></span>
          <input type="range" min="-24" max="-8" step="1" value={loud.targetLufs} oninput={(e) => sound.saveLoudness({ ...loud, targetLufs: Number(e.currentTarget.value) })} />
        </label>
        <p class="small {Math.abs(loudnessGain) >= 0.05 ? 'warn-text' : 'muted'}">
          {Math.abs(loudnessGain) >= 0.05 ? `This song: ${loudnessGain > 0 ? "+" : "−"}${Math.abs(loudnessGain).toFixed(1)} dB · not bit-perfect` : "This song plays at its own level."}
        </p>
      {/if}
    </section>
  </aside>
</div>

<style>
  .taste {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 16px;
  }
  @container (min-width: 980px) {
    .taste {
      grid-template-columns: minmax(0, 1fr) 320px;
      align-items: start;
    }
  }
  .main,
  .side {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }
  .card {
    padding: 14px 16px;
    border-radius: min(16px, var(--r-max));
    background: var(--glass);
    border: 1px solid var(--glass-border);
    backdrop-filter: blur(18px) saturate(1.2);
  }
  .card.head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
  }
  .htext p {
    margin: 0;
    max-width: 62ch;
  }
  h3 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 0 10px;
    font-size: 14px;
  }
  h3 small {
    font-weight: 400;
  }
  .small {
    font-size: 12.5px;
    margin: 0 0 8px;
  }
  .accent {
    color: var(--accent-text);
  }
  .warn-text {
    color: #d97706;
  }
  .switch {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
  }
  .switch input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }
  .track {
    position: relative;
    width: 46px;
    height: 26px;
    border-radius: min(999px, var(--r-max));
    background: var(--surface-3);
    border: 1px solid var(--border);
    transition: background 0.2s;
  }
  .thumb {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 20px;
    height: 20px;
    border-radius: min(999px, var(--r-max));
    background: var(--surface);
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.35);
    transition: transform calc(0.2s * var(--motion-speed)) var(--ease-pop);
  }
  .switch input:checked + .track {
    background: var(--primary-strong);
  }
  .switch input:checked + .track .thumb {
    transform: translateX(20px);
  }
  .empty {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px;
    border-radius: min(12px, var(--r-max));
    border: 1px dashed var(--border);
    color: var(--muted);
  }
  .empty p {
    margin: 0;
    font-size: 13px;
    line-height: 1.45;
  }
  .rules {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .rule {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 10px;
    border-radius: min(12px, var(--r-max));
    background: var(--surface-2);
    border: 1px solid transparent;
    transition: opacity 0.2s;
  }
  .rule.now {
    border-color: color-mix(in srgb, var(--primary) 55%, transparent);
    background: color-mix(in srgb, var(--primary) 9%, var(--surface-2));
  }
  .rule.off .what,
  .rule.off .kind {
    opacity: 0.5;
  }
  .kind {
    flex: none;
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: min(10px, var(--r-max));
    color: var(--accent-text);
    background: color-mix(in srgb, var(--primary) 12%, transparent);
  }
  .what {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .line1 {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    font-size: 13.5px;
  }
  .line2 {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 10px;
    font-size: 12px;
  }
  .scope {
    text-transform: capitalize;
  }
  .mono {
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }
  .badge {
    padding: 1px 7px;
    border-radius: min(999px, var(--r-max));
    font-size: 10.5px;
    font-weight: 600;
    background: var(--glass-strong);
    border: 1px solid var(--glass-border);
  }
  .badge.ai {
    color: var(--accent-text);
  }
  .live {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 10.5px;
    font-weight: 700;
    color: var(--ok);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .led {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--ok);
    box-shadow: 0 0 6px var(--ok);
  }
  .acts {
    flex: none;
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .mini-switch {
    width: 16px;
    height: 16px;
    margin-right: 6px;
    accent-color: var(--primary-strong);
  }
  .icon-btn.mini {
    width: 28px;
    height: 28px;
  }
  .why {
    margin: 2px 0 0;
    font-size: 12px;
    line-height: 1.4;
  }
  .editor {
    container-type: inline-size;
    margin-top: 8px;
    padding: 10px;
    border-radius: min(10px, var(--r-max));
    background: var(--surface);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .erow {
    display: flex;
    gap: 6px;
  }
  .erow input {
    flex: 1;
    min-width: 0;
    padding: 5px 8px;
  }
  .btn.small {
    height: 30px;
    padding: 0 12px;
  }
  .scopes {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip,
  .modes button {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    max-width: 200px;
    height: 28px;
    padding: 0 11px;
    border-radius: min(999px, var(--r-max));
    font-size: 12px;
    color: var(--muted);
    background: var(--glass-strong);
    border: 1px solid var(--glass-border);
  }
  .chip:hover,
  .modes button:hover {
    color: var(--text);
  }
  .modes {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-bottom: 6px;
  }
  .modes button.on {
    color: var(--text);
    border-color: var(--primary);
    box-shadow: inset 0 0 0 1px var(--primary);
    background: color-mix(in srgb, var(--primary) 14%, var(--surface));
  }
  .target {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12.5px;
    margin-bottom: 6px;
  }
  .target input {
    accent-color: var(--primary-strong);
  }
</style>
