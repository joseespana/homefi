<script lang="ts">
  // Settings → Playback: crossfade between songs, and how stereo reaches a
  // surround device. Same settings and meaning as Android's Playback section.
  import { api, type CrossfadeSettings, type FadeCurve, type OutputDevice, type OutputOptions, type ResamplerQuality, type Settings, type SurroundMode } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import ChoiceList from "./settings/ChoiceList.svelte";
  import SettingsGroup from "./settings/SettingsGroup.svelte";
  import SliderRow from "./settings/SliderRow.svelte";
  import SwitchRow from "./settings/SwitchRow.svelte";

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
  const minLabel = (seconds: number) => (seconds > 0 ? `${seconds} s` : `Auto (${xf.seconds * 2} s)`);
</script>

<SettingsGroup title="Crossfade">
  <SwitchRow
    label="Crossfade between songs"
    hint="The next song starts while this one fades out. Off keeps every transition gapless and bit-perfect; during a fade the sound isn't bit-perfect."
    checked={xf.mode === "on"}
    onchange={(on) => saveXf({ mode: on ? "on" : "off" })}
  />
  {#if xf.mode === "on"}
    <SliderRow label="Length" min={1} max={12} step={0.5} value={xf.seconds} format={(n) => `${n} s`} onchange={(n) => saveXf({ seconds: n })} />
    <ChoiceList label="Curve" options={curves} value={xf.curve} onchange={(id) => saveXf({ curve: id })} />
    <SwitchRow label="Skip the silence" hint="Fade from where the music really ends into where the next song starts (silence below −60 dB, up to 10 s)" checked={xf.smart} onchange={(on) => saveXf({ smart: on })} />
    <SwitchRow label="Keep albums gapless" hint="No fade between songs of the same album played in order (live albums, mixes)" checked={xf.skipAlbumRuns} onchange={(on) => saveXf({ skipAlbumRuns: on })} />
    <SwitchRow label="Fade when I skip" hint="A short fade on next, previous or picking a song, instead of a cut" checked={xf.onManualSkip} onchange={(on) => saveXf({ onManualSkip: on })} />
    {#if xf.onManualSkip}
      <SliderRow label="Skip fade" min={0.5} max={3} step={0.25} value={xf.manualSeconds} format={(n) => `${n} s`} onchange={(n) => saveXf({ manualSeconds: n })} />
    {/if}
    <SliderRow
      label="Shortest song that fades"
      hint="Shorter songs (interludes, intros) play whole, with gapless transitions. 0 = twice the fade."
      min={0}
      max={120}
      step={5}
      value={xf.minTrackSeconds}
      format={minLabel}
      onchange={(n) => saveXf({ minTrackSeconds: n })}
    />
  {/if}
</SettingsGroup>

{#if channels > 2}
  <SettingsGroup title="Surround speakers" description="Surround files (5.1, 7.1) always play on their own speakers.">
    <ChoiceList label="Stereo on {channels === 8 ? '7.1' : channels === 6 ? '5.1' : `${channels}-channel`} speakers" options={surrounds} value={out.surround} onchange={(id) => saveOut({ surround: id })} />
  </SettingsGroup>
{/if}

<SettingsGroup title="Resampling" description="Only used when the device can't play a song at its own rate.">
  <ChoiceList label="Resampling quality" options={qualities} value={out.resampler} onchange={(id) => saveOut({ resampler: id })} />
</SettingsGroup>
