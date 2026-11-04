<script lang="ts">
  // What the files will be called: album artist, album, year, genre and an optional cover, asked
  // before the rip. Blank fields show what they become ("Disc 1", "Unknown artist") as the
  // placeholder. Prefilled from the disc's CD-Text when it has it; "Look it up on MusicBrainz"
  // only exists while that setting is on.
  import { open } from "@tauri-apps/plugin-dialog";
  import { UNKNOWN_ARTIST, disc } from "../../lib/disc.svelte";
  import type { DiscSource } from "../../lib/api";
  import Segmented from "../settings/Segmented.svelte";
  import Icon from "../Icon.svelte";

  const SOURCE_NOTE: Record<DiscSource, string> = {
    cdText: "Filled in from the names on the disc (CD-Text). Change anything you like.",
    musicbrainz: "Filled in from MusicBrainz. Check it, and change anything you like.",
    tidal: "Filled in from Tidal.",
    ai: "Filled in by AI. Check it before you rip.",
    manual: "",
  };
  const IMAGE_EXTENSIONS = ["jpg", "jpeg", "png", "webp"];
  const YEAR_DIGITS = 4;

  const lookupOn = $derived(disc.settings.sources.musicbrainz);
  const coverName = $derived(disc.form.coverFile?.split(/[\\/]/).pop() ?? null);

  async function pickCover() {
    const file = await open({ multiple: false, filters: [{ name: "Image", extensions: IMAGE_EXTENSIONS }] });
    if (typeof file === "string") disc.form.coverFile = file;
  }
</script>

<section class="card" aria-labelledby="ident-h">
  <header>
    <h2 id="ident-h">About this disc</h2>
    {#if lookupOn}
      <button type="button" class="btn tonal" onclick={() => disc.lookup()} disabled={disc.looking || disc.ripping}>
        {#if disc.looking}<span class="spin" aria-hidden="true"></span> Looking…{:else}<Icon name="search" size={16} /> Look it up on MusicBrainz{/if}
      </button>
    {/if}
  </header>
  <p class="hint" aria-live="polite">
    {#if disc.lookupNote}{disc.lookupNote}{:else if disc.filledFrom && SOURCE_NOTE[disc.filledFrom]}{SOURCE_NOTE[disc.filledFrom]}{:else}Type what you know. Anything you leave blank gets a plain name, shown in grey.{/if}
  </p>
  {#if disc.matches.length > 1}
    <div class="chips" role="group" aria-label="Other matches">
      {#each disc.matches as m, i (i)}
        <button type="button" class="chip" onclick={() => disc.apply(m)}>{m.album.artist} · {m.album.title}{m.album.year ? ` (${m.album.year})` : ""}</button>
      {/each}
    </div>
  {/if}

  <div class="grid">
    <label class="field wide"><span>Album artist</span><input type="text" bind:value={disc.form.artist} placeholder={UNKNOWN_ARTIST} disabled={disc.ripping} autocomplete="off" spellcheck="false" /></label>
    <label class="field wide"><span>Album</span><input type="text" bind:value={disc.form.album} placeholder={disc.placeholderAlbum} disabled={disc.ripping} autocomplete="off" spellcheck="false" /></label>
    <label class="field"><span>Year</span><input type="text" inputmode="numeric" maxlength={YEAR_DIGITS} bind:value={disc.form.year} placeholder="Optional" disabled={disc.ripping} autocomplete="off" /></label>
    <label class="field"><span>Genre</span><input type="text" bind:value={disc.form.genre} placeholder="Optional" disabled={disc.ripping} autocomplete="off" spellcheck="false" /></label>
  </div>

  <div class="row">
    <div class="label"><b>Cover</b><span class="muted">Optional. A picture from your computer; without one, the files have no cover.</span></div>
    <div class="buttons">
      {#if coverName}<span class="file ellipsis" title={disc.form.coverFile}>{coverName}</span>
        <button type="button" class="icon-btn" aria-label="Remove the cover" onclick={() => (disc.form.coverFile = null)} disabled={disc.ripping}><Icon name="x" size={16} /></button>
      {/if}
      <button type="button" class="btn tonal" onclick={pickCover} disabled={disc.ripping}><Icon name="plus" size={16} /> {coverName ? "Change" : "Choose a picture"}</button>
    </div>
  </div>

  <div class="row">
    <div class="label" id="name-songs"><b>Name the songs?</b><span class="muted">{disc.form.nameSongs ? "Type a title for each song in the list below." : "Songs are called Track 01, Track 02… You can always rename them later."}</span></div>
    <Segmented
      options={[
        { id: "yes", label: "Yes, name them" },
        { id: "no", label: "No, use numbers" },
      ]}
      value={disc.form.nameSongs ? "yes" : "no"}
      onchange={(id) => (disc.form.nameSongs = id === "yes")}
      labelledby="name-songs"
    />
  </div>
</section>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 18px 20px;
    border: 1px solid var(--border);
    border-radius: min(16px, var(--r-max));
    background: var(--surface);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
  }
  h2 {
    margin: 0;
    font-size: 16px;
  }
  .hint {
    margin: -6px 0 0;
    color: var(--muted);
    font-size: 13.5px;
    line-height: 1.5;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px 16px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
    font-weight: 600;
    font-size: 13px;
  }
  .field input {
    height: 40px;
    padding: 0 12px;
    font-size: 15px;
    font-weight: 400;
  }
  .field input::placeholder {
    color: var(--faint);
  }
  @container (max-width: 520px) {
    .grid {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
    padding-top: 14px;
    border-top: 1px solid var(--border);
  }
  .label {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    max-width: 46ch;
    font-size: 13.5px;
    line-height: 1.4;
  }
  .label b {
    font-size: 14.5px;
  }
  .buttons {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .file {
    max-width: 200px;
    color: var(--muted);
    font-size: 13px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip {
    min-height: 32px;
    padding: 0 12px;
    border-radius: min(999px, var(--r-max));
    background: var(--surface-3);
    font-size: 13px;
  }
  .spin {
    width: 14px;
    height: 14px;
    border: 2px solid var(--surface-3);
    border-top-color: var(--primary);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .spin {
      animation-duration: 3s;
    }
  }
</style>
