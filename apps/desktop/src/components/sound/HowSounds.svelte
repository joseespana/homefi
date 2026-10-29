<script lang="ts">
  // "How does it sound?": one tap (needs bass, too bright, perfect...) and the
  // sound changes for this music only (a taste rule; with AI when set up, else
  // fixed safe steps). A note says what changed, with Undo.
  import type { TasteScope } from "../../lib/api";
  import { pop } from "../../lib/motion";
  import { REACTIONS, ruleWords, scopeIcon, scopeShort, sound } from "../../lib/sound.svelte";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";

  // `look`: the deck's hardware button, the room's chip, or a skin's chip (light on light skins)
  let { place, look = "chip", up = false }: { place: "deck" | "room"; look?: "hw" | "chip" | "skin" | "skin-light"; up?: boolean } = $props();

  const open = $derived(sound.reacting === place);
  const now = $derived(sound.now);
  // where the change goes: automatic (the main genre) or one the user picks
  let scopeIndex = $state(-1);
  $effect(() => {
    if (open) scopeIndex = -1;
  });
  const scopes = $derived(now?.scopes ?? []);
  const scope = $derived<TasteScope | null>(scopeIndex >= 0 ? (scopes[scopeIndex] ?? null) : null);
  const autoScope = $derived(scopes.find((s) => s.kind === "genre") ?? scopes.find((s) => s.kind === "artist") ?? scopes[0] ?? null);
  const playing = $derived(!!app.player?.queue.length && app.player?.current != null);
</script>

