<script lang="ts">
  import type { Settings } from "../../../lib/api";
  import { LAYOUTS, fixedTheme, layoutOf, skinOf } from "../../../lib/layouts";
  import SeekBar from "../../SeekBar.svelte";
  import { MOTIONS, PROGRESS_STYLES, THEMES } from "../options";
  import { useSettings } from "../model.svelte";
  import ActionRow from "../ActionRow.svelte";
  import OptionGallery from "../OptionGallery.svelte";
  import SegmentedRow from "../SegmentedRow.svelte";
  import SettingRow from "../SettingRow.svelte";
  import SettingsGroup from "../SettingsGroup.svelte";
  import SettingsNote from "../SettingsNote.svelte";
  import SwitchRow from "../SwitchRow.svelte";

  let { settings, goto }: { settings: Settings; goto: (page: "room") => void } = $props();
  const m = useSettings();

  const skin = $derived(skinOf(settings.room));
  const skinLabel = $derived(LAYOUTS.find((l) => l.id === skin)?.label ?? "");
  const lockedTheme = $derived(fixedTheme(skin));
  const layoutLabel = $derived(LAYOUTS.find((l) => l.id === layoutOf(settings.room.layout))?.label);
  const motion = $derived(settings.room.motion ?? "auto");
  const THICKNESS = [
    { id: "thin", label: "Thin" },
    { id: "medium", label: "Medium" },
    { id: "thick", label: "Thick" },
  ] as const;
  const WIDTHS = [
    { id: "comfortable", label: "Comfortable" },
    { id: "wide", label: "Wide" },
  ] as const;
  /** A progress card needs room for the live bar. */
  const BAR_CARD_MIN_PX = 220;
</script>

<SettingsGroup title="Appearance" span={2}>
  {#if lockedTheme}
    <SettingsNote>{skinLabel} is always {lockedTheme}, the look of its era. Turn off “Use the layout everywhere” to pick the theme yourself.</SettingsNote>
  {/if}
  <SettingRow label="Theme" hint="Light, dark, or whatever your system is set to" stack="always">
    {#snippet children(ids)}
      <OptionGallery options={THEMES} value={settings.theme ?? "system"} onchange={(id) => m.save({ theme: id })} labelledby={ids.label}>
        {#snippet preview(o)}
          <span class="swatch {o.id}" aria-hidden="true"><i></i><i></i></span>
        {/snippet}
      </OptionGallery>
    {/snippet}
  </SettingRow>
  <SwitchRow
    label="Use the layout everywhere"
    hint="The room layout ({layoutLabel}) also dresses the sidebar, the player bar, lists, menus and Settings"
    checked={settings.room.layout_everywhere ?? true}
    onchange={(on) => m.saveRoom({ layout_everywhere: on })}
  />
  <ActionRow label="Skin" hint="The layout is chosen in Listening room">
    <button type="button" class="btn tonal" onclick={() => goto("room")}>Choose the layout</button>
  </ActionRow>
</SettingsGroup>

<SettingsGroup title="Progress bar" description="How the seek bar under the player looks. Click or drag it to seek." span={2}>
  <SettingRow label="Style" stack="always">
    {#snippet children(ids)}
      <OptionGallery options={PROGRESS_STYLES} value={settings.progress_style ?? "wavy"} onchange={(id) => m.save({ progress_style: id })} labelledby={ids.label} min={BAR_CARD_MIN_PX}>
        {#snippet preview(o)}
          <SeekBar fraction={0.62} style={o.id} thickness={settings.progress_thickness} animated={settings.progress_animated} playing seed={7} />
        {/snippet}
      </OptionGallery>
    {/snippet}
  </SettingRow>
  <SegmentedRow label="Thickness" options={THICKNESS} value={settings.progress_thickness} onchange={(id) => m.save({ progress_thickness: id })} />
  <SwitchRow label="Animate while playing" checked={settings.progress_animated} onchange={(on) => m.save({ progress_animated: on })} />
</SettingsGroup>

<SettingsGroup title="Motion" description="How the app moves: views sliding in, menus and dialogs opening, lists arriving, the song changing. The skin gives it its character; the retro ones move in a few quick frames.">
  <SegmentedRow label="Motion" hint={MOTIONS.find((o) => o.id === motion)?.hint} options={MOTIONS} value={motion} onchange={(id) => m.saveRoom({ motion: id })} />
</SettingsGroup>

<SettingsGroup title="This screen">
  <SegmentedRow
    label="Settings width"
    hint={settings.room.settings_width === "wide" ? "One broader column, never side by side" : "A comfortable column; very wide windows show two columns of groups"}
    options={WIDTHS}
    value={settings.room.settings_width ?? "comfortable"}
    onchange={(id) => m.saveRoom({ settings_width: id })}
  />
</SettingsGroup>

<style>
  /* mini previews of the three themes, drawn in their own palettes */
  .swatch {
    display: grid;
    grid-template-columns: 1fr 1fr;
    height: 48px;
    overflow: hidden;
    border-radius: min(8px, var(--r-max));
    border: 1px solid var(--border);
  }
  .swatch i {
    display: block;
  }
  .light i {
    background: #f7fbfc;
  }
  .light i + i {
    background: #ffffff;
  }
  .dark i {
    background: #0a121d;
  }
  .dark i + i {
    background: #172436;
  }
  .system i:first-child {
    background: #f7fbfc;
  }
  .system i:last-child {
    background: #0a121d;
  }
</style>
