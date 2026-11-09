<script lang="ts">
  // Songs to play: the library, a TIDAL search, and the crate (songs set aside). Drag a row onto a
  // deck, or press its A / B; double-click loads it on the free deck (Settings → DJ decides).
  import { api, errorText, type Hit, type Track } from "../../lib/api";
  import { dj, type CrateItem } from "../../lib/dj.svelte";
  import { djApi, type DeckId } from "../../lib/dj-api";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";

  let { drawer = false, onclose }: { drawer?: boolean; onclose?: () => void } = $props();

  type Tab = "library" | "tidal" | "crate";
  const TABS: { id: Tab; label: string }[] = [
    { id: "library", label: "Library" },
    { id: "tidal", label: "TIDAL" },
    { id: "crate", label: "Queue" },
  ];
  /** Rows the library tab shows (it's a picker, not the whole library). */
  const MAX_ROWS = 80;

  let tab = $state<Tab>("library");
  let query = $state("");
  let tidal = $state.raw<Hit[]>([]);
  let searching = $state(false);
  let searchError = $state<string | null>(null);
  let bpm = $state.raw<Record<string, number | null>>({});

  const library = $derived.by((): Track[] => {
    const q = query.trim().toLowerCase();
    const all = app.tracks;
    const hits = q ? all.filter((t) => `${t.title} ${t.artist} ${t.album}`.toLowerCase().includes(q)) : all;
    return hits.slice(0, MAX_ROWS);
  });
  const rows = $derived.by((): CrateItem[] => {
    if (tab === "library") return library.map((t) => dj.itemOfTrack(t));
    if (tab === "tidal") return tidal.filter((h) => h.kind === "track").map((h) => dj.itemOfHit(h));
    return dj.crate;
  });

  // the tempo of songs already analyzed, for the rows on screen
  $effect(() => {
    const paths = library.map((t) => t.path).filter((p) => !(p in bpm));
    if (!paths.length || tab !== "library") return;
    let stale = false;
    djApi
      .cachedBpm(paths)
      .then((found) => {
        if (!stale) bpm = { ...bpm, ...Object.fromEntries(paths.map((p, i) => [p, found[i] ?? null])) };
      })
      .catch(() => {});
    return () => (stale = true);
  });

  async function searchTidal() {
    const q = query.trim();
    if (!q) return;
    tab = "tidal";
    if (!app.account) {
      searchError = "Connect TIDAL in Settings to search it.";
      return;
    }
    searching = true;
    searchError = null;
    try {
      tidal = (await api.search(q)).tracks;
    } catch (e) {
      searchError = errorText(e);
    } finally {
      searching = false;
    }
  }

  const bpmOf = (item: CrateItem) => {
    const v = item.source.kind === "path" ? bpm[item.source.path] : null;
    return v ? Math.round(v) : "–";
  };
  function load(deck: DeckId, item: CrateItem) {
    dj.loadCrate(deck, item);
    if (drawer) onclose?.();
  }
  /** Double-click: the backend picks the deck (the same rule as a song picked anywhere else). */
  function loadFree(item: CrateItem) {
    if (item.source.kind === "path") api.play([item.source.path], 0);
    else api.playTidal([{ id: item.source.id, title: item.title, local: item.source.local ?? null, artist: item.artist, album: "", cover_url: item.cover, duration_ms: item.durationMs, explicit: false }], 0);
  }
</script>

