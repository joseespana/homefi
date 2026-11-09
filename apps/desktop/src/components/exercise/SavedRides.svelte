<script lang="ts">
  // "My rides" (setup's second tab): every ride HomeFi built is saved by itself, so one that was
  // liked can be ridden again, with the same songs, without the AI or a build. Favorites come first.
  // The list and the actions are in lib/exercise.svelte.ts; the files are the core's (.homefiride).
  import { STYLE_INFO, type SavedRide } from "../../lib/exercise-api";
  import { exercise } from "../../lib/exercise.svelte";
  import { ago } from "../../lib/transfer";
  import Icon from "../Icon.svelte";
  import MiniProfile from "./MiniProfile.svelte";

  const rides = $derived(exercise.rides);
  const history = $derived(exercise.settings.ride_history_max);

  /** The ride whose name is being edited, and what is typed. */
  let renaming = $state<string | null>(null);
  let draft = $state("");
  /** The ride waiting for "Delete" to be confirmed. */
  let deleting = $state<string | null>(null);
  let field = $state<HTMLInputElement>();
  let cancelDelete = $state<HTMLButtonElement>();

  $effect(() => {
    if (renaming && field) {
      field.focus();
      field.select();
    }
  });
  $effect(() => {
    if (deleting) cancelDelete?.focus();
  });

  function startRename(r: SavedRide) {
    deleting = null;
    renaming = r.id;
    draft = r.name;
  }
  async function commitRename(r: SavedRide) {
    const name = draft.trim();
    renaming = null;
    if (name && name !== r.name) await exercise.renameRide(r.id, name);
  }
  async function confirmDelete(r: SavedRide) {
    deleting = null;
    await exercise.deleteRide(r.id);
  }

  const meta = (r: SavedRide) => [`${r.minutes} min`, STYLE_INFO[r.style]?.name ?? r.style, r.genres.length ? r.genres.join(" + ") : "Any music", `${r.songs} ${r.songs === 1 ? "song" : "songs"}`].join(" · ");
  const ridden = (r: SavedRide) => (r.times_ridden ? `Ridden ${r.times_ridden} ${r.times_ridden === 1 ? "time" : "times"} · last ${ago(r.last_ridden_ms)}` : `Not ridden yet · built ${ago(r.created_ms)}`);
  const busy = $derived(exercise.step === "building");
</script>

