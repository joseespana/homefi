<script lang="ts">
  import type { EqUiSettings, Settings, SoundUiSettings } from "../../../lib/api";
  import { INTENSITIES, LOUDNESS_DEFAULTS, LOUDNESS_MODES, SOUND_UI_DEFAULTS, TASTE_DEFAULTS, layersWords } from "../../../lib/sound.svelte";
  import { app } from "../../../lib/state.svelte";
  import Icon from "../../Icon.svelte";
  import { useSettings } from "../model.svelte";
  import ActionRow from "../ActionRow.svelte";
  import ChoiceList from "../ChoiceList.svelte";
  import SegmentedRow from "../SegmentedRow.svelte";
  import SettingsGroup from "../SettingsGroup.svelte";
  import SettingsIntro from "../SettingsIntro.svelte";
  import SliderRow from "../SliderRow.svelte";
  import SwitchRow from "../SwitchRow.svelte";

  let { settings }: { settings: Settings } = $props();
  const m = useSettings();

  const EQ_UI: EqUiSettings = { deck_button: true, ab_mode: "hold", band_labels: true, taste_sliders: true };
  const eqUi = $derived({ ...EQ_UI, ...(settings.eq_ui ?? {}) });
  const soundUi = $derived({ ...SOUND_UI_DEFAULTS, ...(settings.sound_ui ?? {}) });
  const loudness = $derived({ ...LOUDNESS_DEFAULTS, ...(settings.loudness ?? {}) });
  const taste = $derived({ ...TASTE_DEFAULTS, ...(settings.taste ?? {}) });
  const saveEqUi = (patch: Partial<EqUiSettings>) => m.save({ eq_ui: { ...eqUi, ...patch } });
  const saveSoundUi = (patch: Partial<SoundUiSettings>) => m.save({ sound_ui: { ...soundUi, ...patch } });

  const AB_MODES = [
    { id: "hold", label: "While held" },
    { id: "toggle", label: "Click on, click off" },
  ] as const;
  const TASTE_EDITORS = [
    { id: "auto", label: "Auto" },
    { id: "simple", label: "Simple" },
    { id: "advanced", label: "Advanced" },
  ] as const;
  /** Loudness targets people choose between (LUFS). */
  const TARGET_MIN_LUFS = -24;
  const TARGET_MAX_LUFS = -8;
  const outputProfiles = $derived(Object.keys(settings.eq?.perOutput ?? {}).length);
</script>

<SettingsIntro>One Sound screen for how your music sounds: a switch (off is bit-perfect), a correction for each output (measured for thousands of headphones), and sounds you make for songs, albums, artists, playlists or genres. While it changes the sound, playback isn't bit-perfect.</SettingsIntro>

<SettingsGroup title="Sound" description="{outputProfiles} output corrections · {settings.eq?.defaultProfile ? `“${settings.eq.defaultProfile.name}” for other outputs` : 'other outputs flat'} · {settings.eq_presets?.length ?? 0} saved presets · {taste.rules.length} {taste.rules.length === 1 ? 'sound' : 'sounds'}">
  <SwitchRow
    label="Sound on"
    hint={settings.eq?.enabled ? (app.player?.eq_active ? `Playing with “${layersWords(app.player)}” now: processed, not bit-perfect` : "Nothing is changed for this song right now: bit-perfect") : "Off: bit-perfect, HomeFi does not touch the audio"}
    checked={settings.eq?.enabled ?? false}
    onchange={(on) => m.save({ eq: { ...settings.eq, enabled: on } })}
  />
  <SwitchRow label="Automatic preamp" hint="Lowers the volume before the boosts just enough that nothing clips" checked={settings.eq?.autoPreamp ?? true} onchange={(on) => m.save({ eq: { ...settings.eq, autoPreamp: on } })} />
  <ActionRow label="Tune it" hint="Open Sound, or let the assistant set it up for you">
    <button type="button" class="btn primary" onclick={() => (app.view = { name: "sound" })}><Icon name="eq" size={16} /> Open Sound</button>
    <button type="button" class="btn tonal" onclick={() => (app.view = { name: "assistant" })}><Icon name="ai" size={16} /> Set up my sound</button>
  </ActionRow>
