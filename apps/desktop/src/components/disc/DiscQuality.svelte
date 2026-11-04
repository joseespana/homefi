<script lang="ts">
  // The quality of a rip: format, how carefully the disc is read, and where the files go. The
  // same values as Settings → Discs, so this component sits in both places (the Disc screen asks
  // for the few that matter now; `advanced` adds the drive-specific ones). Changes persist.
  import { open } from "@tauri-apps/plugin-dialog";
  import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
  import { DISC_AGREE_RANGE, DISC_FLAC_LEVEL_RANGE, DISC_OFFSET_RANGE, DISC_REREADS_RANGE, type DiscC2, type DiscSettings, type DiscSpeed, type RipFormat, type RipFormatKind } from "../../lib/api";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";
  import type { Choice, SegmentOption } from "../settings/types";
  import ActionRow from "../settings/ActionRow.svelte";
  import ChoiceList from "../settings/ChoiceList.svelte";
  import FieldRow from "../settings/FieldRow.svelte";
  import SegmentedRow from "../settings/SegmentedRow.svelte";
  import SettingsGroup from "../settings/SettingsGroup.svelte";
  import SettingsNote from "../settings/SettingsNote.svelte";
  import SliderRow from "../settings/SliderRow.svelte";
  import SwitchRow from "../settings/SwitchRow.svelte";

  let { settings, driveId = null, advanced = false, save }: { settings: DiscSettings; driveId?: string | null; advanced?: boolean; save: (patch: Partial<DiscSettings>) => void } = $props();

  /** Where AccurateRip publishes the read offset of each drive model. */
  const OFFSET_LIST_URL = "https://www.accuraterip.com/driveoffsets.htm";
  const MP3_KINDS = [
    { id: "cbr320", label: "320 kbps", hint: "Constant bit rate: the same size for every song" },
    { id: "v0", label: "V0", hint: "Variable bit rate, about 245 kbps: as good, smaller" },
  ] as const satisfies readonly SegmentOption<"cbr320" | "v0">[];
  const AAC_KBPS = [128, 192, 256, 320];
  const OPUS_KBPS = [96, 128, 160, 192];
  const SPEEDS: DiscSpeed[] = ["auto", "max", 4, 8, 16, 24];

  const ffmpeg = $derived(app.info?.ffmpeg ?? false);
  const noFfmpeg = "Needs ffmpeg, which HomeFi didn't find on this computer. Install it and this works at once.";
  const f = $derived(settings.format);

  const formats = $derived<Choice<RipFormatKind>[]>([
    { id: "flac", label: "FLAC", hint: "Lossless and smaller than the disc. The best choice for keeping your CDs." },
    { id: "wav", label: "WAV", hint: "Lossless and untouched, about twice the size of FLAC." },
    { id: "alac", label: "ALAC", hint: "Lossless for Apple devices.", disabled: !ffmpeg, disabledReason: noFfmpeg },
    { id: "mp3", label: "MP3", hint: "Small and plays everywhere.", disabled: !ffmpeg, disabledReason: noFfmpeg },
    { id: "aac", label: "AAC", hint: "Small and clean on phones.", disabled: !ffmpeg, disabledReason: noFfmpeg },
    { id: "opus", label: "Opus", hint: "The smallest for the sound it keeps.", disabled: !ffmpeg, disabledReason: noFfmpeg },
  ]);

  const FORMAT_OF: Record<RipFormatKind, RipFormat> = {
    flac: { kind: "flac", level: 5 },
    wav: { kind: "wav" },
    alac: { kind: "alac" },
    mp3: { kind: "mp3", mp3: "cbr320" },
    aac: { kind: "aac", kbps: 256 },
    opus: { kind: "opus", kbps: 160 },
  };
  /** Choosing the format you already have keeps its detail (the FLAC level, the bit rate). */
  const choose = (kind: RipFormatKind) => (f.kind === kind ? undefined : save({ format: FORMAT_OF[kind] }));
  const kbpsOptions = (list: number[], now: number): SegmentOption<string>[] => (list.includes(now) ? list : [...list, now].sort((a, b) => a - b)).map((k) => ({ id: String(k), label: `${k} kbps` }));

  const speedId = (s: DiscSpeed) => String(s);
  const speedOptions = $derived<SegmentOption<string>[]>((SPEEDS.includes(settings.speed) ? SPEEDS : [...SPEEDS, settings.speed]).map((s) => ({ id: speedId(s), label: s === "auto" ? "Auto" : s === "max" ? "Max" : `${s}×` })));
  const toSpeed = (id: string): DiscSpeed => (id === "auto" || id === "max" ? id : Number(id));

  const offset = $derived(driveId ? (settings.offsets[driveId] ?? 0) : 0);
  function saveOffset(text: string) {
    if (!driveId) return;
    const n = Math.round(Number(text));
    const clamped = Number.isFinite(n) ? Math.min(DISC_OFFSET_RANGE.max, Math.max(DISC_OFFSET_RANGE.min, n)) : 0;
    save({ offsets: { ...settings.offsets, [driveId]: clamped } });
  }

  async function pickDir() {
    const dir = await open({ directory: true, defaultPath: settings.ripDir ?? app.settings?.download_dir });
    if (typeof dir === "string") save({ ripDir: dir });
  }
  const dir = $derived(settings.ripDir ?? app.settings?.download_dir ?? "");
</script>

