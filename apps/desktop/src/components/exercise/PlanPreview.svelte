<script lang="ts">
  // Preview (docs/EXERCISE_UI.md §3.3): the built course and, part by part, the song, the tempo fit and
  // the notes. The ride's playlist is saved when it starts. Hovering a part of the mountain lights its row.
  import { BUILD_STEPS, FIT_TOOLTIP, buildStepOf, clock, courseText, describeWorkout, effortWord, fitOf, partsOf, positionIcon, positionLabel, zoneName, zoneVar, type PlannedSong } from "../../lib/exercise-api";
  import { exercise } from "../../lib/exercise.svelte";
  import { app } from "../../lib/state.svelte";
  import Cover from "../Cover.svelte";
  import Icon from "../Icon.svelte";
  import Mountain from "./Mountain.svelte";
  import ZoneKey from "./ZoneKey.svelte";

  const plan = $derived(exercise.plan!);
  const segments = $derived(plan.workout.segments);
  const parts = $derived(partsOf(plan.workout));
  const total = $derived(parts.reduce((s, p) => s + p.seconds, 0));
  const songOf = (i: number): PlannedSong | undefined => plan.songs.find((s) => s.segment_index === i);
  const tolerance = $derived(exercise.settings.cadence_tolerance_rpm);
  const saved = $derived(exercise.built?.playlist ?? null);
  const ride = $derived(exercise.currentSaved);
  let renaming = $state(false);
  let draft = $state("");
  let field = $state<HTMLInputElement>();
  $effect(() => {
    if (renaming) field?.focus();
  });
  function startRename() {
    draft = ride?.name ?? "";
    renaming = true;
  }
  async function commit() {
    const name = draft.trim();
    renaming = false;
    if (ride && name) await exercise.keepRide(ride.id, name);
  }
  const SOURCES = { tag: "library tag", tidal: "TIDAL", analysis: "analyzed" } as const;

  let active = $state<number | null>(null);
  let menu = $state<number | null>(null);
  let rows = $state<HTMLElement>();

  const describe = (i: number) => {
    const seg = segments[i];
    const song = songOf(i);
    return `${zoneName(seg.kind)} · ${clock(seg.seconds)} · ${Math.round((seg.target.min_rpm + seg.target.max_rpm) / 2)} RPM${song ? ` · ${song.entry.title}` : ""}`;
  };
  function pick(i: number) {
    rows?.querySelector<HTMLElement>(`[data-row="${i}"]`)?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }
  const openSaved = () => saved && (app.view = { name: "home-playlist", id: saved.id });
  // "new songs" runs on this screen: the plan stays until the new one replaces it
  const step = $derived(buildStepOf(exercise.progress));
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && (menu = null)} onclick={() => (menu = null)} />