<aside class="crate" class:drawer aria-label="Crate">
  <div class="top">
    <input
      class="search"
      type="search"
      placeholder="Search your library or TIDAL"
      aria-label="Search your library or TIDAL"
      bind:value={query}
      onkeydown={(e) => {
        if (e.key === "Enter") searchTidal();
        if (e.key === "Escape" && drawer) onclose?.();
      }}
    />
    <div class="seg" role="tablist" aria-label="Where the songs come from">
      {#each TABS as t (t.id)}
        <button type="button" role="tab" aria-selected={tab === t.id} class:on={tab === t.id} onclick={() => (t.id === "tidal" && query.trim() && !tidal.length ? searchTidal() : (tab = t.id))}>
          {t.label}{#if t.id === "crate" && dj.crate.length}<span class="count">{dj.crate.length}</span>{/if}
        </button>
      {/each}
    </div>
    {#if drawer}<button class="icon-btn" type="button" aria-label="Close the crate" onclick={() => onclose?.()}><Icon name="x" size={18} /></button>{/if}
  </div>

  <div class="list" role="list">
    {#if tab === "tidal" && searching}
      <p class="note">Searching TIDAL…</p>
    {:else if tab === "tidal" && searchError}
      <p class="note">{searchError}</p>
    {:else if !rows.length}
      <p class="note">
        {tab === "crate" ? "Songs you send to the crate (Settings → DJ → where a picked song goes) wait here." : tab === "tidal" ? "Type a song and press Enter to search TIDAL." : query ? "No songs match." : "Your library is empty."}
      </p>
    {/if}
    {#each rows as item (item.key)}
      <div
        class="tr"
        role="listitem"
        draggable="true"
        ondragstart={(e) => {
          dj.dragging = item;
          e.dataTransfer?.setData("text/plain", item.title);
          if (e.dataTransfer) e.dataTransfer.effectAllowed = "copy";
        }}
        ondragend={() => (dj.dragging = null)}
        ondblclick={() => loadFree(item)}
        title="Drag to a deck, or double-click to load it on the free deck"
      >
        <span class="c">{#if item.cover}<img src={item.cover} alt="" loading="lazy" />{/if}</span>
        <span class="tt">{item.title}<small>{item.artist}</small></span>
        <span class="bp" title="Beats per minute (after the song was analyzed once)">{bpmOf(item)}</span>
        <span class="ab">
          <button class="a" type="button" aria-label="Load {item.title} on deck A" title="Load this song on deck A" onclick={() => load("a", item)}>A</button>
          <button class="b" type="button" aria-label="Load {item.title} on deck B" title="Load this song on deck B" onclick={() => load("b", item)}>B</button>
          {#if tab === "crate"}<button type="button" aria-label="Remove {item.title} from the crate" title="Remove from the crate" onclick={() => dj.removeFromCrate(item.key)}><Icon name="x" size={14} /></button>{/if}
        </span>
      </div>
    {/each}
  </div>
</aside>

<style>
  .crate {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
    min-width: 0;
    padding: 10px 12px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
  }
  .crate.drawer {
    position: fixed;
    top: var(--topbar-h, 34px);
    right: 0;
    bottom: 0;
    z-index: 8;
    width: 360px;
    border-radius: var(--r-lg) 0 0 var(--r-lg);
    box-shadow: var(--shadow);
  }
  .top {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .search {
    flex: 1;
    min-width: 0;
    min-height: 40px;
    border-radius: var(--r-pill);
    padding: 0 14px;
  }
  .seg {
    display: inline-flex;
    flex: none;
    border: 1px solid var(--border);
    border-radius: var(--r-pill);
    overflow: hidden;
  }
  .seg button {
    min-height: 36px;
    padding: 0 10px;
    font-size: 12px;
    color: var(--muted);
  }
  .seg button.on {
    background: var(--surface-3);
    color: var(--text);
  }
  .count {
    margin-left: 4px;
    padding: 0 5px;
    border-radius: var(--r-pill);
    background: var(--primary-strong);
    color: var(--on-primary-strong);
    font-size: 10px;
  }
  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
  .note {
    margin: 8px 4px;
    color: var(--muted);
    font-size: 13px;
  }
  .tr {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 44px;
    padding: 0 6px;
    border-radius: var(--r-sm);
    cursor: grab;
  }
  .tr:hover {
    background: var(--surface-2);
  }
  .c {
    flex: none;
    width: 32px;
    height: 32px;
    overflow: hidden;
    border-radius: var(--r-xs);
    background: var(--surface-3);
  }
  .c img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .tt {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tt small {
    display: block;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .bp {
    flex: none;
    font: 12px var(--font-mono);
    color: var(--muted);
  }
  .ab {
    display: flex;
    gap: 4px;
  }
  .ab button {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: var(--r-xs);
    border: 1px solid var(--border);
    background: var(--surface-2);
    font-weight: 800;
  }
  .ab button:hover {
    background: var(--surface-3);
  }
  .ab .a {
    color: var(--dj-a);
  }
  .ab .b {
    color: var(--dj-b);
  }
</style>
