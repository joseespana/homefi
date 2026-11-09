<script lang="ts">
  // Settings → DJ: every option of homefi_core's `DjSettings`, in plain words. The same JSON as
  // Android's; the core clamps whatever is sent.
  import { actionLabel, dj, keyOf, printKey } from "../../../lib/dj.svelte";
  import { DJ_BEND_RANGE, DJ_EQ_BOOST_RANGE, DJ_MEMORY_RANGE, DJ_MIX_BEATS, DJ_TEMPO_RANGE, DJ_ZOOM_RANGE, djApi, djSettingsOf, type CrossfaderCurve, type DjBinding, type DjLatency, type DjSettings, type LoadTarget } from "../../../lib/dj-api";
  import type { Settings } from "../../../lib/api";
  import { isDefaultLayout } from "../../../lib/dj-layout";
  import type { HomifitoMode, HomifitoPlace } from "../../../lib/homifito-dj";
  import { homifitoDj } from "../../../lib/homifito-dj.svelte";
  import { app } from "../../../lib/state.svelte";
  import DjSetupsPanel from "../../dj/DjSetupsPanel.svelte";
  import { useSettings } from "../model.svelte";
  import ActionRow from "../ActionRow.svelte";
  import SegmentedRow from "../SegmentedRow.svelte";
  import SettingsGroup from "../SettingsGroup.svelte";
  import SettingsIntro from "../SettingsIntro.svelte";
  import SliderRow from "../SliderRow.svelte";
  import SwitchRow from "../SwitchRow.svelte";
  import type { SegmentOption } from "../types";

  let { settings }: { settings: Settings } = $props();
  const m = useSettings();

  const d = $derived(djSettingsOf(settings));
  async function save(patch: Partial<DjSettings>) {
    await m.save({ dj: { ...d, ...patch } });
    if ("key_overrides" in patch || "keys_enabled" in patch) loadKeys();
  }

  /** Opens the DJ screen with the layout editor showing. */
  function editLayout() {
    dj.editing = true;
    app.view = { name: "dj" };
  }

  const LOAD_TARGETS: SegmentOption<LoadTarget>[] = [
    { id: "free_deck", label: "The free deck", hint: "The deck that isn't playing (deck A when both are stopped)" },
    { id: "ask", label: "Ask me", hint: "A small window asks: deck A, deck B or the crate" },
    { id: "crate", label: "The crate", hint: "It waits in the crate's Queue tab" },
  ];
  const HOMIFITO_MODES: SegmentOption<HomifitoMode>[] = [
    { id: "off", label: "Off" },
    { id: "booth", label: "Booth", hint: "A small strip with Homifito at his console" },
    { id: "big", label: "Big stage", hint: "A taller stage so you see everything he does" },
  ];
  const HOMIFITO_PLACES: SegmentOption<HomifitoPlace>[] = [
    { id: "top", label: "Above the decks" },
    { id: "bottom", label: "Below" },
  ];
  const MIX_LENGTHS: SegmentOption<string>[] = DJ_MIX_BEATS.map((b) => ({ id: String(b), label: `${b} beats` }));
  const CURVES: SegmentOption<CrossfaderCurve>[] = [
    { id: "smooth", label: "Smooth", hint: "Even loudness all the way across" },
    { id: "linear", label: "Linear", hint: "Straight volume ramps" },
    { id: "cut", label: "Cut", hint: "Both at full volume until the ends (for scratching)" },
  ];
  const LATENCIES: SegmentOption<DjLatency>[] = [
    { id: "low", label: "Quick", hint: "About 5 ms: the controls answer at once; needs a computer that isn't busy" },
    { id: "balanced", label: "Balanced", hint: "About 11 ms" },
    { id: "safe", label: "Safe", hint: "About 21 ms: never stutters, even with a busy computer" },
  ];

  // ------------------------------------------------------------ shortcuts
  let bindings = $state.raw<DjBinding[]>([]);
  let conflicts = $state.raw<[string, string[]][]>([]);
  let capturing = $state<string | null>(null);
  async function loadKeys() {
    try {
      const k = await djApi.keymap();
      bindings = k.bindings;
      conflicts = k.conflicts;
      dj.refreshKeys();
    } catch {}
  }
  $effect(() => {
    loadKeys();
  });
  /** Every action, bound or not (an unbound one is missing from the resolved list). */
  const ACTIONS = $derived([...new Set([...bindings.map((b) => b.action), ...Object.keys(d.key_overrides)])].sort());
  const keyText = (action: string) => {
    const k = bindings.find((b) => b.action === action)?.key;
    return k ? printKey(k) : "None";
  };
  const conflicted = $derived(new Set(conflicts.flatMap(([, actions]) => actions)));
  function capture(e: KeyboardEvent) {
    if (!capturing) return;
    if (["Shift", "Alt", "Control", "Meta"].includes(e.key)) return;
    e.preventDefault();
    e.stopPropagation();
    const action = capturing;
    capturing = null;
    if (e.key === "Escape") return;
    const key = e.key === "Backspace" || e.key === "Delete" ? "" : keyOf(e);
    save({ key_overrides: { ...d.key_overrides, [action]: key } });
  }
  const pretty = (action: string) => (action.startsWith("deck_a.") ? "Deck A · " : action.startsWith("deck_b.") ? "Deck B · " : "") + actionLabel(action);