<div class="how look-{look}" class:up>
  <button
    class={look === "hw" ? "hw small" : "chip"}
    class:on={open}
    title="How does it sound?"
    aria-label="How does it sound?"
    aria-expanded={open}
    disabled={!playing}
    onclick={() => sound.open(place)}
  >
    <Icon name="soundwave" size={look === "hw" ? 16 : 14} />
    {#if look !== "hw"}<span class="label">How does it sound?</span>{/if}
  </button>

  {#if open}
    <button class="scrim" aria-label="Close" onclick={() => (sound.reacting = null)}></button>
    <div class="pop" role="dialog" aria-label="How does it sound?" transition:pop={{ origin: up ? "bottom right" : "top right", y: up ? 8 : -8 }}>
      <div class="phead">
        <b>How does it sound?</b>
        <span class="muted small">Tap what you hear. Only {scope ? scopeShort(scope) : autoScope ? scopeShort(autoScope) : "this music"} changes.</span>
      </div>

      {#if scopes.length > 1}
        <div class="scopes" role="radiogroup" aria-label="Change it for">
          <button role="radio" aria-checked={scopeIndex < 0} class:on={scopeIndex < 0} onclick={() => (scopeIndex = -1)} title="HomeFi picks: usually the main genre">
            <Icon name="sparkles" size={12} /> Auto{#if autoScope}<span class="faint"> · {autoScope.name}</span>{/if}
          </button>
          {#each scopes as s, i (s.kind + s.name)}
            {#if !(s.kind === "genre" && autoScope && s === autoScope)}
            <button role="radio" aria-checked={scopeIndex === i} class:on={scopeIndex === i} onclick={() => (scopeIndex = i)} title={`Only ${scopeShort(s)}`}>
              <Icon name={scopeIcon(s.kind)} size={12} /> <span class="ellipsis">{s.kind === "genre" ? s.name : s.kind}</span>
            </button>
            {/if}
          {/each}
        </div>
      {/if}

      <div class="grid">
        {#each REACTIONS as r (r.id)}
          <button class="react" class:perfect={r.id === "perfect"} class:wide={r.id === "vocalsBuried"} class:busy={sound.busy === r.id} disabled={!!sound.busy} onclick={() => sound.react(r.id, scope)}>
            <b>{r.label}</b>
            <small>{sound.busy === r.id ? "Adjusting…" : r.hint}</small>
          </button>
        {/each}
      </div>

      <div class="pfoot">
        {#if now?.rule}
          <span class="active ellipsis" title={ruleWords(now.rule)}><Icon name="layers" size={13} /> {now.rule.label}</span>
        {:else}
          <span class="muted small">No taste layer for this music yet.</span>
        {/if}
        <button class="link" onclick={() => ((sound.reacting = null), (app.view = { name: "eq", tab: "taste" }))}>Taste rules</button>
      </div>
    </div>
  {/if}
</div>

<style>
  .how {
    position: relative;
    display: inline-flex;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
    padding: 5px 12px;
    border-radius: min(999px, var(--r-max));
    font-size: 12px;
    color: var(--muted);
    background: var(--glass-strong);
    border: 1px solid var(--glass-border);
  }
  .chip:hover:not(:disabled),
  .chip.on {
    color: var(--text);
  }
  .chip.on {
    border-color: var(--primary);
    box-shadow: inset 0 0 0 1px var(--primary);
  }
  .chip:disabled {
    opacity: 0.5;
  }
  /* on the skins that draw their own chrome (dark glass / light glass) */
  .look-skin .chip {
    color: rgb(255 255 255 / 0.85);
    background: rgb(0 0 0 / 0.3);
    border-color: rgb(255 255 255 / 0.18);
  }
  .look-skin .chip:hover {
    color: #fff;
    background: rgb(0 0 0 / 0.45);
  }
  .look-skin-light .chip {
    color: #333;
    background: rgb(255 255 255 / 0.55);
    border-color: rgb(0 0 0 / 0.25);
  }
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 30;
    cursor: default;
  }
  .pop {
    position: absolute;
    right: 0;
    top: calc(100% + 8px);
    z-index: 31;
    width: min(400px, calc(100vw - 24px));
    padding: 12px;
    border-radius: min(16px, var(--r-max));
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
    color: var(--text);
    text-align: left;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .up .pop {
    top: auto;
    bottom: calc(100% + 12px);
  }
  .phead {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .phead b {
    font: 700 15px var(--font-display);
  }
  .small {
    font-size: 12px;
  }
  .scopes {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .scopes button {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    max-width: 170px;
    height: 26px;
    padding: 0 10px;
    border-radius: min(999px, var(--r-max));
    font-size: 11.5px;
    color: var(--muted);
    background: var(--surface-2);
    border: 1px solid var(--border);
    text-transform: capitalize;
  }
  .scopes button.on {
    color: var(--text);
    border-color: var(--primary);
    background: color-mix(in srgb, var(--primary) 14%, var(--surface));
  }
  .faint {
    color: var(--faint);
    text-transform: none;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 6px;
  }
  .react {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
    padding: 9px 11px;
    border-radius: min(12px, var(--r-max));
    background: var(--glass-strong);
    border: 1px solid var(--glass-border);
    text-align: left;
    transition: transform calc(0.15s * var(--motion-speed)) var(--ease-pop), border-color 0.15s;
  }
  .react b {
    font-size: 13px;
  }
  .react small {
    font-size: 11px;
    color: var(--faint);
  }
  .react:hover:not(:disabled) {
    border-color: var(--primary);
    transform: translateY(-1px);
  }
  .react:active:not(:disabled) {
    transform: scale(0.98);
  }
  .react.wide {
    grid-column: 1 / -1;
  }
  .react.perfect {
    grid-column: 1 / -1;
    flex-direction: row;
    align-items: baseline;
    gap: 8px;
    background: color-mix(in srgb, var(--ok) 10%, var(--surface));
  }
  .react.busy {
    border-color: var(--primary);
    animation: breathe 1s ease-in-out infinite;
  }
  .react:disabled:not(.busy) {
    opacity: 0.55;
  }
  @keyframes breathe {
    50% {
      box-shadow: 0 0 0 3px color-mix(in srgb, var(--primary) 25%, transparent);
    }
  }
  .pfoot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding-top: 8px;
    border-top: 1px solid var(--border);
  }
  .active {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    font-size: 12px;
    color: var(--accent-text);
  }
  .link {
    flex: none;
    font-size: 12.5px;
    color: var(--primary);
  }
  .link:hover {
    text-decoration: underline;
  }
  @container (max-width: 520px) {
    .look-skin .label,
    .look-skin-light .label {
      display: none;
    }
  }
</style>
