<script lang="ts">
  // The ride (docs/EXERCISE_UI.md §3.4): one question at a glance each, by size: how fast to pedal (the
  // ring, with the beat inside it), how long until it changes (the timer), how hard (the bars), what is
  // next (the card) and where we are (the mountain). Dark whatever the theme when the setting says so.
  import { STEP_CHIP_MS, clock, courseText, effortWord, partsOf, positionIcon, positionLabel, shownLabel, stepChipText, zoneName, zoneVar } from "../../lib/exercise-api";
  import { exercise } from "../../lib/exercise.svelte";
  import Cover from "../Cover.svelte";
  import Icon from "../Icon.svelte";
  import EffortBars from "./EffortBars.svelte";
  import EndButton from "./EndButton.svelte";
  import Mountain from "./Mountain.svelte";
  import NextCard from "./NextCard.svelte";
  import PulseRing from "./PulseRing.svelte";
  import RideLyrics from "./RideLyrics.svelte";
  import TvButton from "./TvButton.svelte";

  /** The pause card appears once the music has been still this long (a song loading is not a pause). */
  const PAUSE_SHOW_MS = 500;

  const plan = $derived(exercise.plan);
  const view = $derived(exercise.view);
  const parts = $derived(plan ? partsOf(plan.workout) : []);
  const s = $derived(exercise.settings);

  // interval phases flip the colour: GO! in the sprint colour, Rest in the recovery colour
  const work = $derived(view?.interval?.phase === "work");
  const rest = $derived(view?.interval?.phase === "rest");
  const color = $derived(!view ? "var(--primary)" : rest ? "var(--z-recov)" : zoneVar(view.kind));
  const song = $derived(view && plan ? plan.songs.find((x) => x.segment_index === view.segment_index)?.entry : undefined);
  const timer = $derived(view ? (view.interval ? view.interval.phase_remaining_s : view.remaining_s) : 0);
  const lastPart = $derived(!!view && view.segment_index >= view.segment_count - 1);
  const label = $derived(view ? courseText(parts, view.total_s, view.segment_index, view.elapsed_s) : "Course");
  const rpmLabel = $derived(
    view ? (view.shown && view.shown.unit !== "rpm" ? `Target ${shownLabel(view.shown, view.target_rpm)}, about ${view.target_rpm} RPM` : `Target ${view.target_rpm} RPM`) : "",
  );

  // the resistance-step chip: shown for a few seconds after each step (the "resistance" cue sounds with it)
  const chipText = $derived(stepChipText(view?.step));
  let chip = $state("");
  $effect(() => {
    chip = chipText;
    if (!chipText) return;
    const id = setTimeout(() => (chip = ""), STEP_CHIP_MS);
    return () => clearTimeout(id);
  });

  // swapping the song: the song line asks first; "Swap the next one" when there is one
  let askSwap = $state(false);
  const nextSong = $derived(view && plan ? plan.songs.find((x) => x.segment_index === view.segment_index + 1)?.entry : undefined);
  const swapRange = $derived(view ? shownLabel(view.shown, view.target_rpm) : "");
  async function doSwap(next: boolean) {
    if (!view) return;
    askSwap = false;
    // the part's own clock goes on: the new song starts at this instant of the part
    if (next) await exercise.swap(view.segment_index + 1, null);
    else await exercise.swap(view.segment_index, Math.max(0, Math.round((view.segment_seconds - view.remaining_s) * 1000)));
  }

  const tvOwnsMusic = $derived(exercise.tv.hub && exercise.tv.audio === "tv");

  // the pause card: after the music has been still a moment
  let paused = $state(false);
  const finished = $derived(!!view?.finished);
  $effect(() => {
    if (exercise.playing || finished) {
      paused = false;
      return;
    }
    const id = setTimeout(() => (paused = true), PAUSE_SHOW_MS);
    return () => clearTimeout(id);
  });

  function onKey(e: KeyboardEvent) {
    const t = e.target as HTMLElement;
    if (t.closest("input, textarea, select")) return;
    if (e.code === "Space" && !t.closest("button, summary")) {
      e.preventDefault();
      exercise.pause();
    } else if (e.key.toLowerCase() === "n" && !e.metaKey && !e.ctrlKey && !e.altKey && !lastPart) {
      exercise.skip();
    } else if (e.key.toLowerCase() === "f" && !e.metaKey && !e.ctrlKey && !e.altKey) {
      exercise.setFullscreen(!exercise.fullscreen);
    } else if (e.key === "Escape" && paused) {
      exercise.pause();
    } else if (e.key === "Escape" && exercise.fullscreen) {
      exercise.setFullscreen(false);
    }
  }
  // leaving the ride (it ended, or another page opened) leaves fullscreen: the app around it comes back
  $effect(() => () => {
    if (exercise.fullscreen) exercise.setFullscreen(false);
  });

  /** Moves focus into a dialog when it opens. */
  const focusFirst = (node: HTMLElement) => node.focus();
  let resume = $state<HTMLButtonElement>();
  $effect(() => {
    if (paused) resume?.focus();
  });
