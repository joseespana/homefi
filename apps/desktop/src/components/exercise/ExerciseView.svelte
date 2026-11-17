<script lang="ts">
  // Exercise mode (docs/EXERCISE_UI.md): setup, building, preview, the ride and its summary. The state
  // lives in lib/exercise.svelte.ts, so leaving this screen never stops a build or a ride (the rail's
  // dot brings you back); this file only picks the screen.
  import { untrack } from "svelte";
  import { exercise } from "../../lib/exercise.svelte";
  import { app } from "../../lib/state.svelte";
  import BuildProgress from "./BuildProgress.svelte";
  import HealthNotice from "./HealthNotice.svelte";
  import PlanPreview from "./PlanPreview.svelte";
  import RideScreen from "./RideScreen.svelte";
  import RideSetup from "./RideSetup.svelte";
  import RideSummary from "./RideSummary.svelte";

  // which rides play with no internet changes with the library (a download finished) and with the plan on the screen
  $effect(() => {
    void app.tracks;
    void exercise.plan;
    untrack(() => exercise.refreshOffline());
  });
</script>

<!-- every screen lays itself out for the room it gets (no "make the window wider") -->
{#if exercise.step === "ride"}
  <RideScreen />
{:else if exercise.step === "done"}
  <RideSummary />
{:else if exercise.step === "building"}
  <BuildProgress />
{:else if exercise.step === "preview"}
  <PlanPreview />
{:else}
  <RideSetup />
{/if}

{#if exercise.noticeOpen}<HealthNotice />{/if}
<div class="sr" aria-live="polite">{exercise.live}</div>

<style>
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
