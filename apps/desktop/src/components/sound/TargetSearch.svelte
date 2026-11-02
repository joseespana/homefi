<script lang="ts">
  // Search for music to add to a sound: song, album, artist, playlist or
  // genre, in the library and on TIDAL. 300 ms after typing; an answer that
  // arrives after a newer question is dropped. Arrows move, Enter adds the
  // highlighted result, Esc closes (Edit sound's Esc handling relies on it).
  import { onDestroy, onMount, untrack } from "svelte";
  import { errorText, type TargetHit } from "../../lib/api";
  import { rise } from "../../lib/motion";
  import { holds, sound } from "../../lib/sound.svelte";
  import { app } from "../../lib/state.svelte";
  import KindBadge from "./KindBadge.svelte";

  let { soundId, initial = "", onadd, onclose }: { soundId: string; initial?: string; onadd: (hit: TargetHit) => void; onclose: () => void } = $props();

  /** Wait this long after the last key before asking. */
  const DEBOUNCE_MS = 300;

  let query = $state(untrack(() => initial));
  let hits = $state.raw<TargetHit[]>([]);
  let active = $state(0);
  let busy = $state(false);
  let asked = $state(false);
  let input = $state<HTMLInputElement | null>(null);
  let timer: ReturnType<typeof setTimeout> | null = null;
  let seq = 0;

  function ask(q: string) {
    const text = q.trim();
    const my = ++seq;
    if (!text) {
      hits = [];
      busy = false;
      asked = false;
      return;
    }
    busy = true;
    sound
      .searchTargets(text)
      .then((r) => {
        if (my !== seq) return;
        hits = r;
        active = 0;
        asked = true;
      })
      .catch((e) => my === seq && (app.note(errorText(e), "error", "sound-search"), (hits = [])))
      .finally(() => my === seq && (busy = false));
  }
  function typed() {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => ask(query), DEBOUNCE_MS);
  }
  onMount(() => {
    input?.focus();
    if (initial) ask(initial);
  });
  onDestroy(() => {
    seq++;
    if (timer) clearTimeout(timer);
  });

  const here = $derived(sound.ruleById(soundId));
  const inThis = (h: TargetHit) => !!here && holds(here, h.scope);
  const holder = (h: TargetHit) => (h.inSound && h.inSound !== soundId ? sound.ruleById(h.inSound) : null);
  const moving = $derived(hits[active] ? holder(hits[active]) : null);

  function key(e: KeyboardEvent) {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (hits.length) active = (active + (e.key === "ArrowDown" ? 1 : -1) + hits.length) % hits.length;
    } else if (e.key === "Enter") {
      e.preventDefault();
      const h = hits[active];
      if (h && !inThis(h)) onadd(h);
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onclose();
    }
  }
  $effect(() => {
    active;
    document.querySelector(".tsearch [data-active=true]")?.scrollIntoView({ block: "nearest" });
  });
</script>

<div class="tsearch" in:rise={{ y: 6, duration: 180 }}>
  <input
    bind:this={input}
    bind:value={query}
    oninput={typed}
    onkeydown={key}
    role="combobox"
    aria-expanded={hits.length > 0}
    aria-controls="target-results"
    aria-label="Search song, album, artist, playlist or genre"
    placeholder="Search song, album, artist, playlist or genre"
    autocomplete="off"
    spellcheck="false"
  />
  {#if hits.length}
    <ul id="target-results" role="listbox" aria-label="Results">
      {#each hits as h, i (h.scope.kind + (h.scope.kind === "genre" ? "" : h.scope.id) + h.scope.name)}
        <li role="option" aria-selected={i === active} aria-disabled={inThis(h)} data-active={i === active}>
          <button class:active={i === active} disabled={inThis(h)} onmouseenter={() => (active = i)} onclick={() => onadd(h)}>
            <KindBadge kind={h.scope.kind} />
            <span class="name ellipsis">{h.scope.name}</span>
            <span class="sub ellipsis">{h.subtitle}</span>
            <span class="src">{inThis(h) ? "Added" : h.source === "tidal" ? "Tidal" : "library"}</span>
          </button>
        </li>
      {/each}
    </ul>
  {:else if busy}
    <p class="hint">Searching…</p>
  {:else if asked}
    <p class="hint">Nothing found for “{query.trim()}”.</p>
  {/if}
  {#if moving}
    <p class="warn" role="status">{hits[active].scope.name} moves from “{moving.label}”{#if here} to “{here.label}”{/if}, with Undo for 8 s. Enter adds the highlighted result.</p>
  {/if}
</div>

<style>
  .tsearch {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  input {
    width: 100%;
    height: 38px;
    padding: 0 12px;
    border-radius: min(10px, var(--r-max));
    border: 1px solid var(--primary);
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 13.5px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 4px;
    max-height: 280px;
    overflow-y: auto;
    border-radius: min(10px, var(--r-max));
    border: 1px solid color-mix(in srgb, var(--primary) 50%, var(--border));
    background: var(--surface-2);
  }
  li button {
    width: 100%;
    display: grid;
    grid-template-columns: auto minmax(0, auto) minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    padding: 6px 8px;
    border-radius: min(8px, var(--r-max));
    text-align: left;
    font-size: 13px;
  }
  li button.active {
    background: color-mix(in srgb, var(--primary) 14%, transparent);
  }
  li button:disabled {
    opacity: 0.55;
  }
  .name {
    font-weight: 700;
  }
  .sub,
  .src {
    font-size: 11.5px;
    color: var(--muted);
  }
  .src {
    justify-self: end;
  }
  .hint {
    margin: 0;
    padding: 4px 2px;
    font-size: 12.5px;
    color: var(--muted);
  }
  .warn {
    margin: 0;
    padding: 8px 10px;
    border-radius: min(8px, var(--r-max));
    font-size: 12px;
    line-height: 1.4;
    color: var(--warn);
    background: var(--warn-bg);
    border: 1px solid var(--warn-border);
  }
</style>
