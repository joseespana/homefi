<script lang="ts">
  // Settings → Playback: crossfade between songs, and how stereo reaches a
  // surround device. Same settings and meaning as Android's Playback section.
  import { api, type CrossfadeSettings, type FadeCurve, type OutputDevice, type OutputOptions, type ResamplerQuality, type Settings, type SurroundMode } from "../lib/api";
  import { app } from "../lib/state.svelte";

  let { settings, save }: { settings: Settings; save: (patch: Partial<Settings>) => Promise<void> | void } = $props();

  const CROSSFADE: CrossfadeSettings = { mode: "off", seconds: 6, curve: "equalPower", smart: true, skipAlbumRuns: true, onManualSkip: true, manualSeconds: 1.5, minTrackSeconds: 0, lowPowerFallback: true };
  const OUTPUT: OutputOptions = { surround: "frontOnly", resampler: "high" };
  const xf = $derived<CrossfadeSettings>({ ...CROSSFADE, ...(settings.crossfade ?? {}) });
  const out = $derived<OutputOptions>({ ...OUTPUT, ...(settings.output ?? {}) });

  const curves: { id: FadeCurve; label: string; hint: string }[] = [
    { id: "equalPower", label: "Equal power", hint: "Even loudness all the way through" },
    { id: "sCurve", label: "S-curve", hint: "Stays on each song longer, swaps quickly in the middle" },
    { id: "linear", label: "Linear", hint: "Straight lines: a slight dip halfway" },
  ];
  const surrounds: { id: SurroundMode; label: string; hint: string }[] = [
    { id: "frontOnly", label: "Front only", hint: "Stereo on the front left and right speakers; the rest stay silent" },
    { id: "allSpeakers", label: "All speakers", hint: "Stereo also on the side and rear speakers (−3 dB); no center or subwoofer" },
    { id: "upmix", label: "Upmix", hint: "Center from left + right, subwoofer from the lows (below 120 Hz), rears at −6 dB" },
  ];
  const qualities: { id: ResamplerQuality; label: string; hint: string }[] = [
    { id: "high", label: "High", hint: "Inaudible conversion (the default)" },
    { id: "fast", label: "Fast", hint: "Lighter on the CPU, still clean; try it if the sound breaks up" },
  ];

  function saveXf(patch: Partial<CrossfadeSettings>) {
    save({ crossfade: { ...xf, ...patch } });
  }
  function saveOut(patch: Partial<OutputOptions>) {
    save({ output: { ...out, ...patch } });
  }

  // the surround choice only matters on a device with more than two speakers
  let devices = $state<OutputDevice[]>([]);
  $effect(() => {
    api.outputDevices().then((d) => (devices = d)).catch(() => {});
  });
  const device = $derived(devices.find((d) => d.name === (settings.output_device ?? "")) ?? devices.find((d) => d.is_default) ?? null);
  const channels = $derived(Math.max(app.player?.stream?.output_channels ?? 0, device?.max_channels ?? 0));
  const minLabel = $derived(xf.minTrackSeconds > 0 ? `${xf.minTrackSeconds} s` : `Auto (${xf.seconds * 2} s)`);
</script>