<section class="rides" aria-label="My rides">
  <div class="top">
    <p class="sub">Every ride HomeFi builds is saved here. Ride one again with the same songs, no waiting and no AI.</p>
    <button type="button" class="btn tonal" onclick={() => exercise.importRide()}><Icon name="download" size={16} />Import a ride</button>
  </div>

  {#if rides.length === 0}
    <div class="empty">
      <span class="mark" aria-hidden="true"><Icon name="bike" size={30} /></span>
      <h2>No rides yet</h2>
      <p>Build a ride and it is saved here by itself. You can also import one a friend sent you.</p>
      <div class="acts">
        <button type="button" class="btn primary" onclick={() => (exercise.setupTab = "new")}>Plan a ride</button>
        <button type="button" class="btn tonal" onclick={() => exercise.importRide()}>Import a ride</button>
      </div>
    </div>
  {:else}
    <ul class="list">
      {#each rides as r (r.id)}
        <li class="row" class:fav={r.favorite}>
          <MiniProfile profile={r.profile} />
          <div class="info">
            {#if renaming === r.id}
              <form class="rename" onsubmit={(e) => (e.preventDefault(), commitRename(r))}>
                <input bind:this={field} bind:value={draft} class="field" aria-label="Ride name" maxlength={80} onkeydown={(e) => e.key === "Escape" && (e.stopPropagation(), (renaming = null))} />
                <button type="submit" class="icon-btn" aria-label="Save the name" title="Save the name"><Icon name="check" size={18} /></button>
                <button type="button" class="icon-btn" aria-label="Cancel renaming" title="Cancel" onclick={() => (renaming = null)}><Icon name="x" size={18} /></button>
              </form>
            {:else}
              <div class="name">
                <b class="ell" title={r.name}>{r.name}</b>
                {#if r.auto}<span class="badge" title="Saved by itself. It is removed after {history} automatic rides unless you keep or star it.">Auto</span>{/if}
              </div>
              <span class="meta ell" title={meta(r)}>{meta(r)}</span>
              <span class="meta ell">{ridden(r)}</span>
            {/if}
          </div>

          {#if deleting === r.id}
            <div class="confirm" role="alertdialog" aria-label="Delete {r.name}?">
              <span>Delete this ride?</span>
              <button type="button" class="btn danger" onclick={() => confirmDelete(r)}>Delete</button>
              <button type="button" class="btn tonal" bind:this={cancelDelete} onclick={() => (deleting = null)} onkeydown={(e) => e.key === "Escape" && (deleting = null)}>Cancel</button>
            </div>
          {:else}
            <div class="actions">
              <button type="button" class="ride" disabled={busy} onclick={() => exercise.loadRide(r.id)}><Icon name="play" size={15} />Ride again</button>
              <button type="button" class="icon-btn star" class:on={r.favorite} aria-pressed={r.favorite} aria-label="Favorite" title={r.favorite ? "Remove from favorites" : "Favorite: kept and listed first"} onclick={() => exercise.favoriteRide(r.id, !r.favorite)}><Icon name={r.favorite ? "starFill" : "star"} size={18} /></button>
              {#if r.auto}<button type="button" class="icon-btn" aria-label="Keep this ride" title="Keep it: it is never removed automatically" onclick={() => exercise.keepRide(r.id)}><Icon name="save" size={18} /></button>{:else}<span class="slot" aria-hidden="true"></span>{/if}
              <button type="button" class="icon-btn" aria-label="Rename" title="Rename" onclick={() => startRename(r)}><Icon name="edit" size={18} /></button>
              <button type="button" class="icon-btn" aria-label="Export to a file" title="Export to a file (.homefiride): send it, or save it in your Google Drive folder" onclick={() => exercise.exportRide(r)}><Icon name="upload" size={18} /></button>
              <button type="button" class="icon-btn" aria-label="Delete" title="Delete" onclick={() => ((renaming = null), (deleting = r.id))}><Icon name="trash" size={18} /></button>
            </div>
          {/if}
        </li>
      {/each}
    </ul>
    <p class="foot muted">Automatic rides are cleaned up after {history}; kept and favorite rides never are. Change it in Settings → Exercise.</p>
  {/if}
</section>

<style>
  .rides {
    container-type: inline-size;
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-width: 0;
  }
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }
  .sub {
    margin: 0;
    color: var(--muted);
  }
  .btn {
    min-height: 44px;
    padding: 0 16px;
    flex: none;
  }
  .btn.danger {
    color: var(--danger);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--danger) 55%, transparent);
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .row {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 14px;
    padding: 12px 14px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
  }
  .row.fav {
    border-color: color-mix(in srgb, var(--primary) 45%, var(--border));
  }
  .info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .name {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    font-size: 15px;
  }
  .meta {
    color: var(--muted);
    font-size: 13px;
  }
  .ell {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .badge {
    flex: none;
    padding: 0 8px;
    min-height: 20px;
    display: inline-flex;
    align-items: center;
    border-radius: var(--r-pill);
    background: var(--surface-3);
    color: var(--muted);
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .actions,
  .confirm {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .confirm {
    gap: 8px;
    font-size: 13px;
    color: var(--muted);
  }
  .ride {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 44px;
    padding: 0 18px;
    margin-right: 6px;
    border-radius: var(--r-pill);
    background: var(--primary-strong);
    color: var(--on-primary-strong);
    font-weight: 700;
  }
  .ride:hover:not(:disabled) {
    filter: brightness(1.12);
  }
  .ride:disabled {
    opacity: 0.5;
  }
  .icon-btn {
    width: 40px;
    height: 40px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    color: var(--muted);
  }
  .icon-btn:hover {
    background: var(--surface-3);
    color: var(--text);
  }
  .slot {
    width: 40px;
    height: 40px;
  }
  .star.on {
    color: var(--warn);
  }
  .rename {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .field {
    flex: 1;
    min-width: 0;
    min-height: 40px;
    padding: 0 12px;
    user-select: text;
  }
  .foot {
    margin: 2px 0 0;
    font-size: 12px;
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    max-width: 440px;
    margin: 36px auto 0;
    text-align: center;
  }
  .empty h2 {
    margin: 4px 0 0;
    font-size: 20px;
  }
  .empty p {
    margin: 0;
    color: var(--muted);
    line-height: 1.5;
  }
  .mark {
    display: grid;
    place-items: center;
    width: 64px;
    height: 64px;
    border-radius: 50%;
    background: var(--surface-3);
    color: var(--primary);
  }
  .acts {
    display: flex;
    gap: 10px;
    margin-top: 8px;
  }
  /* a laptop window: the buttons go under the name instead of squeezing it */
  @container (max-width: 760px) {
    .row {
      grid-template-columns: auto minmax(0, 1fr);
    }
    .actions,
    .confirm {
      grid-column: 1 / -1;
      justify-content: flex-end;
    }
    .ride {
      margin-right: auto;
    }
  }
</style>
