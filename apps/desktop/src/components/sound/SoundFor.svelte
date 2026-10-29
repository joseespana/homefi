<script lang="ts">
  // "Sound for this album / playlist / artist / track": ask the assistant
  // (the easy way), or set the taste layer by hand. Esc / backdrop closes.
  import { untrack } from "svelte";
  import type { TasteBand, Tilt } from "../../lib/api";
  import { dialog, fade } from "../../lib/motion";
  import { app } from "../../lib/state.svelte";
  import { creatorLabel, ruleNeutral, ruleWords, scopeIcon, scopePhrase, sound } from "../../lib/sound.svelte";
  import Icon from "../Icon.svelte";
  import TasteCurveEditor from "./TasteCurveEditor.svelte";

  const target = $derived(sound.forScope!);
  const existing = $derived(sound.ruleFor(target.scope));
  let tilt = $state<Tilt>({ bassDb: 0, trebleDb: 0, clarityDb: 0 });
  let bands = $state<TasteBand[]>([]);
  let label = $state("");
  let manual = $state(false);
  $effect(() => {
    const r = existing;
    untrack(() => {
      tilt = r ? { bassDb: r.tilt.bassDb, trebleDb: r.tilt.trebleDb, clarityDb: r.tilt.clarityDb ?? 0 } : { bassDb: 0, trebleDb: 0, clarityDb: 0 };
      bands = r ? JSON.parse(JSON.stringify(r.bands ?? [])) : [];
      label = r?.label ?? "";
      manual = !!r;
    });
  });
  const neutral = $derived(ruleNeutral({ tilt, bands }));
  const close = () => (sound.forScope = null);
  const aiOn = $derived(!!app.settings?.ai?.provider);

  async function save() {
    await sound.setRule(target.scope, tilt, label, "manual", bands);
    close();
  }
  /** Saves what's set by hand, then the assistant refines that curve. */
  async function refine() {
    await sound.setRule(target.scope, tilt, label, "manual", bands);
    const r = sound.ruleFor(target.scope);
    if (r) sound.refine(r);
  }
  async function remove() {
    if (existing) await sound.removeRule(existing.id);
    close();
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

<div class="backdrop" role="presentation" transition:fade onclick={(e) => e.target === e.currentTarget && close()}>
  <div class="dialog" transition:dialog role="dialog" aria-modal="true" aria-labelledby="soundfor-title">
    <div class="head">
      <span class="mark" aria-hidden="true"><Icon name={scopeIcon(target.scope.kind)} size={18} /></span>
      <div class="titles">
        <span class="cap">Sound for this {target.scope.kind}</span>
        <h2 id="soundfor-title" class="ellipsis">{target.label}</h2>
      </div>
      <button class="icon-btn" aria-label="Close" onclick={close}><Icon name="x" size={16} /></button>
    </div>

    {#if existing}
      <p class="now"><Icon name="layers" size={14} /> <b>{existing.label}</b> <span class="muted">· {ruleWords(existing)} · {creatorLabel(existing.createdBy)}</span></p>
    {:else}
      <p class="muted">A taste layer changes how {scopePhrase(target.scope)} sounds, on top of your headphones' profile. Everything else stays as it is.</p>
    {/if}

    <button class="ask" onclick={() => sound.askAi(target.scope)}>
      <Icon name="ai" size={18} />
      <span><b>Ask the assistant</b><small>Tell it what you hear in your own words; it proposes the change and asks before applying.</small></span>
      <Icon name="next" size={14} />
    </button>

    {#if manual}
      <div class="manual">
        <TasteCurveEditor bind:tilt bind:bands rule={existing} height={180} onrefine={aiOn && !neutral ? refine : null} />
        <label class="lbl">
          <span class="cap">Name</span>
          <input bind:value={label} maxlength="60" placeholder="e.g. More punch for this album" />
        </label>
      </div>
    {:else}
      <button class="link" onclick={() => (manual = true)}>Or set it by hand (three sliders, or the full curve)</button>
    {/if}

    <div class="actions">
      {#if existing}<button class="btn tonal danger-text" onclick={remove}><Icon name="trash" size={14} /> Remove</button>{/if}
      <span class="grow"></span>
      <button class="btn tonal" onclick={close}>Cancel</button>
      {#if manual}<button class="btn primary" disabled={neutral} title={neutral ? "Move a slider or add a band first" : undefined} onclick={save}>Save</button>{/if}
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: grid;
    place-items: center;
    background: rgb(4 10 18 / 0.6);
    backdrop-filter: blur(6px);
  }
  .dialog {
    container-type: inline-size;
    width: min(640px, calc(100vw - 32px));
    max-height: calc(100vh - 48px);
    overflow-y: auto;
    padding: 20px 22px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: min(20px, var(--r-max));
    box-shadow: var(--shadow);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .mark {
    flex: none;
    display: grid;
    place-items: center;
    width: 38px;
    height: 38px;
    border-radius: min(12px, var(--r-max));
    color: var(--accent-text);
    background: color-mix(in srgb, var(--primary) 14%, transparent);
  }
  .titles {
    flex: 1;
    min-width: 0;
  }
  .cap {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--faint);
  }
  h2 {
    margin: 0;
    font-size: 18px;
  }
  p {
    margin: 0;
    font-size: 13px;
    line-height: 1.45;
  }
  .now {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    color: var(--accent-text);
  }
  .ask {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 14px;
    border-radius: min(14px, var(--r-max));
    text-align: left;
    background: color-mix(in srgb, var(--primary) 10%, var(--surface));
    border: 1px solid color-mix(in srgb, var(--primary) 40%, var(--border));
    color: var(--text);
  }
  .ask:hover {
    border-color: var(--primary);
  }
  .ask span {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .ask small {
    color: var(--muted);
    font-size: 12px;
  }
  .manual {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 12px;
    border-radius: min(14px, var(--r-max));
    background: var(--surface-2);
  }
  .lbl {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .link {
    align-self: flex-start;
    font-size: 12.5px;
    color: var(--primary);
  }
  .link:hover {
    text-decoration: underline;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .grow {
    flex: 1;
  }
  .danger-text {
    color: var(--danger);
  }
</style>
