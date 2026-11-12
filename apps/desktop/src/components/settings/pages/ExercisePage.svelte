<script lang="ts">
  // Settings → Exercise: every option of homefi_core's `ExerciseSettings`, in plain words. The same JSON as
  // Android's (which adds the buzz on part changes); the core clamps whatever is sent.
  import {
    DEFAULT_EXERCISE_SETTINGS,
    EX_ANALYSIS_BUDGET,
    EX_CLIMB_STEPS,
    EX_COUNTDOWN_S,
    EX_CROSSFADE_S,
    EX_HISTORY,
    EX_MINUTES,
    EX_NOTICE_S,
    EX_PULSE_OFFSET_MS,
    EX_RESISTANCE_LEVELS,
    EX_TOLERANCE_RPM,
    EX_TV_DISPLAY_AUTO,
    EX_TV_PORT,
    PRESET_IDS,
    STYLE_INFO,
    exerciseApi,
    exerciseSettingsOf,
    type ConsoleUnit,
    type CourseLook,
    type CourseRider,
    type ExerciseSettings,
    type MonitorView,
    type PresetId,
    type PulseStyle,
    type SourcesMode,
    type TvAudio,
  } from "../../../lib/exercise-api";
  import type { Settings } from "../../../lib/api";
  import { useSettings } from "../model.svelte";
  import ActionRow from "../ActionRow.svelte";
  import FieldRow from "../FieldRow.svelte";
  import SegmentedRow from "../SegmentedRow.svelte";
  import SettingRow from "../SettingRow.svelte";
  import BikeCalibration from "./BikeCalibration.svelte";
  import SettingsGroup from "../SettingsGroup.svelte";
  import SettingsIntro from "../SettingsIntro.svelte";
  import SliderRow from "../SliderRow.svelte";
  import SwitchRow from "../SwitchRow.svelte";
  import type { SegmentOption } from "../types";

  let { settings }: { settings: Settings } = $props();
  const m = useSettings();

  const e = $derived(exerciseSettingsOf(settings));
  const save = (patch: Partial<ExerciseSettings>) => m.save({ exercise: { ...e, ...patch } });

  const STYLES: SegmentOption<PresetId>[] = PRESET_IDS.map((id) => ({ id, label: STYLE_INFO[id].name, hint: STYLE_INFO[id].line }));
  const SOURCES: SegmentOption<SourcesMode>[] = [
    { id: "library_first", label: "Library first", hint: "Songs you own first, TIDAL fills the gaps" },
    { id: "tidal_first", label: "TIDAL first", hint: "TIDAL first, your library as the backup" },
    { id: "library_only", label: "Library only", hint: "Never uses TIDAL for the songs of a ride" },
  ];
  const PULSES: SegmentOption<PulseStyle>[] = [
    { id: "ring", label: "Ring", hint: "The ring swells and glows on every pedal stroke" },
    { id: "bar", label: "Bar", hint: "A bar sweeps under the number and flashes on the stroke" },
    { id: "flash", label: "Flash", hint: "The whole ring flashes softly (never faster than 3 a second)" },
  ];
  const LOOKS: SegmentOption<CourseLook>[] = [
    { id: "ribbon", label: "Ribbon", hint: "Warm summits over a cool foot; the zone is a thin line and a ribbon" },
    { id: "alpine", label: "Alpine", hint: "Calm slate ground with snow on the high ground" },
    { id: "stage", label: "Stage", hint: "A dark stage with the zone colour washing down from the ridge" },
  ];
  const RIDERS: SegmentOption<CourseRider>[] = [
    { id: "bike", label: "Bike", hint: "A small bike silhouette rides the mountain" },
    { id: "dot", label: "Dot", hint: "Only the position dot" },
  ];
  const CONSOLES: SegmentOption<ConsoleUnit>[] = [
    { id: "rpm", label: "RPM", hint: "Your console shows the cadence" },
    { id: "kph", label: "km/h", hint: "Your console shows the speed in kilometres an hour" },
    { id: "mph", label: "mph", hint: "Your console shows the speed in miles an hour" },
    { id: "none", label: "Nothing", hint: "Targets show in RPM" },
  ];
  const STEPS: SegmentOption<string>[] = [
    { id: String(EX_CLIMB_STEPS.off), label: "Off", hint: "A climb keeps one resistance" },
    ...EX_CLIMB_STEPS.options.map((n) => ({ id: String(n), label: String(n), hint: `${n} levels of resistance in each climb` })),
  ];
  const AUDIO: SegmentOption<TvAudio>[] = [
    { id: "tv", label: "On the TV", hint: "The TV plays the music too (HomeFi for Android TV)" },
    { id: "phone", label: "Here", hint: "Pictures only on the TV; the music stays on this computer" },
  ];

  const seconds = (n: number) => `${n} s`;
  const lyricsOffset = $derived(settings.room.lyrics_offset_ms ?? 0);
  const offsetLabel = (n: number) => (n === 0 ? "0 ms" : `${n > 0 ? "+" : "−"}${Math.abs(n)} ms`);

  let monitors = $state.raw<MonitorView[]>([]);
  $effect(() => {
    exerciseApi.monitors().then((list) => (monitors = list)).catch(() => {});
  });

  // the bike: what the console shows, how many positions the knob has, and the speed calibration
  const bike = $derived(e.bike);
  const saveBike = (patch: Partial<ExerciseSettings["bike"]>) => save({ bike: { ...bike, ...patch } });
  const canCalibrate = $derived(bike.console === "kph" || bike.console === "mph");
  let calibrating = $state(false);
  const factorText = (f: number) => `1 RPM ≈ ${(bike.console === "mph" ? f / 1.609344 : f).toFixed(2)} ${bike.console === "mph" ? "mph" : "km/h"}`;

  const port = (value: string) => {
    const n = Math.round(Number(value));
    if (Number.isFinite(n)) save({ tv_port: Math.min(EX_TV_PORT.max, Math.max(EX_TV_PORT.min, n)) });
  };
