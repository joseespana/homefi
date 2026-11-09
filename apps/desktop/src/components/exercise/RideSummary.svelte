<script lang="ts">
  // The finished ride (docs/EXERCISE_UI.md §3.6): the whole mountain lit, four numbers and where to go next.
  import { clock, courseText, partsOf } from "../../lib/exercise-api";
  import { exercise } from "../../lib/exercise.svelte";
  import Icon from "../Icon.svelte";
  import Mountain from "./Mountain.svelte";

  const plan = $derived(exercise.plan!);
  const parts = $derived(partsOf(plan.workout));
  const total = $derived(plan.workout.segments.reduce((s, x) => s + x.seconds, 0));
  const stats = $derived([
    { label: "Time", value: clock(total) },
    { label: "Songs", value: String(plan.songs.length) },
    { label: "Climbs", value: String(plan.workout.segments.filter((s) => s.kind === "seated_climb" || s.kind === "standing_climb").length) },
    { label: "Sprint blocks", value: String(plan.workout.segments.filter((s) => !!s.interval).length) },
  ]);
  const end = $derived(plan.profile.length ? plan.profile[plan.profile.length - 1] : null);
</script>

<div class="done">
  <span class="mark" aria-hidden="true"><Icon name="check" size={30} /></span>
  <h1>Ride complete. Nice work!</h1>

  <section class="card">
    <Mountain {parts} profile={plan.profile} mode="summary" height={200} marker={end} label={courseText(parts, total, null)} />
  </section>

  <dl class="stats">
    {#each stats as s}
      <div><dd>{s.value}</dd><dt>{s.label}</dt></div>
    {/each}
  </dl>
  <p class="guide">This is a guide for effort, not a medical measurement.</p>

  <div class="acts">
    <button type="button" class="again" onclick={() => exercise.rideAgain()}><Icon name="refresh" size={18} />Ride again</button>
    <button type="button" class="btn tonal" onclick={() => exercise.rebuild()}>Same ride, new songs</button>
    <button type="button" class="btn tonal" onclick={() => exercise.finish()}>Done</button>
  </div>
</div>

<style>
  .done {
    max-width: 920px;
    margin: 0 auto;
    padding: 12px 0 24px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
    text-align: center;
  }
  .mark {
    display: grid;
    place-items: center;
    width: 64px;
    height: 64px;
    border-radius: 50%;
    background: color-mix(in srgb, var(--ok) 18%, transparent);
    color: var(--ok);
  }
  h1 {
    font-size: 34px;
  }
  .card {
    width: 100%;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
    padding: 18px 18px 14px;
  }
  .stats {
    margin: 0;
    width: 100%;
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 12px;
  }
  .stats div {
    display: flex;
    flex-direction: column-reverse;
    gap: 2px;
    padding: 14px 8px;
    border-radius: var(--r-md);
    background: var(--surface);
    border: 1px solid var(--border);
  }
  dd {
    margin: 0;
    font-size: 56px;
    font-weight: 800;
    line-height: 1;
    font-variant-numeric: tabular-nums;
  }
  dt {
    color: var(--muted);
    font-weight: 600;
  }
  .guide {
    margin: 0;
    color: var(--muted);
    font-size: 13px;
  }
  .acts {
    display: flex;
    gap: 12px;
    align-items: center;
  }
  .again {
    height: 56px;
    padding: 0 30px;
    border-radius: var(--r-pill);
    background: var(--primary-strong);
    color: var(--on-primary-strong);
    font-weight: 700;
    font-size: 16px;
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }
  .again:hover {
    filter: brightness(1.12);
  }
  .acts .btn {
    height: 48px;
    padding: 0 22px;
  }
</style>
