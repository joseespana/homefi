<script lang="ts">
  // The ride while another page shows: one strip at the foot of every page, like the song that plays. The part
  // and its colour, the time left in it and the target; a click goes back to the ride, the button pauses it.
  import { clock, shownLabel, zoneName, zoneVar } from "../../lib/exercise-api";
  import { exercise } from "../../lib/exercise.svelte";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";

  const view = $derived(exercise.view);
</script>

{#if view}
  <div class="strip" style:--c={zoneVar(view.kind)} role="region" aria-label="Ride in progress">
    <button type="button" class="back" onclick={() => (app.view = { name: "exercise" })} title="Back to the ride">
      <span class="dot"><Icon name="bike" size={18} /></span>
      <span class="txt">
        <b>{zoneName(view.kind)} · {clock(view.remaining_s)}</b>
        <small>{shownLabel(view.shown, view.target_rpm)} · Back to the ride</small>
      </span>
    </button>
    <button type="button" class="pause" aria-label={exercise.playing ? "Pause the ride" : "Resume the ride"} onclick={() => exercise.pause()}>
      <Icon name={exercise.playing ? "pause" : "play"} size={20} />
    </button>
  </div>
{/if}

<style>
  .strip {
    position: sticky;
    bottom: 0;
    z-index: 5;
    display: flex;
    align-items: center;
    gap: 12px;
    margin: 0 clamp(18px, 3.2vw, 40px) 12px;
    padding: 6px 8px 6px 6px;
    border: 1px solid var(--c);
    border-radius: var(--r-pill);
    background: color-mix(in srgb, var(--c) 16%, var(--surface));
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.25);
  }
  .back {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 12px;
    border: 0;
    background: none;
    color: var(--text);
    text-align: left;
    cursor: pointer;
    padding: 0;
  }
  .dot,
  .pause {
    flex: none;
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: 50%;
    background: var(--c);
    color: #000;
  }
  .pause {
    border: 0;
    cursor: pointer;
  }
  .txt {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .txt b,
  .txt small {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .txt small {
    color: var(--muted);
  }
  .back:focus-visible,
  .pause:focus-visible {
    outline: 2px solid var(--c);
    outline-offset: 2px;
  }
</style>
