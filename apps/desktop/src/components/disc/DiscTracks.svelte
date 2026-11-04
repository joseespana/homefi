<script lang="ts">
  // The tracks of the disc: pick which to rip and, when the user chose to name the songs, a title
  // field per song (blank shows "Track 01" in grey). Rows are focusable: Space toggles, Enter edits.
  import { formatTime } from "../../lib/api";
  import { disc, trackMs, trackName } from "../../lib/disc.svelte";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";

  const tracks = $derived(disc.current?.toc.tracks ?? []);
  const all = $derived(tracks.length > 0 && disc.selection.size === tracks.length);
  const some = $derived(disc.selection.size > 0 && !all);
  const inLibrary = $derived(new Set(app.tracks.map((t) => t.isrc).filter((i): i is string => !!i)));
  const naming = $derived(disc.form.nameSongs);
  let master = $state<HTMLInputElement>();
  $effect(() => {
    if (master) master.indeterminate = some;
  });

  function onKey(e: KeyboardEvent, n: number) {
    if (e.target !== e.currentTarget) return;
    if (e.key === " ") {
      e.preventDefault();
      disc.toggleTrack(n);
    } else if (e.key === "Enter" && naming) {
      e.preventDefault();
      (e.currentTarget as HTMLElement).querySelector<HTMLInputElement>("input[type=text]")?.focus();
    }
  }
</script>

<section class="card" aria-labelledby="tracks-h">
  <header>
    <h2 id="tracks-h">Songs</h2>
    <label class="all"><input type="checkbox" bind:this={master} checked={all} onchange={(e) => disc.selectAll(e.currentTarget.checked)} disabled={disc.ripping} /> {disc.selection.size} of {tracks.length} selected</label>
  </header>
  <ul>
    {#each tracks as t (t.number)}
      {@const isrc = disc.isrcs[t.number - 1]}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
      <li class:off={!disc.selection.has(t.number)} tabindex="0" onkeydown={(e) => onKey(e, t.number)}>
        <input type="checkbox" checked={disc.selection.has(t.number)} onchange={() => disc.toggleTrack(t.number)} disabled={disc.ripping} aria-label="Rip song {t.number}" />
        <span class="num">{String(t.number).padStart(2, "0")}</span>
        {#if naming}
          <input type="text" class="title" bind:value={disc.form.titles[t.number - 1]} placeholder={trackName(t.number)} disabled={disc.ripping} aria-label="Title of song {t.number}" autocomplete="off" spellcheck="false" />
        {:else}
          <span class="title fixed">{trackName(t.number)}</span>
        {/if}
        <span class="isrc">
          {#if isrc}{isrc}{:else if !disc.isrcs.length}<span class="skeleton" aria-hidden="true"></span>{/if}
          {#if isrc && inLibrary.has(isrc)}<span class="have" title="Already in your library"><Icon name="check" size={13} /></span>{/if}
        </span>
        <span class="len">{formatTime(trackMs(t.startLba, t.endLba))}</span>
      </li>
    {/each}
  </ul>
</section>

<style>
  .card {
    padding: 12px 8px 8px;
    border: 1px solid var(--border);
    border-radius: min(16px, var(--r-max));
    background: var(--surface);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 12px 8px;
  }
  h2 {
    margin: 0;
    font-size: 16px;
  }
  .all {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 32px;
    color: var(--muted);
    font-size: 13px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li {
    display: grid;
    grid-template-columns: 28px 28px minmax(0, 1fr) auto 52px;
    align-items: center;
    gap: 10px;
    min-height: 44px;
    padding: 2px 12px;
    border-radius: min(10px, var(--r-max));
  }
  li:hover {
    background: var(--surface-2);
  }
  li.off {
    opacity: 0.55;
  }
  input[type="checkbox"] {
    width: 18px;
    height: 18px;
    accent-color: var(--primary-strong);
  }
  .num {
    color: var(--faint);
    font-variant-numeric: tabular-nums;
  }
  .title {
    min-width: 0;
  }
  input.title {
    height: 34px;
    padding: 0 10px;
  }
  input.title::placeholder {
    color: var(--faint);
  }
  .fixed {
    color: var(--muted);
  }
  .isrc {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--faint);
    font: 12px var(--font-mono);
  }
  .skeleton {
    width: 96px;
    height: 10px;
    border-radius: min(5px, var(--r-max));
    background: var(--surface-3);
    animation: pulse 1.4s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.45;
    }
  }
  .have {
    color: var(--ok);
  }
  .len {
    text-align: right;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  @container (max-width: 560px) {
    li {
      grid-template-columns: 28px 28px minmax(0, 1fr) 52px;
    }
    .isrc {
      display: none;
    }
  }
</style>
