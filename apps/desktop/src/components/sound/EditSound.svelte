<script lang="ts">
  // Edit sound: one sound's curve, the outputs it plays on and the music it
  // applies to, in a panel over the Sound screen. Curve edits are saved a
  // moment after the last one (like the equalizer). Esc closes the search,
  // then the panel.
  import { onDestroy, untrack } from "svelte";
  import type { TasteBand, TasteScope, TasteScopeKind, Tilt } from "../../lib/api";
  import { assistant } from "../../lib/assistant.svelte";
  import { usePreview } from "../../lib/preview.svelte";
  import { SCOPE_KINDS, outputsLabel, ruleNeutral, sound } from "../../lib/sound.svelte";
  import { app } from "../../lib/state.svelte";
  import EqGraph from "../eq/EqGraph.svelte";
  import Icon from "../Icon.svelte";
  import AiTag from "./AiTag.svelte";
  import SidePanel from "./SidePanel.svelte";
  import TargetSearch from "./TargetSearch.svelte";
  import TasteCurveEditor from "./TasteCurveEditor.svelte";

  let { id, search = "", onclose }: { id: string; search?: string; onclose: () => void } = $props();

  /** Curve edits are saved at most this often (and once more after the last one). */
  const SAVE_DELAY_MS = 140;
  /** The kinds in the order "Applies to" lists them. */
  const GROUP_ORDER: TasteScopeKind[] = ["artist", "track", "album", "playlist", "genre"];

  const rule = $derived(sound.ruleById(id));
  const aiOn = $derived(assistant.ready);

  // the curve being edited: a draft that is saved, not re-read from the settings while typing
  let tilt = $state<Tilt>({ bassDb: 0, trebleDb: 0, clarityDb: 0 });
  let bands = $state<TasteBand[]>([]);
  let editingCurve = $state(false);
  let renaming = $state<string | null>(null);
  let searching = $state(untrack(() => !!search));
  let loadedId: string | null = null;
  let shown = "";
  let timer: ReturnType<typeof setTimeout> | null = null;

  $effect(() => {
    assistant.loadStatus();
    sound.loadDevices();
  });
  $effect(() => {
    const r = rule;
    if (!r || r.id === loadedId) return;
    untrack(() => {
      loadedId = r.id;
      tilt = { bassDb: r.tilt.bassDb, trebleDb: r.tilt.trebleDb, clarityDb: r.tilt.clarityDb ?? 0 };
      bands = JSON.parse(JSON.stringify(r.bands ?? []));
      shown = JSON.stringify([tilt, bands]);
      // a sound with no curve yet opens straight on the curve
      editingCurve = ruleNeutral(r);
    });
  });
  // a sound removed elsewhere closes its panel
  $effect(() => {
    if (!rule && loadedId) onclose();
  });

  function flush() {
    if (timer) clearTimeout(timer);
    timer = null;
    const r = sound.ruleById(id);
    const now = JSON.stringify([tilt, bands]);
    if (!r || now === shown) return;
    shown = now;
    sound.save({ ...r, tilt: { ...tilt, clarityDb: tilt.clarityDb ?? null }, bands: JSON.parse(JSON.stringify(bands)) }, true);
  }
  $effect(() => {
    const now = JSON.stringify([tilt, bands]);
    if (now === shown) return;
    timer ??= setTimeout(flush, SAVE_DELAY_MS);
  });
  onDestroy(flush);

  const preview = usePreview(
    () => (rule ? { ...rule, tilt, bands } : null),
    () => sound.output ?? rule?.outputs?.[0] ?? null,
  );
  const handles = $derived({ name: "", preampDb: 0, bands: [], source: { kind: "manual" as const } });

  const outputs = $derived(sound.outputChoices());
  const picked = $derived(rule?.outputs ?? []);
  function toggleOutput(name: string) {
    if (!rule) return;
    const next = picked.includes(name) ? picked.filter((o) => o !== name) : [...picked, name];
    sound.setOutputs(rule.id, next);
  }

  const groups = $derived(
    GROUP_ORDER.map((kind) => ({ kind, label: SCOPE_KINDS.find((k) => k.id === kind)!.plural, scopes: (rule?.targets ?? []).map((t) => t.scope).filter((s) => s.kind === kind) })).filter((g) => g.scopes.length),
  );

  async function add(hit: { scope: TasteScope }) {
    searching = false;
    await sound.assign(id, hit.scope);
  }
  async function commitRename() {
    const name = renaming?.trim();
    renaming = null;
    if (rule && name && name !== rule.label) await sound.save({ ...rule, label: name }, true);
  }
  async function removeSound() {
    if (rule && (await sound.remove(rule))) onclose();
  }
  /** Esc: the search first, then a rename, then the panel. */
  function escape(): boolean {
    if (searching) return (searching = false), true;
    if (renaming !== null) return (renaming = null), true;
    return false;
  }
  function refine() {
    if (!rule) return;
    flush();
    assistant.refine(rule);
  }
</script>

