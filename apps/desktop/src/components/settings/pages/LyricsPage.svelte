<script lang="ts">
  import type { Settings } from "../../../lib/api";
  import { SIZE_LABELS, offsetLabel } from "../options";
  import { useSettings } from "../model.svelte";
  import ChoiceList from "../ChoiceList.svelte";
  import SegmentedRow from "../SegmentedRow.svelte";
  import SettingsGroup from "../SettingsGroup.svelte";
  import SettingsIntro from "../SettingsIntro.svelte";
  import SliderRow from "../SliderRow.svelte";
  import SwitchRow from "../SwitchRow.svelte";

  let { settings }: { settings: Settings } = $props();
  const m = useSettings();

  const MODES = [
    { id: "normal", label: "Normal", hint: "The whole text, the line being sung lit and followed" },
    { id: "karaoke", label: "Karaoke", hint: "The line being sung, big, filling with color as it goes" },
  ] as const;
  const SIZES = (["small", "medium", "large"] as const).map((id) => ({ id, label: SIZE_LABELS[id] }));
  /** Lyrics can move this far (ms) against the music. */
  const OFFSET_MAX_MS = 2000;
  const OFFSET_STEP_MS = 100;
</script>

<SettingsIntro>The song's words in the listening room: open them with the Lyrics button in the room or the player bar. They come from the file (a .lrc next to it, or the lyrics tag) or from Tidal.</SettingsIntro>

<SettingsGroup title="Reading">
  <ChoiceList label="Mode" options={MODES} value={settings.room.lyrics_mode === "karaoke" ? "karaoke" : "normal"} onchange={(id) => m.saveRoom({ lyrics_mode: id })} />
  <SegmentedRow label="Text size" options={SIZES} value={settings.room.lyrics_size} onchange={(id) => m.saveRoom({ lyrics_size: id })} />
  <SliderRow
    label="Lyrics timing"
    hint="Bluetooth speakers and headphones play a little late: move the lyrics later until they match the voice"
    min={-OFFSET_MAX_MS}
    max={OFFSET_MAX_MS}
    step={OFFSET_STEP_MS}
    value={settings.room.lyrics_offset_ms}
    format={offsetLabel}
    onchange={(ms) => m.saveRoom({ lyrics_offset_ms: ms })}
  />
</SettingsGroup>

<SettingsGroup title="Behaviour">
  <SwitchRow label="Open the lyrics on their own" hint="When a song that has lyrics starts, the room shows them" checked={settings.room.lyrics_auto_open} onchange={(on) => m.saveRoom({ lyrics_auto_open: on })} />
  <SwitchRow label="HomeFito sings in karaoke" hint="The mascot stands beside the words, singing into a microphone while the song plays" checked={settings.room.lyrics_mascot} onchange={(on) => m.saveRoom({ lyrics_mascot: on })} />
</SettingsGroup>
