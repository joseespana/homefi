<script lang="ts">
  // Settings → Discs: when HomeFi notices a disc, how it names, reads and saves the songs of an
  // audio CD. The quality block is the same component as on the Disc screen. Every value of
  // homefi-disc's `DiscSettings` that the app uses is here.
  import { DISC_WATCH_SECONDS_RANGE, discSettingsOf, type DiscOpenOnInsert, type DiscRailItem, type DiscSettings, type Settings } from "../../../lib/api";
  import { disc } from "../../../lib/disc.svelte";
  import { useSettings } from "../model.svelte";
  import DiscQuality from "../../disc/DiscQuality.svelte";
  import FieldRow from "../FieldRow.svelte";
  import SegmentedRow from "../SegmentedRow.svelte";
  import SettingsGroup from "../SettingsGroup.svelte";
  import SettingsIntro from "../SettingsIntro.svelte";
  import SliderRow from "../SliderRow.svelte";
  import SwitchRow from "../SwitchRow.svelte";
  import type { SegmentOption } from "../types";

  let { settings }: { settings: Settings } = $props();
  const m = useSettings();

  const TEMPLATE_EXAMPLE = "{album.artist}/{album.title}/{item.title}";
  const d = $derived(discSettingsOf(settings));
  const save = (patch: Partial<DiscSettings>) => m.save({ disc: { ...d, ...patch } });

  const OPEN_ON_INSERT: SegmentOption<DiscOpenOnInsert>[] = [
    { id: "notice", label: "Tell me", hint: "A notice with an Open button" },
    { id: "open", label: "Open the Disc screen" },
    { id: "off", label: "Do nothing" },
  ];
  const RAIL_ITEM: SegmentOption<DiscRailItem>[] = [
    { id: "whenInserted", label: "When a disc is in" },
    { id: "always", label: "Always" },
    { id: "never", label: "Never" },
  ];
</script>

<SettingsIntro>Rip audio CDs to your library in the quality you choose. HomeFi doesn't bypass copy protection, so protected DVDs and Blu-rays can't be copied.</SettingsIntro>

<SettingsGroup title="When a disc goes in">
  <SwitchRow label="Notice discs" hint="Looks at the drives every few seconds and tells you when a disc is inserted." checked={d.watch} onchange={(watch) => save({ watch })} />
  <SliderRow label="Look every" min={DISC_WATCH_SECONDS_RANGE.min} max={DISC_WATCH_SECONDS_RANGE.max} step={1} value={d.watchSeconds} format={(n) => `${n} seconds`} onchange={(watchSeconds) => save({ watchSeconds })} disabled={!d.watch} />
  <SegmentedRow label="Then" options={OPEN_ON_INSERT} value={d.openOnInsert} onchange={(openOnInsert) => save({ openOnInsert })} disabled={!d.watch} />
  <SegmentedRow label="Disc in the sidebar" hint="The button that opens the Disc screen." options={RAIL_ITEM} value={d.railItem} onchange={(railItem) => save({ railItem })} />
</SettingsGroup>

<DiscQuality settings={d} driveId={disc.drive?.id ?? null} advanced {save} />

<SettingsGroup title="Names and covers" description="HomeFi asks you for the names before ripping.">
  <SwitchRow label="Use the names on the disc" hint="Some discs carry the artist and song titles (CD-Text). They fill the form, and you can change them." checked={d.sources.cdText} onchange={(cdText) => save({ sources: { ...d.sources, cdText } })} />
  <SwitchRow
    label="Look up discs on MusicBrainz"
    hint="Off by default. Adds a “Look it up” button to the Disc screen; pressing it sends the disc's table of contents (not your name or files) to musicbrainz.org."
    checked={d.sources.musicbrainz}
    onchange={(musicbrainz) => save({ sources: { ...d.sources, musicbrainz } })}
  />
  <SwitchRow label="Also save the cover as a file" hint="A cover picture you choose is embedded in the songs (Settings → Downloads); this also puts it in the album folder." checked={d.cover.saveFile} onchange={(saveFile) => save({ cover: { ...d.cover, saveFile } })} />
</SettingsGroup>

<SettingsGroup title="Files">
  <FieldRow
    label="File names"
    hint="Leave empty to name them like downloads: artist / album / song."
    placeholder={TEMPLATE_EXAMPLE}
    mono
    value={d.template ?? ""}
    onchange={(text) => save({ template: text.trim() || null })}
  />
  <SwitchRow label="Write a log" hint="A text file next to the songs with the drive, the settings and how each song was read." checked={d.writeLog} onchange={(writeLog) => save({ writeLog })} />
  <SwitchRow label="Write a cue sheet" hint="For people who burn the disc again or use players that read .cue files." checked={d.writeCue} onchange={(writeCue) => save({ writeCue })} />
  <SwitchRow label="Hide the 16-bit / 44.1 kHz note" hint="The note about CD audio and ffmpeg in the format list." checked={d.hideCdOnlyNote} onchange={(hideCdOnlyNote) => save({ hideCdOnlyNote })} />
</SettingsGroup>