</script>

<svelte:window onkeydown={onKey} />

{#if view && plan}
  <main class="ride" class:full={exercise.fullscreen} class:ex-dark={exercise.dark} style:--c={color} aria-label="Ride">
    <header class="top">
      <span class="zchip"><Icon name="bike" size={22} />{zoneName(view.kind)}</span>
      <span class="pos"><Icon name={positionIcon(view.position)} size={20} />{positionLabel(view.position)}</span>
      <span class="sp"></span>
      {#if song}
        {#if s.ride_swap}
          <button type="button" class="song swap" aria-label="Swap this song: {song.title}" aria-haspopup="dialog" disabled={exercise.swapping !== null} onclick={() => (askSwap = true)}>
            <Cover url={song.cover_url} path={song.local_path} size={44} radius={6} />
            <span class="txt"><b class="ell">{song.title}</b><small class="ell">{exercise.swapping !== null ? "Finding another song…" : song.artist}</small></span>
            <Icon name="refresh" size={16} />
          </button>
        {:else}
          <span class="song">
            <Cover url={song.cover_url} path={song.local_path} size={44} radius={6} />
            <span class="txt"><b class="ell">{song.title}</b><small class="ell">{song.artist}</small></span>
          </span>
        {/if}
      {/if}
      {#if tvOwnsMusic}
        <span class="tvnote">Music is playing on the TV<button type="button" class="link" onclick={() => exercise.playHere()}>Play here instead</button></span>
      {/if}
      <TvButton />
      <button type="button" class="skip" title={exercise.fullscreen ? "Leave fullscreen [F]" : "Fullscreen [F]"} aria-label={exercise.fullscreen ? "Leave fullscreen" : "Fullscreen"} aria-pressed={exercise.fullscreen} onclick={() => exercise.setFullscreen(!exercise.fullscreen)}>
        <Icon name={exercise.fullscreen ? "shrink" : "expand"} size={18} />
      </button>
      <button type="button" class="pause" title="Pause the ride [Space]" aria-label={exercise.playing ? "Pause" : "Resume"} onclick={() => exercise.pause()}>
        <Icon name={exercise.playing ? "pause" : "play"} size={24} />
      </button>
      <button type="button" class="skip" title="Skip to the next part [N]" aria-label="Skip to the next part" disabled={lastPart} onclick={() => exercise.skip()}><Icon name="next" size={18} /></button>
      <EndButton onend={() => exercise.stop()} />
    </header>

    <section class="main">
      <div class="left"><EffortBars effort={view.effort} level={view.level ?? null} levelCount={view.level_count ?? 0} /></div>

      <div class="mid">
        {#if view.interval}
          <div class="phase" class:work class:rest>
            <b>{work ? "GO!" : "Rest"}</b>
            <span>Round {view.interval.round} / {view.interval.rounds}</span>
          </div>
        {/if}
        <PulseRing {view} pulseStyle={s.pulse_style} offsetMs={s.pulse_offset_ms} playing={exercise.playing} countdown={exercise.countdown} color={work ? "var(--z-sprint)" : color} label={rpmLabel} />
        <p class="pace">Pedal when the ring closes.</p>
      </div>

      <div class="right">
        <span class="lbl">{view.interval ? (work ? "Time left in this push" : "Time left in this rest") : "Time left in this part"}</span>
        <div class="timer" role="timer" aria-live="off" aria-label="{effortWord(view.effort)} part, {clock(timer)} left">{clock(timer)}</div>
        {#if view.interval}<small class="sub">{clock(view.remaining_s)} left in this part</small>{/if}
        {#if view.next}
          <NextCard next={view.next} notice={view.notice} soonIn={view.remaining_s} />
        {:else}
          <div class="last">Last part. Almost there.</div>
        {/if}
      </div>
    </section>

    <section class="map">
      <div class="mtnbox">
        {#if s.ride_karaoke}
          <RideLyrics />
        {:else}
          <Mountain
            {parts}
            profile={plan.profile}
            mode="live"
            height="100%"
            current={view.segment_index}
            marker={view.marker}
            hot={exercise.hotPoints}
            paused={!exercise.playing}
            {label}
            describe={(i) => `${zoneName(parts[i].kind)} · ${clock(parts[i].seconds)} · ${Math.round((plan.workout.segments[i].target.min_rpm + plan.workout.segments[i].target.max_rpm) / 2)} RPM`}
          />
        {/if}
        <button
          type="button"
          class="kmic"
          class:on={s.ride_karaoke}
          aria-pressed={s.ride_karaoke}
          title={s.ride_karaoke ? "Show the mountain" : "Show the lyrics (karaoke)"}
          aria-label={s.ride_karaoke ? "Show the mountain" : "Show the lyrics"}
          onclick={() => exercise.saveSettings({ ride_karaoke: !s.ride_karaoke })}
        >
          <Icon name={s.ride_karaoke ? "mountain" : "mic"} size={20} />
        </button>
        {#if chip}<span class="chip" role="status" aria-live="polite"><Icon name="up" size={16} />{chip}</span>{/if}
      </div>
      <div class="prog">
        <span>{clock(view.elapsed_s)}</span>
        <div class="bar" role="progressbar" aria-label="Ride progress" aria-valuemin="0" aria-valuemax={view.total_s} aria-valuenow={view.elapsed_s}><i style:width="{(view.progress * 100).toFixed(2)}%"></i></div>
        <span>{clock(view.total_s)}</span>
      </div>
    </section>

    {#if askSwap}
      <div class="veil" role="presentation" onclick={() => (askSwap = false)}>
        <div class="dlg" role="dialog" aria-modal="true" aria-labelledby="ex-swap-t" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.key === "Escape" && (askSwap = false)}>
          <h2 id="ex-swap-t">Swap this song?</h2>
          <p>HomeFi will put another that fits {swapRange}. The part's clock keeps going.</p>
          <div class="acts">
            <button type="button" class="resume" use:focusFirst onclick={() => doSwap(false)}>Swap</button>
            <button type="button" class="btn tonal" onclick={() => (askSwap = false)}>Keep</button>
            {#if nextSong}<button type="button" class="btn tonal" onclick={() => doSwap(true)}>Swap the next one</button>{/if}
          </div>
        </div>
      </div>
    {/if}

    {#if paused}
      <div class="veil" role="presentation">
        <div class="dlg" role="alertdialog" aria-modal="true" aria-labelledby="ex-paused-t" aria-describedby="ex-paused-d">
          <h2 id="ex-paused-t">Paused</h2>
          <p id="ex-paused-d">The music and the timer are stopped.</p>
          <p class="where"><b style:color={color}>{zoneName(view.kind)}</b> · {clock(timer)} left</p>
          <div class="acts">
            <button type="button" class="resume" bind:this={resume} onclick={() => exercise.pause()}><Icon name="play" size={18} />Resume</button>
            <button type="button" class="btn tonal" onclick={() => exercise.stop()}>End ride</button>
          </div>
        </div>
      </div>
    {/if}
  </main>
{:else}
  <p class="muted">Getting the ride ready…</p>
{/if}

<style>
  /* fullscreen: the ride covers the whole window (rail, top bar and all), the window itself fills the screen */
  .ride.full {
    position: fixed;
    inset: 0;
    z-index: 40;
    border-radius: 0;
  }
  .ride {
    /* the layout follows the room this screen gets (rail, side panels), not the window */
    container: ride / inline-size;
    position: relative;
    display: grid;
    /* the ring gets the larger share, the map the rest (never under 190 px: the rider and the step tags need height) */
    grid-template-rows: auto minmax(0, 1.5fr) minmax(190px, 1fr);
    gap: 12px;
    flex: 1;
    min-height: 0;
    min-width: 0;
    padding: 14px;
    border-radius: var(--r-lg);
    background: var(--bg);
    color: var(--text);
    overflow: hidden;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .zchip {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-height: var(--ride-hit);
    padding: 0 18px;
    border-radius: var(--r-pill);
    background: var(--c);
    color: var(--z-ink);
    font-size: 20px;
    font-weight: 800;
    white-space: nowrap;
  }
  .pos {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: var(--ride-hit);
    padding: 0 16px;
    border-radius: var(--r-pill);
    border: 2px solid var(--c);
    font-weight: 700;
    font-size: 16px;
    white-space: nowrap;
  }
  .sp {
    flex: 1;
  }
  .song {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    max-width: 280px;
  }
  .txt {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .txt small {
    color: var(--muted);
  }
  .ell {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tvnote {
    font-size: 13px;
    color: var(--muted);
    display: flex;
    gap: 8px;
    white-space: nowrap;
  }
  .link {
    color: var(--primary);
    font-weight: 600;
  }
  .pause {
    width: 56px;
    height: 56px;
    border-radius: 50%;
    background: var(--primary);
    color: var(--on-primary);
    display: grid;
    place-items: center;
    flex: none;
  }
  .pause:hover {
    filter: brightness(1.1);
  }
  button.swap {
    padding: 4px 10px 4px 4px;
    border-radius: var(--r-md);
    color: inherit;
    text-align: left;
    background: transparent;
  }
  button.swap:hover:not(:disabled) {
    background: var(--surface-2);
  }
  .skip {
    width: var(--ride-hit);
    height: var(--ride-hit);
    border-radius: 50%;
    border: 1px solid var(--border);
    background: var(--surface-2);
    display: grid;
    place-items: center;
  }
  .skip:hover:not(:disabled) {
    background: var(--surface-3);
  }
  /* the middle row sizes the ring: it is as big as the row allows (380 px at most) */
  .main {
    /* side columns; the ring takes what is left between them (see .mid) */
    --l: 300px;
    --r: 340px;
    --ph: 0px;
    display: grid;
    grid-template-columns: var(--l) minmax(0, 1fr) var(--r);
    align-items: center;
    gap: 12px;
    min-height: 0;
    container-type: size;
  }
  /* GO! / Rest above the ring takes height from it */
  .main:has(.phase) {
    --ph: 58px;
  }
  .left {
    justify-self: start;
    padding-left: 8px;
  }
  .mid {
    /* as big as the row allows: its height (less the pace line and GO!) and the room between the side columns */
    --ride-ring: clamp(150px, min(100cqh - 84px - var(--ph), 100cqw - var(--l) - var(--r) - 48px), 380px);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .phase {
    /* clears the ring even at the top of its pulse (it swells 7 %) */
    margin-bottom: 18px;
    display: flex;
    align-items: baseline;
    gap: 12px;
    color: var(--c);
  }
  .phase b {
    font-size: 34px;
    font-weight: 900;
    letter-spacing: 0.04em;
  }
  .phase.work b {
    color: var(--z-sprint);
  }
  .phase.rest b {
    color: var(--z-recov);
  }
  .phase span {
    white-space: nowrap;
    color: var(--muted);
    font-weight: 700;
    font-size: 16px;
  }
  .pace {
    margin: 0;
    color: var(--muted);
    font-size: 14px;
  }
  .right {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }
  .lbl {
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .timer {
    /* four digits fit the right column at any size of it */
    font-size: clamp(44px, calc(var(--r) * 0.28), 96px);
    font-weight: 800;
    line-height: 1;
    font-variant-numeric: tabular-nums;
    letter-spacing: -0.02em;
  }
  .sub {
    color: var(--muted);
    font-size: 14px;
    margin-top: -4px;
  }
  .last {
    padding: 12px 14px;
    border: 2px dashed var(--border);
    border-radius: var(--r-md);
    color: var(--muted);
    font-weight: 600;
  }
  .mtnbox {
    position: relative;
    flex: 1;
    min-height: 0;
  }
  /* the karaoke switch: over the map, top left */
  .kmic {
    position: absolute;
    top: 4px;
    left: 4px;
    z-index: 1;
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: 50%;
    border: 1px solid var(--border);
    background: color-mix(in srgb, var(--surface) 70%, transparent);
    color: var(--text);
    cursor: pointer;
  }
  .kmic.on {
    border-color: var(--c);
    background: color-mix(in srgb, var(--c) 20%, transparent);
    color: var(--c);
  }
  .kmic:focus-visible {
    outline: 2px solid var(--c);
    outline-offset: 2px;
  }
  /* the resistance-step chip: over the mountain, top centre (no layout space, nothing jumps) */
  .chip {
    position: absolute;
    top: 8px;
    left: 50%;
    transform: translateX(-50%);
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 32px;
    padding: 0 12px;
    border-radius: var(--r-pill);
    border: 2px solid var(--c);
    background: color-mix(in srgb, var(--c) 14%, color-mix(in srgb, var(--surface) 92%, transparent));
    color: var(--c);
    font-size: 14px;
    font-weight: 700;
    white-space: nowrap;
    pointer-events: none;
    animation: ex-chip 0.15s ease-out;
  }
  @keyframes ex-chip {
    from {
      opacity: 0;
      transform: translate(-50%, -4px);
    }
  }
  :global(:root[data-motion="off"]) .chip {
    animation: none;
  }
  .map {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
    padding: 14px 16px 12px;
  }
  .prog {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: 10px;
    font-size: 13px;
    font-weight: 700;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .bar {
    flex: 1;
    height: 8px;
    border-radius: var(--r-pill);
    background: var(--track);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    background: var(--c);
    border-radius: inherit;
    transition: width 0.4s linear;
  }
  .veil {
    position: absolute;
    inset: 0;
    z-index: 10;
    display: grid;
    place-items: center;
    background: rgb(5 10 18 / 0.78);
  }
  .dlg {
    width: 460px;
    max-width: calc(100% - 32px);
    padding: 28px;
    border-radius: var(--r-lg);
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
    text-align: center;
  }
  .dlg h2 {
    margin: 0 0 6px;
    font-size: 32px;
  }
  .dlg p {
    margin: 0;
    color: var(--muted);
  }
  .where {
    margin-top: 10px !important;
    color: var(--text) !important;
    font-size: 16px;
  }
  .acts {
    display: flex;
    justify-content: center;
    gap: 12px;
    margin-top: 22px;
  }
  .resume {
    height: 56px;
    padding: 0 32px;
    border-radius: var(--r-pill);
    background: var(--primary);
    color: var(--on-primary);
    font-weight: 800;
    font-size: 17px;
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }
  .acts .btn {
    height: 56px;
    padding: 0 22px;
  }
  /* the narrower layouts last, so they win over the rules above */
  @container ride (max-width: 1080px) {
    .main {
      --l: 200px;
      --r: 250px;
    }
    .left {
      padding-left: 0;
    }
    .left :global(.bars i) {
      width: 14px;
      height: 26px;
    }
    .left :global(.word) {
      font-size: 22px;
    }
    .left :global(.hint) {
      display: none;
    }
  }
  /* a laptop window with the rail: everything a size down, the song keeps its cover only */
  @container ride (max-width: 820px) {
    .main {
      --l: 150px;
      --r: 190px;
      gap: 8px;
    }
    .lbl {
      font-size: 11px;
    }
    .left :global(.bars i) {
      width: 10px;
      height: 22px;
    }
    .left :global(.bars) {
      gap: 3px;
    }
    .zchip {
      font-size: 17px;
      padding: 0 14px;
    }
    .pos {
      font-size: 14px;
      padding: 0 12px;
    }
    .song .txt {
      display: none;
    }
    .phase b {
      font-size: 28px;
    }
  }
</style>
