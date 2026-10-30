<script lang="ts">
  import type { Settings } from "../../../lib/api";
  import { LAYOUTS, layoutOf } from "../../../lib/layouts";
  import { useSettings } from "../model.svelte";
  import ChoiceList from "../ChoiceList.svelte";
  import OptionGallery from "../OptionGallery.svelte";
  import SegmentedRow from "../SegmentedRow.svelte";
  import SettingRow from "../SettingRow.svelte";
  import SettingsGroup from "../SettingsGroup.svelte";
  import SettingsIntro from "../SettingsIntro.svelte";
  import SwitchRow from "../SwitchRow.svelte";

  let { settings }: { settings: Settings } = $props();
  const m = useSettings();

  const layout = $derived(layoutOf(settings.room.layout));
  const SHADES = [
    { id: "auto", label: "Auto" },
    { id: "strong", label: "Strong" },
    { id: "soft", label: "Soft" },
    { id: "off", label: "Off" },
  ] as const;
  const CHAINS = [
    { id: "compact", label: "Compact", hint: "Small stages joined by a flowing signal" },
    { id: "detailed", label: "Detailed", hint: "Big boxes with every stage spelled out" },
    { id: "off", label: "Off", hint: "Not shown" },
  ] as const;
  type Chain = (typeof CHAINS)[number]["id"];
  const chain = $derived<Chain>(settings.room.signal_chain === false ? "off" : (settings.room.signal_style ?? "compact"));
  const SHOWN: { key: "vu_meters" | "visualizer" | "halo" | "album_colors"; label: string }[] = [
    { key: "vu_meters", label: "VU meters" },
    { key: "visualizer", label: "Visualizer" },
    { key: "halo", label: "Halo around the cover" },
    { key: "album_colors", label: "Colors from the album art" },
  ];
</script>

<SettingsIntro>Choose what the now-playing room shows. Everything can be turned off.</SettingsIntro>

<SettingsGroup title="Layout" span={2}>
  <SettingRow label="Layout" hint="The look of the room, and of the whole app when it is used everywhere" stack="always">
    {#snippet children(ids)}
      <OptionGallery options={LAYOUTS} value={layout} onchange={(id) => m.saveRoom({ layout: id })} labelledby={ids.label} />
    {/snippet}
  </SettingRow>
  <SwitchRow
    label="Use the layout everywhere"
    hint="The whole app takes the layout's look, not only the now-playing room. Studio and Retro Amp are always dark; Luna, Aluminum and Web 2000 always light"
    checked={settings.room.layout_everywhere ?? true}
    onchange={(on) => m.saveRoom({ layout_everywhere: on })}
  />
  {#if layout === "immersive"}
    <SegmentedRow label="Shade under the title" hint="So the title reads on any album. Auto makes it darker on light covers." options={SHADES} value={settings.room.immersive_shade ?? "auto"} onchange={(id) => m.saveRoom({ immersive_shade: id })} />
  {/if}
</SettingsGroup>

<SettingsGroup title="Signal chain" description="The path of the sound: the file, what works on it (resampling, loudness, equalizer) and the output. Point at a stage for everything about it.">
  <ChoiceList label="Signal chain" options={CHAINS} value={chain} onchange={(id) => m.saveRoom(id === "off" ? { signal_chain: false } : { signal_chain: true, signal_style: id })} />
  {#if settings.room.signal_chain !== false && settings.room.signal_style !== "detailed"}
    <SwitchRow
      label="Animate the signal"
      hint="The links flow while the music plays, swinging with it; still when paused. Motion off or subtle keeps them still too"
      checked={settings.room.signal_animate ?? true}
      onchange={(on) => m.saveRoom({ signal_animate: on })}
    />
  {/if}
</SettingsGroup>

<SettingsGroup title="Show">
  {#each SHOWN as s (s.key)}
    <SwitchRow label={s.label} checked={settings.room[s.key]} onchange={(on) => m.saveRoom({ [s.key]: on })} />
  {/each}
</SettingsGroup>
