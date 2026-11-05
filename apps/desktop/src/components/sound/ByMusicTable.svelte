<script lang="ts">
  // By music: one row for each (sound, target): what it is, which sound it
  // sits in (change it right here), on which outputs, when and by whom it was
  // added. Filter by kind, output or name, sort by any column. Keyboard: arrows
  // move between rows, Enter opens the Sound menu, Delete takes the row out.
  import { tick } from "svelte";
  import type { TasteCreator, TasteRule, TasteScope, TasteScopeKind } from "../../lib/api";
  import { Windowed } from "../../lib/virtual.svelte";
  import { SCOPE_KINDS, kindLabel, kindRank, outputsLabel, playsOn, sameScope, sound } from "../../lib/sound.svelte";
  import Icon from "../Icon.svelte";
  import KindBadge from "./KindBadge.svelte";

  type Row = { key: string; rule: TasteRule; scope: TasteScope; name: string; kind: TasteScopeKind; addedMs: number; addedBy: TasteCreator };
  type SortKey = "name" | "kind" | "sound" | "output" | "added";

  /** Height of one row, for the windowed list. */
  const ROW_PX = 52;
  const ADDED_BY: Record<TasteCreator, string> = { manual: "you", ai: "AI", guided: "guide", reaction: "tap" };

  const win = new Windowed(ROW_PX);
  let kindFilter = $state<TasteScopeKind | "all">("all");
  let outputFilter = $state("");
  let query = $state("");
  let sort = $state<{ key: SortKey; dir: 1 | -1 }>({ key: "kind", dir: 1 });
  let cursor = $state(0);
  let list = $state<HTMLElement | null>(null);

  const rules = $derived(sound.rules);
  const all = $derived<Row[]>(
    rules.flatMap((rule) => rule.targets.map((t) => ({ key: `${rule.id}|${t.scope.kind}|${t.scope.kind === "genre" ? "" : t.scope.id}|${t.scope.name}`, rule, scope: t.scope, name: t.scope.name, kind: t.scope.kind, addedMs: t.addedMs, addedBy: t.addedBy }))),
  );
  const counts = $derived.by(() => {
    const c: Partial<Record<TasteScopeKind, number>> = {};
    for (const r of all) c[r.kind] = (c[r.kind] ?? 0) + 1;
    return c;
  });
  const outputs = $derived(sound.outputChoices());

  const rows = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const matches = all.filter((r) => (kindFilter === "all" || r.kind === kindFilter) && (!outputFilter || playsOn(r.rule, outputFilter)) && (!q || r.name.toLowerCase().includes(q)));
    const { key, dir } = sort;
    const by: Record<SortKey, (a: Row, b: Row) => number> = {
      name: (a, b) => a.name.localeCompare(b.name),
      kind: (a, b) => kindRank(a.kind) - kindRank(b.kind) || a.name.localeCompare(b.name),
      sound: (a, b) => a.rule.label.localeCompare(b.rule.label) || a.name.localeCompare(b.name),
      output: (a, b) => outputsLabel(a.rule).localeCompare(outputsLabel(b.rule)) || a.name.localeCompare(b.name),
      added: (a, b) => a.addedMs - b.addedMs || a.name.localeCompare(b.name),
    };
    return matches.sort((a, b) => dir * by[key](a, b));
  });

  function sortBy(key: SortKey) {
    sort = sort.key === key ? { key, dir: sort.dir === 1 ? -1 : 1 } : { key, dir: 1 };
  }
  const ariaSort = (key: SortKey) => (sort.key === key ? (sort.dir === 1 ? "ascending" : "descending") : "none");

  const fmtAdded = (r: Row) => (r.addedMs ? `${new Date(r.addedMs).toLocaleDateString("en-US", { month: "short", day: "numeric" })} · ${ADDED_BY[r.addedBy]}` : "—");

  function move(row: Row, soundId: string) {
    if (soundId !== row.rule.id) sound.assign(soundId, row.scope);
  }

  async function focusRow(i: number) {
    cursor = Math.max(0, Math.min(rows.length - 1, i));
    await tick();
    list?.querySelector<HTMLElement>(`[data-i="${cursor}"]`)?.focus();
  }
  function key(e: KeyboardEvent, i: number, row: Row) {
    if ((e.target as HTMLElement).tagName === "SELECT") return;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      focusRow(i + (e.key === "ArrowDown" ? 1 : -1));
    } else if (e.key === "Enter") {
      e.preventDefault();
      const sel = (e.currentTarget as HTMLElement).querySelector<HTMLSelectElement>("select");
      sel?.focus();
      try {
        sel?.showPicker?.();
      } catch {
        // not allowed without a click in some webviews: the focused select opens with Space
      }
    } else if (e.key === "Delete" || e.key === "Backspace") {
      e.preventDefault();
      sound.unassign(row.rule.id, row.scope);
      focusRow(i);
    }
  }
  // the filters changed: back to the first row
  $effect(() => {
    kindFilter;
    outputFilter;
    query;
    cursor = 0;
  });