</SettingsGroup>

<SettingsGroup title="Sound screen">
  <SwitchRow label="Sound button in the player" hint="One button next to the volume. Click opens Sound; the A key asks the AI" checked={eqUi.deck_button} onchange={(on) => saveEqUi({ deck_button: on })} />
  <SwitchRow label="Band numbers on the curve" checked={eqUi.band_labels} onchange={(on) => saveEqUi({ band_labels: on })} />
  <SwitchRow label="Tilt sliders in Edit curve" hint="Bass, treble and clarity sliders next to the full curve" checked={eqUi.taste_sliders} onchange={(on) => saveEqUi({ taste_sliders: on })} />
  <SegmentedRow label="“Hear without”" hint="Compare the music with and without the sound; nothing is saved" options={AB_MODES} value={eqUi.ab_mode} onchange={(id) => saveEqUi({ ab_mode: id })} />
</SettingsGroup>

<SettingsGroup title="Sounds">
  <SegmentedRow
    label="Change size"
    hint="Intensity: the strength of each step the assistant and “How does it sound?” take. {INTENSITIES.map((i) => `${i.label}: ${i.hint}`).join(' · ')}"
    options={INTENSITIES}
    value={taste.intensity}
    onchange={(id) => m.save({ taste: { ...taste, intensity: id } })}
  />
  <SwitchRow label="“How does it sound?” button with the player buttons" checked={soundUi.react_deck} onchange={(on) => saveSoundUi({ react_deck: on })} />
  <SwitchRow label="“How does it sound?” in the listening room" checked={soundUi.react_room} onchange={(on) => saveSoundUi({ react_room: on })} />
  <SwitchRow label="Name the sound under the song" hint="“Warm and airy” next to the format. The full “HD 650 + Warm and airy · Eagles” is in the tooltip" checked={soundUi.layer_chip} onchange={(on) => saveSoundUi({ layer_chip: on })} />
  <SwitchRow label="Suggestions for what you listen to most" checked={soundUi.suggestions} onchange={(on) => saveSoundUi({ suggestions: on })} />
  <SegmentedRow
    label="A sound's curve opens in"
    hint="Simple: bass, treble and clarity sliders. Advanced: the full curve with bands. Auto: the curve when the sound has bands"
    options={TASTE_EDITORS}
    value={soundUi.taste_editor ?? "auto"}
    onchange={(id) => saveSoundUi({ taste_editor: id })}
  />
  <ActionRow label="Your sounds" hint="Make, edit and remove sounds, or see every song with its sound">
    <button type="button" class="btn tonal" onclick={() => (app.view = { name: "sound" })}><Icon name="layers" size={16} /> Sounds</button>
    <button type="button" class="btn tonal" onclick={() => (app.view = { name: "sound", tab: "music" })}><Icon name="music" size={16} /> By music</button>
  </ActionRow>
</SettingsGroup>

<SettingsGroup title="Loudness" description="From ReplayGain (Tidal's, or the file's tags); any change of level means it isn't bit-perfect.">
  <ChoiceList label="Normalization" options={LOUDNESS_MODES} value={loudness.mode} onchange={(id) => m.save({ loudness: { ...loudness, mode: id } })} />
  {#if loudness.mode !== "off"}
    <SliderRow
      label="Target"
      hint="−14 is what streaming services use; lower is quieter with more headroom"
      min={TARGET_MIN_LUFS}
      max={TARGET_MAX_LUFS}
      step={1}
      value={loudness.targetLufs}
      format={(n) => `${n} LUFS`}
      onchange={(n) => m.save({ loudness: { ...loudness, targetLufs: n } })}
    />
    <SwitchRow label="Prevent clipping" hint="Never raises a quiet song past its loudest peak" checked={loudness.preventClipping} onchange={(on) => m.save({ loudness: { ...loudness, preventClipping: on } })} />
  {/if}
</SettingsGroup>