<SidePanel label="Edit sound" {onclose} onescape={escape}>
  {#if rule}
    <header class="head">
      <div class="titles">
        <h2 class="ellipsis">{rule.label} <AiTag by={rule.createdBy} /></h2>
        <p class="muted">{outputsLabel(rule)} · {rule.targets.length} {rule.targets.length === 1 ? "target" : "targets"}</p>
      </div>
      <button class="btn tonal" onclick={() => sound.tryNow({ ...rule, tilt, bands })}>Try</button>
      <button class="btn primary" onclick={onclose}>Done</button>
    </header>

    <div class="curve">
      <EqGraph profile={handles} curve={preview.value?.taste ?? null} height={110} range={12} />
    </div>
    <div class="row">
      <button class="btn tonal" aria-expanded={editingCurve} onclick={() => (editingCurve = !editingCurve)}>Edit curve</button>
      {#if renaming === null}
        <button class="btn tonal" onclick={() => (renaming = rule.label)}>Rename</button>
      {:else}
        <!-- svelte-ignore a11y_autofocus -->
        <input class="name" bind:value={renaming} maxlength="60" aria-label="Sound name" autofocus onkeydown={(e) => e.key === "Enter" && commitRename()} onblur={commitRename} />
      {/if}
      <button class="btn tonal danger" onclick={removeSound}>Delete sound</button>
    </div>

    {#if editingCurve}
      <div class="editor">
        <TasteCurveEditor bind:tilt bind:bands {rule} height={170} onrefine={aiOn ? refine : null} />
      </div>
    {/if}

    <section aria-labelledby="plays-on">
      <h3 id="plays-on" class="cap">Plays on</h3>
      <div class="chips">
        <button class="chip" class:on={!picked.length} aria-pressed={!picked.length} onclick={() => sound.setOutputs(rule.id, [])}>All outputs</button>
        {#each outputs as o (o)}
          <button class="chip" class:on={picked.includes(o)} aria-pressed={picked.includes(o)} onclick={() => toggleOutput(o)}>{o}</button>
        {/each}
      </div>
    </section>

    <section aria-labelledby="applies-to">
      <div class="applies-head">
        <h3 id="applies-to" class="cap">Applies to</h3>
        <button class="btn primary add" aria-expanded={searching} onclick={() => (searching = !searching)}><Icon name="plus" size={14} /> Add</button>
      </div>
      {#if searching}
        <TargetSearch soundId={rule.id} initial={search} onadd={add} onclose={() => (searching = false)} />
      {/if}
      {#if !rule.targets.length}
        <p class="empty">Applies to nothing yet. Add an artist, song, album, playlist or genre.</p>
      {:else}
        {#each groups as g (g.kind)}
          <div class="group">
            <span class="cap">{g.label}</span>
            <div class="chips">
              {#each g.scopes as s (s.kind + (s.kind === "genre" ? "" : s.id) + s.name)}
                <span class="target">
                  <span class="ellipsis">{s.name || "Untitled"}</span>
                  <button class="x" aria-label="Remove {s.name} from {rule.label}" onclick={() => sound.unassign(rule.id, s)}><Icon name="x" size={12} /></button>
                </span>
              {/each}
            </div>
          </div>
        {/each}
      {/if}
    </section>
  {/if}
</SidePanel>

<style>
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .titles {
    flex: 1;
    min-width: 0;
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    font-size: 19px;
  }
  .titles p {
    margin: 2px 0 0;
    font-size: 12.5px;
  }
  .curve {
    border-radius: var(--r-sm);
    background: var(--surface-2);
    border: 1px solid var(--border);
    overflow: hidden;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .btn.danger {
    color: var(--danger);
  }
  .name {
    flex: 1;
    min-width: 0;
    height: 36px;
    padding: 0 12px;
    border-radius: var(--r-pill);
    border: 1px solid var(--primary);
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
  }
  .editor {
    padding: 12px;
    border-radius: var(--r-sm);
    background: var(--surface-2);
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .cap {
    margin: 0;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--faint);
  }
  .applies-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .add {
    height: 30px;
    padding: 0 14px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip {
    max-width: 100%;
    height: 30px;
    padding: 0 12px;
    border-radius: var(--r-pill);
    font-size: 12.5px;
    color: var(--muted);
    background: var(--glass-strong);
    border: 1px solid var(--glass-border);
  }
  .chip.on {
    color: var(--on-primary-strong);
    background: var(--primary-strong);
    border-color: var(--primary-strong);
  }
  .group {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 4px;
  }
  .target {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    max-width: 100%;
    height: 30px;
    padding: 0 4px 0 12px;
    border-radius: var(--r-pill);
    font-size: 12.5px;
    background: var(--surface-2);
    border: 1px solid var(--border);
  }
  .x {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    color: var(--muted);
  }
  .x:hover {
    color: var(--text);
    background: var(--surface-3);
  }
  .empty {
    margin: 0;
    padding: 10px 12px;
    border-radius: var(--r-sm);
    border: 1px dashed var(--border);
    font-size: 12.5px;
    color: var(--muted);
  }
</style>