</script>

<svelte:window onkeydowncapture={capture} />

<SettingsIntro>Mix two songs like a DJ, with the mouse. DJ mode follows Look → Motion and the app's skin; the sound it makes is mixed, so it's never bit-perfect.</SettingsIntro>

<SettingsGroup title="Screen layout" description="Arrange the DJ screen your way: the order of the decks, mixer, sampler and crate, which ones show, and the sound on each sampler pad.">
  <ActionRow label="Edit the layout" hint="Opens the DJ screen with the layout editor: drag the sections, hide some, choose the pad sounds.">
    <button class="btn tonal" type="button" onclick={editLayout}>Edit layout</button>
    <button class="btn tonal" type="button" disabled={isDefaultLayout(dj.layout)} onclick={() => dj.resetLayout()}>Reset layout</button>
  </ActionRow>
</SettingsGroup>

<SettingsGroup title="Homifito DJ" description="Homifito stands behind his own little booth with two turntables, a mixer and lights, and works with you: he bobs on the beat, scratches, reaches for the crossfader during a mix, and cheers when you fire a pad. It follows Look → Motion (calm on Subtle, still on Off). The button in the DJ top bar turns him on and off.">
  <SegmentedRow label="Homifito" hint="Off hides him." options={HOMIFITO_MODES} value={homifitoDj.mode} onchange={(mode) => homifitoDj.setMode(mode)} />
  {#if homifitoDj.on}
    <SegmentedRow label="Where he stands" options={HOMIFITO_PLACES} value={homifitoDj.place} onchange={(place) => homifitoDj.setPlace(place)} />
  {/if}
  <SwitchRow label="Homifito colours" hint="The whole DJ board in his colours: mint deck A, pink deck B, navy waveforms. Works in light and dark." checked={homifitoDj.look} onchange={(look) => homifitoDj.setLook(look)} />
</SettingsGroup>

<SettingsGroup title="Setups" description="Save everything about DJ mode (layout, pad sounds, shortcuts and options) under a name, and switch between setups. Export one to a .homefidj file to use it on another computer, phone or tablet, or keep it in your Google Drive folder.">
  <div class="setups"><DjSetupsPanel /></div>
</SettingsGroup>

<SettingsGroup title="Easy mixing" description="For a party, without being a DJ.">
  <SwitchRow label="Simple view" hint="Hides hot cues, loops, trim and the tempo faders; Sync does the matching. “Show all controls” on the DJ screen turns it off." checked={d.simple_view} onchange={(simple_view) => save({ simple_view })} />
  <SwitchRow label="Match the tempo by itself" hint="A song loaded while the other deck plays takes its speed and beat." checked={d.sync_on_load} onchange={(sync_on_load) => save({ sync_on_load })} />
  <SegmentedRow label="Mix length" hint="How many beats “Mix to B” takes. Longer is smoother." options={MIX_LENGTHS} value={String(d.auto_mix_beats)} onchange={(v) => save({ auto_mix_beats: Number(v) })} />
  <SwitchRow label="Swap the bass during a mix" hint="Halfway through, the old song's bass goes out as the new one's comes in, so the two basses don't pile up." checked={d.bass_swap} onchange={(bass_swap) => save({ bass_swap })} />
  <SegmentedRow label="A song you pick goes to" hint="While DJ mode is on, a song you play anywhere in the app goes here instead of the player." options={LOAD_TARGETS} value={d.load_target} onchange={(load_target) => save({ load_target })} />
  <SwitchRow label="Show the first-mix tips" hint="The “Your first mix” card at the top of the DJ screen." checked={d.show_tips} onchange={(show_tips) => save({ show_tips })} />
  <SwitchRow label="DJ in the sidebar" hint="The button that opens DJ mode. While DJ mode is on it stays." checked={d.show_entry} onchange={(show_entry) => save({ show_entry })} />
</SettingsGroup>

<SettingsGroup title="Decks">
  <SliderRow label="Tempo fader range" hint="How far the tempo fader goes up and down. The button under it switches 8 / 16 / 50 %." min={DJ_TEMPO_RANGE.min} max={DJ_TEMPO_RANGE.max} step={1} value={d.tempo_range_pct} format={(n) => `±${n} %`} onchange={(tempo_range_pct) => save({ tempo_range_pct })} />
  <SliderRow label="Nudge strength" hint="How much the − / + buttons and dragging the waveform speed a song up or slow it down while held." min={DJ_BEND_RANGE.min} max={DJ_BEND_RANGE.max} step={0.5} value={d.bend_pct} format={(n) => `${n} %`} onchange={(bend_pct) => save({ bend_pct })} />
  <SwitchRow label="Snap to the beat" hint="Cue points, hot cues and loops land exactly on a beat." checked={d.quantize} onchange={(quantize) => save({ quantize })} />
  <SwitchRow label="Even out the loudness" hint="Each song is brought to the same level when it loads, so one isn't louder than the other." checked={d.auto_gain} onchange={(auto_gain) => save({ auto_gain })} />
</SettingsGroup>

<SettingsGroup title="Mixer">
  <SegmentedRow label="Crossfader curve" hint="How the crossfader blends deck A into deck B." options={CURVES} value={d.crossfader_curve} onchange={(crossfader_curve) => save({ crossfader_curve })} />
  <SliderRow label="Most an EQ knob can boost" hint="Turning a knob right adds up to this much; left always goes down to silent." min={DJ_EQ_BOOST_RANGE.min} max={DJ_EQ_BOOST_RANGE.max} step={1} value={d.eq_max_boost_db} format={(n) => `${n} dB`} onchange={(eq_max_boost_db) => save({ eq_max_boost_db })} />
  <SwitchRow label="Protect from distortion" hint="A soft limiter on the output keeps loud moments from clipping." checked={d.master_limiter} onchange={(master_limiter) => save({ master_limiter })} />
  <SliderRow label="Sound effects volume" hint="The sampler pads, on top of the music." min={0} max={100} step={5} value={Math.round(d.sampler_volume * 100)} format={(n) => `${n} %`} onchange={(n) => save({ sampler_volume: n / 100 })} />
</SettingsGroup>

<SettingsGroup title="Waveforms">
  <SwitchRow label="Coloured waveforms" hint="Bass orange, voice in the deck's colour, treble white. Off draws them in one colour." checked={d.waveform_colors} onchange={(waveform_colors) => save({ waveform_colors })} />
  <SliderRow label="Zoomed waveform shows" hint="Beats around the playhead. The wheel over the waveform changes it for now." min={DJ_ZOOM_RANGE.min} max={DJ_ZOOM_RANGE.max} step={1} value={d.zoom_beats} format={(n) => `${n} beats`} onchange={(zoom_beats) => save({ zoom_beats })} />
  <SwitchRow label="Beat grid" hint="Lines on every beat (thicker every bar). Hidden on songs whose beat isn't clear." checked={d.show_beat_grid} onchange={(show_beat_grid) => save({ show_beat_grid })} />
</SettingsGroup>

<SettingsGroup title="Keyboard shortcuts" description="All optional: everything works with the mouse.">
  <SwitchRow label="Keyboard shortcuts" hint="Only on the DJ screen, and never while you type." checked={d.keys_enabled} onchange={(keys_enabled) => save({ keys_enabled })} />
  {#if d.keys_enabled}
    {#each ACTIONS as action (action)}
      <ActionRow label={pretty(action)} hint={conflicted.has(action) ? "This key is also used by another shortcut." : undefined}>
        <button class="btn tonal key" class:warn={conflicted.has(action)} type="button" aria-label="Change the key for {pretty(action)}" onclick={() => (capturing = action)}>
          {capturing === action ? "Press a key… (Esc cancels, Delete clears)" : keyText(action)}
        </button>
      </ActionRow>
    {/each}
    <ActionRow label="Back to the defaults" hint="Every shortcut as HomeFi ships it.">
      <button class="btn tonal" type="button" disabled={!Object.keys(d.key_overrides).length} onclick={() => save({ key_overrides: {} })}>Reset shortcuts</button>
    </ActionRow>
  {/if}
</SettingsGroup>

<SettingsGroup title="Performance" description="Takes effect the next time DJ mode starts.">
  <SegmentedRow label="Response" hint="A smaller audio buffer answers quicker; a bigger one is safer on a busy computer." options={LATENCIES} value={d.latency} onchange={(latency) => save({ latency })} />
  <SliderRow label="Memory for the decks" hint="Songs are kept whole in memory while on a deck; a long hi-res song needs more." min={DJ_MEMORY_RANGE.min} max={DJ_MEMORY_RANGE.max} step={DJ_MEMORY_RANGE.step} value={d.memory_mb} format={(n) => (n >= 1024 ? `${(n / 1024).toFixed(n % 1024 ? 1 : 0)} GB` : `${n} MB`)} onchange={(memory_mb) => save({ memory_mb })} />
</SettingsGroup>

<style>
  .setups {
    padding: var(--s-4, 14px);
  }
  .key {
    min-width: 72px;
    justify-content: center;
    font-family: var(--font-mono);
  }
  .key.warn {
    color: var(--warn);
    box-shadow: inset 0 0 0 1px var(--warn-border);
  }
</style>