</script>

<SettingsIntro>Rides for an exercise bike: HomeFi builds a course, picks songs whose beat fits each part and cues the changes. It gives a pace and an effort to follow, not a medical measurement.</SettingsIntro>

<SettingsGroup title="Ride">
  <SwitchRow label="Exercise in the sidebar" hint="The button that opens Exercise. While a ride is on it stays." checked={e.show_entry} onchange={(show_entry) => save({ show_entry })} />
  <SwitchRow label="Beginner mode" hint="Gentler caps: a calmer pace, lower effort, shorter sprints with more rest." checked={e.beginner_mode} onchange={(beginner_mode) => save({ beginner_mode })} />
  <SegmentedRow label="Default style" hint="What the setup starts with." options={STYLES} value={e.default_preset} onchange={(default_preset) => save({ default_preset })} />
  <SliderRow label="Default length" hint="What the setup starts with." min={EX_MINUTES.min} max={EX_MINUTES.max} step={EX_MINUTES.step} value={e.ride_minutes} format={(n) => `${n} min`} onchange={(ride_minutes) => save({ ride_minutes })} />
</SettingsGroup>

<SettingsGroup title="My bike" description="So targets read the way your console does. Without it, everything is in RPM.">
  <SegmentedRow label="The console shows" options={CONSOLES} value={bike.console} onchange={(console) => saveBike({ console })} />
  <SliderRow label="Resistance levels" hint="How many positions your knob has. 0 if you don't know: effort shows as 1 to 10 only." min={EX_RESISTANCE_LEVELS.min} max={EX_RESISTANCE_LEVELS.max} step={1} value={bike.resistance_levels} format={(n) => (n === 0 ? "Don't know" : `${n} levels`)} onchange={(resistance_levels) => saveBike({ resistance_levels })} />
  <ActionRow label="Calibrate speed" hint={!canCalibrate ? "Choose km/h or mph above first." : bike.kph_per_rpm > 0 ? `Calibrated: ${factorText(bike.kph_per_rpm)}.` : "Not calibrated: targets show in RPM."} disabled={!canCalibrate}>
    <button class="btn tonal" type="button" disabled={!canCalibrate || calibrating} onclick={() => (calibrating = true)}>Calibrate…</button>
    <button class="btn tonal" type="button" disabled={bike.kph_per_rpm <= 0} onclick={() => saveBike({ kph_per_rpm: 0 })}>Clear</button>
  </ActionRow>
  {#if calibrating && canCalibrate}
    <BikeCalibration console={bike.console} volume={e.cue_volume} onsave={(kph_per_rpm) => ((calibrating = false), saveBike({ kph_per_rpm }))} onclose={() => (calibrating = false)} />
  {/if}
</SettingsGroup>

<SettingsGroup title="Saved rides" description="Every ride HomeFi builds is saved by itself, so you can ride it again without the AI. They are in Exercise → My rides.">
  <SliderRow label="Automatic rides to keep" hint="When there are more, the oldest automatic ones go. Rides you keep, star or import are never removed." min={EX_HISTORY.min} max={EX_HISTORY.max} step={EX_HISTORY.step} value={e.ride_history_max} format={(n) => `${n} rides`} onchange={(ride_history_max) => save({ ride_history_max })} />
</SettingsGroup>

<SettingsGroup title="Climbs and songs">
  <SegmentedRow label="Resistance steps on climbs" hint="Each climb asks for one more turn of resistance two or three times, like a class: the map marks the spots and a sound says when." options={STEPS} value={String(e.climb_steps)} onchange={(n) => save({ climb_steps: Number(n) })} />
  <SwitchRow label="Change a song during the ride" hint="A tap on the song puts another that fits the part's pace." checked={e.ride_swap} onchange={(ride_swap) => save({ ride_swap })} />
  <SwitchRow label="Ask the AI for the replacement" hint="When nothing already found fits, the AI proposes one (uses your key); off, only TIDAL and the library are searched." checked={e.swap_uses_ai} disabled={!e.ride_swap} onchange={(swap_uses_ai) => save({ swap_uses_ai })} />
</SettingsGroup>

<SettingsGroup title="Music" description="How songs are matched to the beat of each part.">
  <SegmentedRow label="Pick songs from" options={SOURCES} value={e.sources} onchange={(sources) => save({ sources })} />
  <SliderRow label="Tempo tolerance" hint="How far a song's tempo may be from the pace you are asked to keep." min={EX_TOLERANCE_RPM.min} max={EX_TOLERANCE_RPM.max} step={1} value={e.cadence_tolerance_rpm} format={(n) => `${n} RPM`} onchange={(cadence_tolerance_rpm) => save({ cadence_tolerance_rpm })} />
  <SwitchRow label="Allow half-time" hint="A fast song (120 to 180 BPM) counts one pedal stroke every two beats." checked={e.allow_half_time} onchange={(allow_half_time) => save({ allow_half_time })} />
  <SwitchRow label="Allow double-time" hint="A very slow song counts two pedal strokes a beat." checked={e.allow_double_time} onchange={(allow_double_time) => save({ allow_double_time })} />
  <SwitchRow label="Allow explicit songs" checked={e.allow_explicit} onchange={(allow_explicit) => save({ allow_explicit })} />
  <SliderRow label="Tempo check budget" hint="Songs HomeFi may listen to (decode) per ride when no tempo is known. 0 turns the check off." min={EX_ANALYSIS_BUDGET.min} max={EX_ANALYSIS_BUDGET.max} step={1} value={e.analysis_budget} format={(n) => (n === 0 ? "Off" : `${n} songs`)} onchange={(analysis_budget) => save({ analysis_budget })} />
</SettingsGroup>

<SettingsGroup title="Cues and beat">
  <SwitchRow label="Sounds for changes" hint="A short sound when the part changes, you stand up or sit down, a sprint starts or ends." checked={e.audible_cues} onchange={(audible_cues) => save({ audible_cues })} />
  <SliderRow label="Cue volume" min={0} max={100} step={5} value={Math.round(e.cue_volume * 100)} format={(n) => `${n} %`} disabled={!e.audible_cues} onchange={(n) => save({ cue_volume: n / 100 })} />
  <SliderRow label="Crossfade between the ride's songs" hint="How long the next song fades in when a part ends. 0 keeps the cut." min={EX_CROSSFADE_S.min} max={EX_CROSSFADE_S.max} step={EX_CROSSFADE_S.step} value={e.crossfade_seconds} format={(n) => (n === 0 ? "Off" : `${n} s`)} onchange={(crossfade_seconds) => save({ crossfade_seconds })} />
  <SliderRow label="Countdown before a change" hint="3-2-1 over the ring. 0 turns it off." min={EX_COUNTDOWN_S.min} max={EX_COUNTDOWN_S.max} step={1} value={e.countdown_seconds} format={seconds} onchange={(countdown_seconds) => save({ countdown_seconds })} />
  <SliderRow label="Next-part notice" hint="How long before a change the Next card fills in." min={EX_NOTICE_S.min} max={EX_NOTICE_S.max} step={1} value={e.segment_notice_seconds} format={seconds} onchange={(segment_notice_seconds) => save({ segment_notice_seconds })} />
  <SwitchRow label="Click on the beat" hint="A soft click on every pedal stroke, for when you can't look at the screen." checked={e.metronome_click} onchange={(metronome_click) => save({ metronome_click })} />
  <SegmentedRow label="Beat pulse" hint="How the beat shows inside the ring." options={PULSES} value={e.pulse_style} onchange={(pulse_style) => save({ pulse_style })} />
  <SliderRow label="Pulse offset" hint="Moves the pulse earlier or later, to match a Bluetooth speaker's delay." min={EX_PULSE_OFFSET_MS.min} max={EX_PULSE_OFFSET_MS.max} step={EX_PULSE_OFFSET_MS.step} value={e.pulse_offset_ms} format={offsetLabel} onchange={(pulse_offset_ms) => save({ pulse_offset_ms })} />
  <ActionRow label="Match my lyrics offset" hint={`Uses the offset you set for synced lyrics (${offsetLabel(lyricsOffset)}).`}>
    <button class="btn tonal" type="button" disabled={e.pulse_offset_ms === Math.max(EX_PULSE_OFFSET_MS.min, Math.min(EX_PULSE_OFFSET_MS.max, lyricsOffset))} onclick={() => save({ pulse_offset_ms: Math.max(EX_PULSE_OFFSET_MS.min, Math.min(EX_PULSE_OFFSET_MS.max, lyricsOffset)) })}>Match</button>
  </ActionRow>
</SettingsGroup>

<SettingsGroup title="Course" description="The mountain that shows the ride: setup, preview, the ride itself, the summary and the TV.">
  <SegmentedRow label="Course look" options={LOOKS} value={e.course_look} onchange={(course_look) => save({ course_look })} />
  <SegmentedRow label="Rider on the map" options={RIDERS} value={e.course_rider} onchange={(course_rider) => save({ course_rider })} />
  <SwitchRow label="Summit times" hint="Marks the tallest summits with the time you reach them." checked={e.course_summits} onchange={(course_summits) => save({ course_summits })} />
</SettingsGroup>

<SettingsGroup title="Screen">
  <SwitchRow label="Always dark during a ride" hint="The ride screen stays dark in any theme." checked={e.ride_always_dark} onchange={(ride_always_dark) => save({ ride_always_dark })} />
  <SwitchRow label="Keep the screen on" hint="No screen saver or sleep during a ride." checked={e.keep_screen_on} onchange={(keep_screen_on) => save({ keep_screen_on })} />
</SettingsGroup>

<SettingsGroup title="TV" description="Show the ride on a TV: a window on another screen, or a link for a TV's browser.">
  <SwitchRow label="TV" hint="Start the TV page with every ride." checked={e.tv_enabled} onchange={(tv_enabled) => save({ tv_enabled })} />
  <SegmentedRow label="Music plays" options={AUDIO} value={e.tv_audio} onchange={(tv_audio) => save({ tv_audio })} />
  <SettingRow label="TV window on" hint="The screen the TV window opens on." labelFor>
    {#snippet children(ids)}
      <select id={ids.control} value={e.tv_window_display} onchange={(ev) => save({ tv_window_display: ev.currentTarget.value })}>
        <option value={EX_TV_DISPLAY_AUTO}>The second display</option>
        {#each monitors as mon}<option value={mon.name ?? ""}>{mon.name ?? "Display"} · {mon.width} × {mon.height}{mon.primary ? " (main)" : ""}</option>{/each}
        {#if e.tv_window_display !== EX_TV_DISPLAY_AUTO && !monitors.some((mon) => mon.name === e.tv_window_display)}<option value={e.tv_window_display}>{e.tv_window_display}</option>{/if}
      </select>
    {/snippet}
  </SettingRow>
  <SwitchRow label="TV window fullscreen" checked={e.tv_window_fullscreen} onchange={(tv_window_fullscreen) => save({ tv_window_fullscreen })} />
  <SwitchRow label="Lyrics on the TV" hint="Shows the song's lyrics on the TV, line by line, when they are available. The shift against the music is the one of the listening room's lyrics." checked={e.tv_lyrics} onchange={(tv_lyrics) => save({ tv_lyrics })} />
  <FieldRow label="TV port" hint="The TV page is served on this port of your network. Takes effect the next time the TV page starts." type="number" inputmode="numeric" min={EX_TV_PORT.min} max={EX_TV_PORT.max} width="110px" mono value={e.tv_port} onchange={port} />
</SettingsGroup>

<SettingsGroup title="Health">
  <ActionRow label="Health notice" hint={e.health_notice_seen ? "You accepted it. It can be shown again before your next ride." : "It will be shown before your first ride."}>
    <button class="btn tonal" type="button" disabled={!e.health_notice_seen} onclick={() => save({ health_notice_seen: false })}>Show it again</button>
  </ActionRow>
  <ActionRow label="Back to the defaults" hint="Every Exercise option as HomeFi ships it (the health notice stays accepted).">
    <button class="btn tonal" type="button" onclick={() => save({ ...DEFAULT_EXERCISE_SETTINGS, health_notice_seen: e.health_notice_seen })}>Reset Exercise settings</button>
  </ActionRow>
</SettingsGroup>

<style>
  select {
    min-height: 36px;
    max-width: 100%;
  }
</style>
