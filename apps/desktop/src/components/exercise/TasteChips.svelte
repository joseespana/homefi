<script lang="ts">
  // The taste questions under the genres: how famous, which era, and (folded away) the artists you like
  // and the ones to avoid. Every answer is a setting (saved as it changes), so the next ride starts from it.
  // Smart defaults: big hits, any era, no artists.
  import { EX_DECADES, EX_MAX_ARTIST_CHARS, EX_MAX_TASTE_ARTISTS, POPULARITIES } from "../../lib/exercise-api";
  import { exercise } from "../../lib/exercise.svelte";
  import Icon from "../Icon.svelte";
  import ExSegmented from "./ExSegmented.svelte";

  const taste = $derived(exercise.settings.taste);
  const genres = $derived(exercise.request.wish.genres.join("|"));
  const offered = $derived(exercise.suggestions.filter((a) => !taste.artists.some((n) => n.toLowerCase() === a.toLowerCase())));
  const artistsCount = $derived(taste.artists.length + taste.avoid.length);
  let open = $state(false);
  let liked = $state("");
  let avoid = $state("");

  // the suggestions follow the genres picked
  $effect(() => {
    void genres;
    exercise.loadSuggestions();
  });
  // "80s", but "2000s": "00s" or "20s" would be unclear
  const decadeLabel = (d: number) => (d < 2000 ? `${d % 100}s` : `${d}s`);
  function commit(list: "artists" | "avoid") {
    exercise.addArtist(list, list === "artists" ? liked : avoid);
    if (list === "artists") liked = "";
    else avoid = "";
  }
</script>

<div class="taste">
  <span class="lbl" id="ex-pop-l">How famous</span>
  <ExSegmented label="How famous the songs are" options={POPULARITIES} value={taste.popularity} onchange={(id) => exercise.setTaste({ popularity: id })} />

  <span class="lbl" id="ex-era-l">Era</span>
  <div class="chips" role="group" aria-labelledby="ex-era-l">
    <button type="button" class="chip" aria-pressed={taste.eras.length === 0} onclick={() => exercise.setTaste({ eras: [] })}>Any</button>
    {#each EX_DECADES as d}
      <button type="button" class="chip" aria-pressed={taste.eras.includes(d)} aria-label="{d}s" onclick={() => exercise.toggleEra(d)}>{decadeLabel(d)}</button>
    {/each}
  </div>

  <button type="button" class="fold" aria-expanded={open} onclick={() => (open = !open)}>
    <Icon name={open ? "down" : "chevron"} size={16} />
    Artists (optional){#if artistsCount}<span class="count">{artistsCount}</span>{/if}
  </button>
  {#if open}
    <div class="artists">
      <span class="lbl" id="ex-like-l">Artists you like</span>
      <div class="chips" role="group" aria-labelledby="ex-like-l">
        {#each taste.artists as a (a)}
          <button type="button" class="chip on" aria-label="Remove {a}" onclick={() => exercise.removeArtist("artists", a)}>{a}<Icon name="x" size={14} /></button>
        {/each}
        {#each offered as a (a)}
          <button type="button" class="chip offer" aria-label="Add {a}" title="From your library" onclick={() => exercise.addArtist("artists", a)}><Icon name="plus" size={14} />{a}</button>
        {/each}
      </div>
      <input class="field" bind:value={liked} maxlength={EX_MAX_ARTIST_CHARS} placeholder="Add an artist and press Enter" aria-label="Add an artist you like" disabled={taste.artists.length >= EX_MAX_TASTE_ARTISTS} onkeydown={(e) => e.key === "Enter" && commit("artists")} />

      <span class="lbl" id="ex-avoid-l">Never play</span>
      {#if taste.avoid.length}
        <div class="chips" role="group" aria-labelledby="ex-avoid-l">
          {#each taste.avoid as a (a)}
            <button type="button" class="chip on avoid" aria-label="Remove {a}" onclick={() => exercise.removeArtist("avoid", a)}>{a}<Icon name="x" size={14} /></button>
          {/each}
        </div>
      {/if}
      <input class="field" bind:value={avoid} maxlength={EX_MAX_ARTIST_CHARS} placeholder="An artist to leave out" aria-label="Add an artist to avoid" disabled={taste.avoid.length >= EX_MAX_TASTE_ARTISTS} onkeydown={(e) => e.key === "Enter" && commit("avoid")} />
    </div>
  {/if}
</div>

<style>
  .taste {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
    margin-top: 16px;
  }
  .lbl {
    display: block;
    margin-top: 6px;
    font-size: 13px;
    font-weight: 600;
    color: var(--muted);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .chip {
    min-height: 40px;
    padding: 0 14px;
    border-radius: var(--r-pill);
    border: 1px solid var(--border);
    background: var(--surface-2);
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
    transition: background 0.15s;
  }
  .chip:hover:not([aria-pressed="true"]):not(.on) {
    background: var(--surface-3);
  }
  .chip[aria-pressed="true"],
  .chip.on {
    background: var(--primary);
    color: var(--on-primary);
    border-color: transparent;
  }
  .chip.avoid {
    background: var(--surface-3);
    color: var(--text);
    border-color: var(--border);
    text-decoration: line-through;
  }
  .chip.offer {
    border-style: dashed;
  }
  .fold {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 40px;
    margin-top: 4px;
    font-weight: 600;
    color: var(--primary);
  }
  .count {
    min-width: 20px;
    height: 20px;
    padding: 0 6px;
    border-radius: var(--r-pill);
    background: var(--surface-3);
    color: var(--text);
    font-size: 12px;
    display: inline-grid;
    place-items: center;
  }
  .artists {
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 100%;
  }
  .field {
    width: 100%;
    min-height: 44px;
    padding: 0 14px;
    user-select: text;
  }
</style>