<div class="ex">
  <header>
    <div>
      <h1>Your ride is ready</h1>
      <p class="sub">{exercise.summary} · {plan.songs.length} {plan.songs.length === 1 ? "song" : "songs"}</p>
    </div>
    <span class="sp"></span>
    <button type="button" class="btn tonal" onclick={() => exercise.changeSetup()} disabled={exercise.rebuilding}>Change setup</button>
    <button type="button" class="btn tonal" onclick={() => exercise.rebuild()} disabled={exercise.rebuilding} title="Same ride, songs it did not play yet"><Icon name="refresh" size={16} />New songs</button>
    <button type="button" class="start" onclick={() => exercise.start()} disabled={exercise.rebuilding}><Icon name="play" size={18} />Start ride</button>
  </header>

  {#if exercise.rebuilding}
    <section class="again" aria-label="Finding new songs">
      <div class="againrow">
        <b>Finding new songs…</b>
        <span class="muted">{BUILD_STEPS[step.at].label}</span>
        <span class="sp"></span>
        <button type="button" class="btn tonal" onclick={() => exercise.cancelBuild()}>Cancel</button>
      </div>
      <div class="bar" role="progressbar" aria-label="Finding new songs" aria-valuemin="0" aria-valuemax="100" aria-valuenow={step.percent}><i style:width="{step.percent}%"></i></div>
      <span class="sr" aria-live="polite">{BUILD_STEPS[step.at].label}</span>
    </section>
  {:else if exercise.rebuildError}
    <p class="again err" role="alert"><Icon name="alert" size={16} />Could not find new songs. {exercise.rebuildError} This ride is still ready.</p>
  {/if}
  {#if plan.ai_problem}
    <p class="again err" role="alert"><Icon name="alert" size={16} />{plan.ai_problem}</p>
  {/if}

  {#if ride}
    <section class="kept" aria-label="Saved ride">
      <Icon name={ride.auto ? "save" : "check"} size={16} />
      {#if renaming}
        <form class="rename" onsubmit={(e) => (e.preventDefault(), commit())}>
          <input bind:this={field} bind:value={draft} class="field" aria-label="Ride name" maxlength={80} onkeydown={(e) => e.key === "Escape" && (e.stopPropagation(), (renaming = false))} />
          <button type="submit" class="btn tonal">Save name</button>
          <button type="button" class="link" onclick={() => (renaming = false)}>Cancel</button>
        </form>
      {:else}
        <span class="what">
          {#if ride.auto}Saved automatically{:else}Saved in My rides{/if} · <b>{ride.name}</b>
        </span>
        <span class="sp"></span>
        {#if ride.auto}<button type="button" class="btn tonal" onclick={() => exercise.keepRide(ride.id)} title="Keep it: it is never removed automatically">Keep it</button>{/if}
        <button type="button" class="btn tonal" onclick={startRename}>Rename</button>
        <button type="button" class="icon-btn star" class:on={ride.favorite} aria-pressed={ride.favorite} aria-label="Favorite" title={ride.favorite ? "Remove from favorites" : "Favorite"} onclick={() => exercise.favoriteRide(ride.id, !ride.favorite)}><Icon name={ride.favorite ? "starFill" : "star"} size={18} /></button>
      {/if}
    </section>
  {/if}

  <section class="card">
    <Mountain {parts} profile={plan.profile} mode="preview" height={160} label={courseText(parts, total, null)} {describe} bind:active onpick={pick} />
    <div class="legend"><ZoneKey kinds={parts.map((p) => p.kind)} /><span class="muted">{describeWorkout(plan.workout)}</span></div>
  </section>

  <section class="card table" bind:this={rows}>
    <div class="plan head" aria-hidden="true">
      <span></span><span>Part</span><span>Time</span><span>Pedal</span><span>Effort</span><span>Song</span><span class="beatcell">Beat</span><span></span>
    </div>
    {#each segments as seg, i}
      {@const song = songOf(i)}
      {@const fit = song ? fitOf(song, tolerance) : null}
      <div class="plan" class:hot={active === i} data-row={i} role="row" tabindex="-1" onmouseenter={() => (active = i)} onmouseleave={() => (active = null)} style:--c={zoneVar(seg.kind)}>
        <i class="dot" aria-hidden="true"></i>
        <span class="part"><b>{zoneName(seg.kind)}</b><small><Icon name={positionIcon(seg.position)} size={13} />{positionLabel(seg.position)}</small></span>
        <span class="num">{clock(seg.seconds)}</span>
        <span class="num">{seg.target.min_rpm === seg.target.max_rpm ? seg.target.min_rpm : `${seg.target.min_rpm} to ${seg.target.max_rpm}`}<small>RPM</small></span>
        <span class="num" title={effortWord(seg.effort)}>{seg.effort}/10</span>
        <span class="song">
          {#if song}
            <Cover url={song.entry.cover_url} path={song.entry.local_path} size={44} radius={6} />
            <span class="txt"><b class="ell">{song.entry.title}</b><small class="ell">{song.entry.artist}</small></span>
          {:else}
            <span class="muted">No song</span>
          {/if}
        </span>
        <span class="beatcell">
          {#if fit}<span class="beat {fit.tone}" title={FIT_TOOLTIP}>{fit.text}</span>{/if}
        </span>
        <span class="more">
          {#if song?.source}
            <button type="button" class="icon-btn" aria-label="More about this song" aria-haspopup="menu" aria-expanded={menu === i} onclick={(e) => (e.stopPropagation(), (menu = menu === i ? null : i))}><Icon name="more" size={18} /></button>
            {#if menu === i}<div class="menu" role="menu"><span role="menuitem" tabindex="-1">Tempo source: {SOURCES[song.source]}</span></div>{/if}
          {/if}
        </span>
      </div>
    {/each}
  </section>

  {#if plan.notes.length}
    <ul class="notes">
      {#each plan.notes as n}<li><span class="ntag">Note</span>{n}</li>{/each}
    </ul>
  {/if}
  {#if saved}
    <p class="saved"><button type="button" class="link" onclick={openSaved}>Saved in Playlists as “{saved.name}”</button></p>
  {/if}
</div>

<style>
  .ex {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding-bottom: 20px;
    min-width: 0;
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  h1 {
    font-size: 26px;
  }
  .sub {
    margin: 4px 0 0;
    color: var(--muted);
  }
  .sp {
    flex: 1;
  }
  .btn {
    height: 44px;
    padding: 0 18px;
  }
  .start {
    height: 56px;
    padding: 0 28px;
    border-radius: var(--r-pill);
    background: var(--primary-strong);
    color: var(--on-primary-strong);
    font-size: 16px;
    font-weight: 700;
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }
  .start:hover {
    filter: brightness(1.12);
  }
  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
    padding: 16px;
  }
  .kept {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 52px;
    padding: 6px 12px;
    border-radius: var(--r-lg);
    border: 1px dashed var(--border);
    color: var(--muted);
    font-size: 13px;
  }
  .kept b {
    color: var(--text);
  }
  .kept .btn {
    height: 36px;
    padding: 0 14px;
  }
  .kept .rename {
    display: flex;
    flex: 1;
    align-items: center;
    gap: 8px;
  }
  .kept .field {
    flex: 1;
    min-width: 0;
    min-height: 36px;
    padding: 0 12px;
    user-select: text;
  }
  .kept .star.on {
    color: var(--warn);
  }
  .legend {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    margin-top: 10px;
    flex-wrap: wrap;
    font-size: 12px;
  }
  .table {
    padding: 8px;
    container: plan-table / inline-size;
  }
  .plan {
    display: grid;
    grid-template-columns: 14px minmax(130px, 170px) 60px 84px 62px minmax(0, 1fr) minmax(190px, 250px) 36px;
    gap: 12px;
    align-items: center;
    min-height: 60px;
    padding: 6px 10px;
    border-radius: var(--r-sm);
  }
  .plan.hot {
    background: var(--surface-2);
    box-shadow: inset 3px 0 0 var(--c);
  }
  .plan.head {
    min-height: 30px;
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--c);
  }
  .part,
  .txt {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  small {
    color: var(--muted);
    font-size: 12px;
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .num {
    font-variant-numeric: tabular-nums;
    font-weight: 600;
    display: flex;
    flex-direction: column;
  }
  .num small {
    font-weight: 400;
  }
  .song {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .ell {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .beat {
    display: inline-flex;
    align-items: center;
    min-height: 24px;
    padding: 0 10px;
    border-radius: var(--r-pill);
    font-size: 12px;
    font-weight: 700;
    white-space: nowrap;
    background: var(--surface-3);
    color: var(--muted);
  }
  .beat.ok {
    background: rgb(74 222 128 / 0.16);
    color: var(--ok);
  }
  .beat.warn {
    background: var(--warn-bg);
    color: var(--warn);
  }
  .more {
    position: relative;
  }
  .icon-btn {
    width: 44px;
    height: 44px;
    margin: -4px;
  }
  .menu {
    position: absolute;
    right: 0;
    top: 100%;
    z-index: 5;
    padding: 10px 14px;
    border-radius: var(--r-sm);
    border: 1px solid var(--border);
    background: var(--surface-3);
    box-shadow: var(--shadow);
    white-space: nowrap;
    font-size: 13px;
  }
  .notes {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .notes li {
    display: flex;
    gap: 10px;
    align-items: baseline;
    color: var(--muted);
    line-height: 1.4;
  }
  .ntag {
    flex: none;
    padding: 0 8px;
    border-radius: var(--r-pill);
    background: var(--warn-bg);
    color: var(--warn);
    font-size: 11px;
    font-weight: 700;
  }
  .saved {
    margin: 0;
  }
  .link {
    color: var(--muted);
    font-size: 13px;
  }
  .link:hover {
    color: var(--text);
    text-decoration: underline;
  }
  /* (last, so it wins over the rules above) a laptop window: the beat tag moves under its song instead of squeezing it */
  @container plan-table (max-width: 920px) {
    .plan {
      grid-template-columns: 14px minmax(110px, 150px) 52px 78px 46px minmax(0, 1fr) 36px;
      gap: 4px 10px;
    }
    .plan .beatcell {
      grid-column: 6;
      grid-row: 2;
    }
    .plan.head .beatcell {
      display: none;
    }
  }
  .again {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 14px;
    border-radius: var(--r-md);
    background: var(--surface-2);
    border: 1px solid var(--border);
  }
  .again.err {
    flex-direction: row;
    align-items: center;
    margin: 0;
    color: var(--danger, var(--warn));
  }
  .againrow {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .again .bar {
    height: 6px;
    border-radius: 99px;
    background: var(--track);
    overflow: hidden;
  }
  .again .bar i {
    display: block;
    height: 100%;
    background: var(--primary);
    transition: width 0.3s;
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
