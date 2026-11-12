<script lang="ts">
  // Calibrating the speed (Settings → Exercise → My bike): pedal to the clicks for 20 s at 80 RPM, then type the speed
  // the console shows. One point is enough: on a spinning bike the speed follows the cadence. The core turns it
  // into km/h per RPM (and says no to a number that can't be right); this only plays the clicks and asks.
  import { EX_CALIBRATION, exerciseApi, type ConsoleUnit } from "../../../lib/exercise-api";
  import { CuePlayer } from "../../../lib/exerciseCues";
  import { errorText } from "../../../lib/api";

  let {
    console: consoleUnit,
    volume,
    onsave,
    onclose,
  }: {
    /** What the console shows (only km/h and mph can be calibrated). */
    console: ConsoleUnit;
    /** The click's volume, 0 to 1 (the ride's cue volume). */
    volume: number;
    /** Receives km/h per RPM. */
    onsave: (kphPerRpm: number) => void;
    onclose: () => void;
  } = $props();

  const CLICKS = Math.floor((EX_CALIBRATION.seconds * 1000) / EX_CALIBRATION.clickMs) + 1;
  const TICK_MS = 250;
  const unit = $derived(consoleUnit === "mph" ? "mph" : "km/h");

  const player = new CuePlayer();
  let left = $state(0);
  let running = $state(false);
  let speed = $state("");
  let problem = $state<string | null>(null);
  let timer = 0;

  async function start() {
    problem = null;
    player.unlock();
    if (!(await player.clickTrain(CLICKS, EX_CALIBRATION.clickMs, volume))) {
      problem = "The click could not play. Check the sound output and try again.";
      return;
    }
    running = true;
    left = EX_CALIBRATION.seconds;
    const endAt = performance.now() + EX_CALIBRATION.seconds * 1000;
    timer = window.setInterval(() => {
      left = Math.max(0, Math.ceil((endAt - performance.now()) / 1000));
      if (left === 0) stopClicks();
    }, TICK_MS);
  }
  function stopClicks() {
    clearInterval(timer);
    player.stop();
    running = false;
  }
  $effect(() => () => {
    clearInterval(timer);
    player.dispose();
  });

  async function save() {
    const value = Number(speed.replace(",", "."));
    problem = null;
    if (!Number.isFinite(value) || value <= 0) {
      problem = `Type the speed in ${unit}.`;
      return;
    }
    try {
      const factor = await exerciseApi.calibrate(consoleUnit, EX_CALIBRATION.rpm, value);
      if (factor === null) problem = `${value} ${unit} does not look right for ${EX_CALIBRATION.rpm} RPM. Check the number and try again.`;
      else onsave(factor);
    } catch (e) {
      problem = errorText(e);
    }
  }
</script>

<div class="cal" role="group" aria-label="Calibrate speed">
  <p class="how">Pedal to the clicks for {EX_CALIBRATION.seconds} s ({EX_CALIBRATION.rpm} RPM), then type the speed your console shows.</p>
  <div class="row">
    <button type="button" class="btn tonal" disabled={running} onclick={start}>{running ? `Pedal… ${left} s` : "Play the clicks"}</button>
    {#if running}<button type="button" class="btn tonal" onclick={stopClicks}>Stop</button>{/if}
    <label class="speed">
      <span>Console speed</span>
      <input type="text" inputmode="decimal" bind:value={speed} placeholder="0.0" aria-describedby="cal-unit" onkeydown={(e) => e.key === "Enter" && save()} />
      <b id="cal-unit">{unit}</b>
    </label>
    <button type="button" class="btn primary" disabled={!speed.trim()} onclick={save}>Save</button>
    <button type="button" class="btn tonal" onclick={onclose}>Cancel</button>
  </div>
  {#if problem}<p class="err" role="alert">{problem}</p>{/if}
</div>

<style>
  .cal {
    display: flex;
    flex-direction: column;
    gap: var(--s-2);
    padding: var(--s-3) var(--s-4);
    border-top: 1px solid var(--border);
    background: var(--surface-2);
  }
  .how {
    margin: 0;
    color: var(--muted);
    font-size: 13px;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-2);
  }
  .speed {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--muted);
  }
  input {
    width: 84px;
    min-height: 36px;
    font-variant-numeric: tabular-nums;
  }
  .err {
    margin: 0;
    color: var(--danger);
    font-size: 13px;
  }
</style>
