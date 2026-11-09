<script lang="ts">
  // Building (docs/EXERCISE_UI.md §3.2): the ghost of the course, four steps from the builder's progress,
  // an overall bar and Cancel. Leaving the screen does not stop it. A failed build lands here too, with
  // the reason and a way back.
  import { sketchOf, type RideProgress } from "../../lib/exercise-api";
  import { exercise } from "../../lib/exercise.svelte";
  import Icon from "../Icon.svelte";
  import ExCard from "./ExCard.svelte";
  import Mountain from "./Mountain.svelte";

  type StepId = RideProgress["kind"];
  const STEPS: { id: StepId; label: string }[] = [
    { id: "searching", label: "Searching songs" },
    { id: "resolving", label: "Finding them on TIDAL and in your library" },
    { id: "analyzing", label: "Checking the tempo" },
    { id: "matching", label: "Matching songs to each part" },
  ];
  /** Where each step starts on the overall bar (%), and how much of it the step takes. */
  const SPAN: Record<StepId, [number, number]> = { searching: [4, 14], resolving: [18, 40], analyzing: [58, 30], matching: [90, 8] };

  const p = $derived(exercise.progress);
  const at = $derived(p ? STEPS.findIndex((s) => s.id === p.kind) : 0);
  const fraction = $derived(p && "done" in p && p.total > 0 ? p.done / p.total : 0);
  const percent = $derived(Math.min(99, Math.round(SPAN[STEPS[at].id][0] + SPAN[STEPS[at].id][1] * fraction)));
  const count = (id: StepId): string => {
    if (!p || p.kind !== id) return "";
    if (p.kind === "resolving") return `${p.done} of ${p.total}`;
    if (p.kind === "analyzing") return `Song ${Math.min(p.done + 1, p.total)} of ${p.total}`;
    return "";
  };
  const sketch = $derived(sketchOf(exercise.request.style, exercise.request.minutes));
  const failed = $derived(!!exercise.buildError && !exercise.building);
</script>

<div class="ex">
  <header>
    <h1>{failed ? "Could not build the ride" : "Building your ride"}</h1>
    <p class="sub">{exercise.summary}</p>
  </header>

  <ExCard>
    <Mountain parts={sketch.parts} profile={sketch.profile} mode="ghost" height={160} label="Sketch of the course" />

    {#if failed}
      <p class="note danger" role="alert"><Icon name="alert" size={16} />Could not build the ride. {exercise.buildError}</p>
      <div class="row">
        <button type="button" class="btn primary" onclick={() => exercise.build()}>Try again</button>
        <button type="button" class="btn tonal" onclick={() => exercise.changeSetup()}>Change setup</button>
      </div>
    {:else}
      <ol class="steps">
        {#each STEPS as s, i}
          {@const state = i < at ? "done" : i === at ? "now" : "next"}
          <li class={state} aria-current={state === "now" ? "step" : undefined}>
            <span class="tagw {state}">{state === "done" ? "Done" : state === "now" ? "Now" : "Next"}</span>
            <span class="name">{s.label}</span>
            <span class="count">{count(s.id)}</span>
          </li>
        {/each}
      </ol>
      <div class="bar" role="progressbar" aria-label="Building your ride" aria-valuemin="0" aria-valuemax="100" aria-valuenow={percent}><i style:width="{percent}%"></i></div>
      <div class="foot">
        <span class="pct">{percent}%</span>
        <span class="muted">You can leave this screen; the build keeps going.</span>
        <span class="sp"></span>
        <button type="button" class="btn tonal" onclick={() => exercise.cancelBuild()}>Cancel</button>
      </div>
      <span class="sr" aria-live="polite">{STEPS[at].label}</span>
    {/if}
  </ExCard>
</div>

<style>
  .ex {
    display: flex;
    flex-direction: column;
    gap: 14px;
    max-width: 880px;
  }
  h1 {
    font-size: 26px;
  }
  .sub {
    margin: 4px 0 0;
    color: var(--muted);
  }
  .steps {
    list-style: none;
    margin: 18px 0 14px;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  li {
    display: grid;
    grid-template-columns: 64px 1fr auto;
    align-items: center;
    gap: 12px;
    min-height: 40px;
    padding: 0 10px;
    border-radius: var(--r-sm);
  }
  li.now {
    background: var(--surface-2);
  }
  li.next {
    color: var(--muted);
  }
  .name {
    font-weight: 600;
  }
  .count {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    font-size: 13px;
  }
  .tagw {
    justify-self: start;
    padding: 1px 10px;
    border-radius: var(--r-pill);
    font-size: 12px;
    font-weight: 700;
    background: var(--surface-3);
    color: var(--muted);
  }
  .tagw.done {
    background: rgb(74 222 128 / 0.16);
    color: var(--ok);
  }
  .tagw.now {
    background: var(--primary);
    color: var(--on-primary);
  }
  .bar {
    height: 10px;
    border-radius: var(--r-pill);
    background: var(--track);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    background: var(--primary);
    border-radius: inherit;
    transition: width 0.3s ease;
  }
  .foot {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-top: 14px;
  }
  .pct {
    font-weight: 800;
    font-size: 18px;
    font-variant-numeric: tabular-nums;
  }
  .sp {
    flex: 1;
  }
  .btn {
    height: 44px;
    padding: 0 22px;
  }
  .row {
    display: flex;
    gap: 10px;
    margin-top: 14px;
  }
  .note {
    margin: 18px 0 0;
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 12px 14px;
    border-radius: var(--r-md);
    line-height: 1.4;
  }
  .note.danger {
    background: color-mix(in srgb, var(--danger) 12%, transparent);
    border: 1px solid color-mix(in srgb, var(--danger) 45%, transparent);
    color: var(--danger);
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }
</style>