<section>
  <h3>Crossfade</h3>
  <label class="toggle top">
    <input type="checkbox" checked={xf.mode === "on"} onchange={(e) => saveXf({ mode: e.currentTarget.checked ? "on" : "off" })} />
    <span>Crossfade between songs<small class="muted">The next song starts while this one fades out. Off keeps every transition gapless and bit-perfect; during a fade the sound isn't bit-perfect.</small></span>
  </label>
  {#if xf.mode === "on"}
    <div class="row-setting">
      <span class="label">Length <b class="mono">{xf.seconds} s</b></span>
      <input type="range" min="1" max="12" step="0.5" value={xf.seconds} aria-label="Crossfade length" onchange={(e) => saveXf({ seconds: Number(e.currentTarget.value) })} />
    </div>
    <div class="row-setting">
      <span class="label">Curve</span>
      <div class="segmented compact">
        {#each curves as c (c.id)}
          <button class:selected={xf.curve === c.id} title={c.hint} onclick={() => saveXf({ curve: c.id })}><b>{c.label}</b></button>
        {/each}
      </div>
      <small class="muted">{curves.find((c) => c.id === xf.curve)?.hint}</small>
    </div>
    <label class="toggle top">
      <input type="checkbox" checked={xf.smart} onchange={(e) => saveXf({ smart: e.currentTarget.checked })} />
      <span>Skip the silence<small class="muted">Fade from where the music really ends into where the next song starts (silence below −60 dB, up to 10 s)</small></span>
    </label>
    <label class="toggle top">
      <input type="checkbox" checked={xf.skipAlbumRuns} onchange={(e) => saveXf({ skipAlbumRuns: e.currentTarget.checked })} />
      <span>Keep albums gapless<small class="muted">No fade between songs of the same album played in order (live albums, mixes)</small></span>
    </label>
    <label class="toggle top">
      <input type="checkbox" checked={xf.onManualSkip} onchange={(e) => saveXf({ onManualSkip: e.currentTarget.checked })} />
      <span>Fade when I skip<small class="muted">A short fade on next, previous or picking a song, instead of a cut</small></span>
    </label>
    {#if xf.onManualSkip}
      <div class="row-setting">
        <span class="label">Skip fade <b class="mono">{xf.manualSeconds} s</b></span>
        <input type="range" min="0.5" max="3" step="0.25" value={xf.manualSeconds} aria-label="Skip fade length" onchange={(e) => saveXf({ manualSeconds: Number(e.currentTarget.value) })} />
      </div>
    {/if}
    <div class="row-setting">
      <span class="label">Shortest song that fades <b class="mono">{minLabel}</b></span>
      <input type="range" min="0" max="120" step="5" value={xf.minTrackSeconds} aria-label="Shortest song that fades" onchange={(e) => saveXf({ minTrackSeconds: Number(e.currentTarget.value) })} />
      <small class="muted">Shorter songs (interludes, intros) play whole, with gapless transitions. 0 = twice the fade.</small>
    </div>
  {/if}

  {#if channels > 2}
    <h3>Surround speakers</h3>
    <div class="row-setting">
      <span class="label">Stereo on {channels === 8 ? "7.1" : channels === 6 ? "5.1" : `${channels}-channel`} speakers</span>
      <div class="segmented compact">
        {#each surrounds as m (m.id)}
          <button class:selected={out.surround === m.id} title={m.hint} onclick={() => saveOut({ surround: m.id })}><b>{m.label}</b></button>
        {/each}
      </div>
      <small class="muted">{surrounds.find((m) => m.id === out.surround)?.hint}. Surround files (5.1, 7.1) always play on their own speakers.</small>
    </div>
  {/if}

  <h3>Resampling</h3>
  <div class="row-setting">
    <span class="label">Quality</span>
    <div class="segmented compact">
      {#each qualities as q (q.id)}
        <button class:selected={out.resampler === q.id} title={q.hint} onclick={() => saveOut({ resampler: q.id })}><b>{q.label}</b></button>
      {/each}
    </div>
    <small class="muted">{qualities.find((q) => q.id === out.resampler)?.hint}. Only used when the device can't play a song at its own rate.</small>
  </div>
</section>

<style>
  h3 {
    margin: 0 0 8px;
    font-size: 14px;
  }
  h3:not(:first-child) {
    margin-top: 20px;
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 0;
  }
  .toggle input {
    flex: none;
    width: 16px;
    height: 16px;
    accent-color: var(--primary-strong);
  }
  .toggle.top {
    align-items: flex-start;
  }
  .toggle.top input {
    margin-top: 3px;
  }
  .toggle span {
    display: flex;
    flex-direction: column;
  }
  .toggle small {
    font-weight: 400;
    font-size: 12px;
  }
  .row-setting {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 12px 0 6px;
    max-width: 560px;
  }
  .row-setting .label {
    font-size: 13px;
    font-weight: 600;
  }
  .row-setting input[type="range"] {
    accent-color: var(--primary-strong);
  }
  .segmented {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(120px, 160px));
    gap: 8px;
  }
  .segmented button {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px 12px;
    border-radius: min(12px, var(--r-max));
    background: var(--surface-2);
    border: 1px solid var(--border);
    text-align: left;
    transition: background 0.2s, border-color 0.2s, box-shadow 0.2s;
  }
  .segmented button:hover:not(.selected) {
    border-color: color-mix(in srgb, var(--primary) 45%, var(--border));
  }
  .segmented button.selected {
    border-color: var(--primary);
    box-shadow: inset 0 0 0 1px var(--primary);
    background: color-mix(in srgb, var(--primary) 14%, var(--surface));
  }
  .segmented button.selected b {
    color: var(--primary);
  }
</style>