</script>

<section class="card" aria-label="By music">
  <div class="tools">
    <div class="chips" role="group" aria-label="Kind">
      <button class="chip" class:on={kindFilter === "all"} aria-pressed={kindFilter === "all"} onclick={() => (kindFilter = "all")}>All <b>{all.length}</b></button>
      {#each SCOPE_KINDS as k (k.id)}
        <button class="chip" class:on={kindFilter === k.id} aria-pressed={kindFilter === k.id} disabled={!counts[k.id]} onclick={() => (kindFilter = kindFilter === k.id ? "all" : k.id)}>{k.plural} <b>{counts[k.id] ?? 0}</b></button>
      {/each}
    </div>
    <select bind:value={outputFilter} aria-label="Output">
      <option value="">All outputs</option>
      {#each outputs as o (o)}<option value={o}>{o}</option>{/each}
    </select>
    <label class="search"><Icon name="search" size={14} /><input bind:value={query} placeholder="Search" aria-label="Search the music in your sounds" /></label>
  </div>

  {#if !all.length}
    <p class="empty">Nothing is assigned yet. Add music to a sound from Playing now, the library menu or the Edit sound panel.</p>
  {:else}
    <div class="grid" role="grid" aria-rowcount={rows.length + 1}>
      <div class="head row" role="row">
        <span class="c-music" role="columnheader" aria-sort={ariaSort("name")}><button onclick={() => sortBy("name")}>Music {#if sort.key === "name"}{sort.dir === 1 ? "▲" : "▼"}{/if}</button></span>
        <span class="c-kind" role="columnheader" aria-sort={ariaSort("kind")}><button onclick={() => sortBy("kind")}>Kind {#if sort.key === "kind"}{sort.dir === 1 ? "▲" : "▼"}{/if}</button></span>
        <span class="c-sound" role="columnheader" aria-sort={ariaSort("sound")}><button onclick={() => sortBy("sound")}>Sound {#if sort.key === "sound"}{sort.dir === 1 ? "▲" : "▼"}{/if}</button></span>
        <span class="c-out" role="columnheader" aria-sort={ariaSort("output")}><button onclick={() => sortBy("output")}>Output {#if sort.key === "output"}{sort.dir === 1 ? "▲" : "▼"}{/if}</button></span>
        <span class="c-added" role="columnheader" aria-sort={ariaSort("added")}><button onclick={() => sortBy("added")}>Added {#if sort.key === "added"}{sort.dir === 1 ? "▲" : "▼"}{/if}</button></span>
        <span class="c-acts" role="columnheader"><span class="sr">Actions</span></span>
      </div>
      <div class="body" role="rowgroup" bind:this={list} {@attach win.attach(() => rows.length)}>
        <div aria-hidden="true" style:height="{win.before}px"></div>
        {#each rows.slice(win.start, win.end) as r, j (r.key)}
          {@const i = win.start + j}
          <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
          <div class="row" role="row" data-row data-i={i} aria-rowindex={i + 2} tabindex={i === cursor ? 0 : -1} onkeydown={(e) => key(e, i, r)} onfocus={() => (cursor = i)} style:min-height="{ROW_PX}px">
            <span class="c-music" role="gridcell"><b class="ellipsis">{r.name || "Untitled"}</b></span>
            <span class="c-kind" role="gridcell"><KindBadge kind={r.kind} /> <span class="kind-name">{kindLabel(r.kind)}</span></span>
            <span class="c-sound" role="gridcell">
              <select value={r.rule.id} aria-label="Sound for {r.name}" onchange={(e) => move(r, e.currentTarget.value)}>
                {#each rules as s (s.id)}<option value={s.id}>{s.label}</option>{/each}
              </select>
            </span>
            <span class="c-out ellipsis" role="gridcell">{outputsLabel(r.rule)}</span>
            <span class="c-added" role="gridcell">{fmtAdded(r)}</span>
            <span class="c-acts" role="gridcell">
              <button class="btn tonal small" onclick={() => sound.tryNow(r.rule)} title="Hear {r.rule.label} on this song">Try</button>
              <button class="icon-btn mini" aria-label="Take {r.name} out of {r.rule.label}" title="Remove" onclick={() => sound.unassign(r.rule.id, r.scope)}><Icon name="x" size={14} /></button>
            </span>
          </div>
        {/each}
        <div aria-hidden="true" style:height="{win.after}px"></div>
      </div>
      {#if !rows.length}<p class="empty">Nothing matches these filters.</p>{/if}
    </div>
  {/if}
</section>

<style>
  .card {
    container-type: inline-size;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 16px;
    border-radius: var(--r-lg);
    background: var(--glass);
    border: 1px solid var(--glass-border);
    backdrop-filter: blur(18px) saturate(1.2);
  }
  .tools {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    flex: 1;
  }
  .chip {
    height: 30px;
    padding: 0 12px;
    border-radius: var(--r-pill);
    font-size: 12.5px;
    color: var(--muted);
    background: var(--glass-strong);
    border: 1px solid var(--glass-border);
  }
  .chip b {
    margin-left: 4px;
    font-weight: 700;
    color: var(--text);
  }
  .chip.on {
    border-color: var(--primary);
    box-shadow: inset 0 0 0 1px var(--primary);
    background: color-mix(in srgb, var(--primary) 14%, var(--surface));
  }
  .chip:disabled {
    opacity: 0.5;
  }
  select {
    height: 30px;
    max-width: 100%;
    padding: 0 8px;
    border-radius: var(--r-sm);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 12.5px;
  }
  .search {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 10px;
    border-radius: var(--r-sm);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--muted);
  }
  .search input {
    width: 140px;
    border: 0;
    outline: none;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 12.5px;
  }
  .row {
    display: grid;
    grid-template-columns: minmax(150px, 2fr) 110px minmax(150px, 1.5fr) minmax(110px, 1fr) 110px 100px;
    align-items: center;
    gap: 10px;
    padding: 0 8px;
    font-size: 13px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 55%, transparent);
  }
  .body .row:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: -2px;
    border-radius: var(--r-sm);
  }
  .head {
    min-height: 32px;
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--faint);
  }
  .head button {
    font: inherit;
    letter-spacing: inherit;
    text-transform: inherit;
    color: inherit;
  }
  .head button:hover {
    color: var(--text);
  }
  .c-kind {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .kind-name {
    font-size: 12px;
    color: var(--muted);
  }
  .c-sound select {
    width: 100%;
  }
  .body .c-out,
  .body .c-added {
    font-size: 12px;
    color: var(--muted);
  }
  .c-acts {
    display: flex;
    justify-content: flex-end;
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
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }
  .empty {
    margin: 0;
    padding: 16px 4px;
    font-size: 13px;
    color: var(--muted);
  }
  @container (max-width: 760px) {
    .row {
      grid-template-columns: minmax(120px, 2fr) minmax(130px, 1.5fr) 100px;
    }
    .c-kind .kind-name,
    .c-out,
    .c-added {
      display: none;
    }
    .c-kind {
      display: none;
    }
  }
</style>