<SettingsGroup title="Format" description="What the songs are saved as.">
  {#if !settings.hideCdOnlyNote}
    <SettingsNote>CD audio is 16-bit / 44.1 kHz, so a higher bit depth or sample rate would add nothing. ALAC, MP3, AAC and Opus need ffmpeg; FLAC and WAV don't.</SettingsNote>
  {/if}
  <ChoiceList label="Format" options={formats} value={f.kind} onchange={choose} />
  {#if f.kind === "flac"}
    <SliderRow
      label="FLAC level"
      hint="Higher makes smaller files and takes longer. The sound is the same at every level."
      min={DISC_FLAC_LEVEL_RANGE.min}
      max={DISC_FLAC_LEVEL_RANGE.max}
      step={1}
      value={f.level}
      format={(n) => (n === 5 ? "5 (default)" : String(n))}
      onchange={(level) => save({ format: { kind: "flac", level } })}
    />
    <SwitchRow label="Check each FLAC file" hint="Decodes the file and compares it with what was read, so “lossless” is proven. Adds about a fifth to the time." checked={settings.flacVerify} onchange={(on) => save({ flacVerify: on })} />
  {:else if f.kind === "mp3"}
    <SegmentedRow label="MP3 type" options={MP3_KINDS} value={f.mp3} onchange={(mp3) => save({ format: { kind: "mp3", mp3 } })} />
  {:else if f.kind === "aac"}
    <SegmentedRow label="AAC bit rate" options={kbpsOptions(AAC_KBPS, f.kbps)} value={String(f.kbps)} onchange={(k) => save({ format: { kind: "aac", kbps: Number(k) } })} />
  {:else if f.kind === "opus"}
    <SegmentedRow label="Opus bit rate" options={kbpsOptions(OPUS_KBPS, f.kbps)} value={String(f.kbps)} onchange={(k) => save({ format: { kind: "opus", kbps: Number(k) } })} />
  {/if}
</SettingsGroup>

<SettingsGroup title="Reading" description="Discs get scratched. Careful reading keeps the copy exact.">
  <SegmentedRow
    label="Reading mode"
    hint={settings.mode === "secure" ? "Reads every part until it comes out the same twice. Slower, and the copy is checked." : "Reads once and only repeats what the drive says went wrong. Quick; fine for clean discs."}
    options={[
      { id: "secure", label: "Secure" },
      { id: "fast", label: "Fast" },
    ]}
    value={settings.mode}
    onchange={(mode) => save({ mode })}
  />
  {#if advanced}
    <SliderRow label="Identical reads needed" hint="Secure mode: how many reads must agree before a part is trusted." min={DISC_AGREE_RANGE.min} max={DISC_AGREE_RANGE.max} step={1} value={settings.agree} onchange={(agree) => save({ agree })} disabled={settings.mode === "fast"} />
    <SliderRow label="Re-reads per part, at most" hint="After this many tries a part is marked in the log and the ripping goes on." min={DISC_REREADS_RANGE.min} max={DISC_REREADS_RANGE.max} step={1} value={settings.maxRereads} onchange={(maxRereads) => save({ maxRereads })} />
    <SegmentedRow
      label="Error flags (C2)"
      hint="Many drives tell which parts they couldn't read well. Auto uses that when the drive says it can."
      options={[
        { id: "auto", label: "Auto" },
        { id: "on", label: "On" },
        { id: "off", label: "Off" },
      ]}
      value={settings.c2}
      onchange={(c2: DiscC2) => save({ c2 })}
    />
    <FieldRow
      label="Drive read offset"
      hint={driveId ? "In samples. Every drive model reads a little early or late; look yours up in the list below. Saved for this drive only." : "Insert a disc to set the offset of its drive."}
      type="number"
      min={DISC_OFFSET_RANGE.min}
      max={DISC_OFFSET_RANGE.max}
      inputmode="numeric"
      width="120px"
      value={offset}
      disabled={!driveId}
      onchange={saveOffset}
    />
    <ActionRow label="List of drive offsets" hint="Published by AccurateRip. Find your drive's model and type its number above.">
      <button type="button" class="btn tonal" onclick={() => openUrl(OFFSET_LIST_URL)}>Open the list</button>
    </ActionRow>
    <SegmentedRow label="Drive speed" hint="A slower drive often reads a scratched disc better. A best effort: some drives ignore it." options={speedOptions} value={speedId(settings.speed)} onchange={(id) => save({ speed: toSpeed(id) })} />
    <SegmentedRow
      label="Parts that can't be read"
      hint="Keep the song and mark the spots in the log, or leave that song out."
      options={[
        { id: "keep", label: "Keep the song" },
        { id: "skip", label: "Skip the song" },
      ]}
      value={settings.onUnreadable}
      onchange={(onUnreadable) => save({ onUnreadable })}
    />
  {/if}
</SettingsGroup>

<SettingsGroup title="Save to">
  <ActionRow label="Folder" hint={settings.ripDir ? dir : `${dir} (the downloads folder)`}>
    {#if dir}<button type="button" class="icon-btn" title="Show in Finder / Explorer" aria-label="Show the folder in Finder / Explorer" onclick={() => revealItemInDir(dir)}><Icon name="external" /></button>{/if}
    {#if settings.ripDir}<button type="button" class="btn tonal" onclick={() => save({ ripDir: null })}>Use downloads folder</button>{/if}
    <button type="button" class="btn tonal" onclick={pickDir}>Change</button>
  </ActionRow>
  <SwitchRow label="Eject when done" checked={settings.ejectWhenDone} onchange={(on) => save({ ejectWhenDone: on })} />
</SettingsGroup>
