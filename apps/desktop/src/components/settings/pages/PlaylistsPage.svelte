<script lang="ts">
  // Settings → Playlists: how "Get ready for a concert" reads the shows, and what the playlists
  // HomeFi makes look like. The thresholds' order (Always >= Usually >= Sometimes) is kept by the
  // core; what it answers is what is shown here.
  import { CONCERT_SHOWS_RANGE, DEFAULT_PLAYLIST_SETTINGS, ENERGY_CURVES, PARTY_MINUTES_RANGE, PER_ARTIST_RANGE, SEARCH_BUDGET_RANGE, THRESHOLD_PCT_RANGE, partyLength, type ConcertThresholds, type PlaylistSettings, type Settings } from "../../../lib/api";
  import { useSettings } from "../model.svelte";
  import SettingsGroup from "../SettingsGroup.svelte";
  import SegmentedRow from "../SegmentedRow.svelte";
  import SliderRow from "../SliderRow.svelte";
  import SwitchRow from "../SwitchRow.svelte";

  let { settings }: { settings: Settings } = $props();
  const m = useSettings();

  const pl = $derived<PlaylistSettings>({ ...DEFAULT_PLAYLIST_SETTINGS, ...settings.playlists });
  const th = $derived<ConcertThresholds>({ ...DEFAULT_PLAYLIST_SETTINGS.concert_thresholds, ...settings.playlists?.concert_thresholds });
  const save = (patch: Partial<PlaylistSettings>) => m.save({ playlists: { ...pl, ...patch } });
  const saveThreshold = (patch: Partial<ConcertThresholds>) => save({ concert_thresholds: { ...th, ...patch } });
  const ofShows = (n: number) => `${n}% of shows`;
</script>

<SettingsGroup title="Get ready for a concert" description="Thresholds keep their order: Always ≥ Usually ≥ Sometimes.">
  <SliderRow
    label="Shows to consider"
    hint="How many recent shows the AI looks up (3–15)."
    min={CONCERT_SHOWS_RANGE.min}
    max={CONCERT_SHOWS_RANGE.max}
    step={1}
    value={pl.concert_shows}
    format={(n) => `${n} recent shows`}
    onchange={(n) => save({ concert_shows: n })}
  />
  <SliderRow label="“Always” from" min={THRESHOLD_PCT_RANGE.min} max={THRESHOLD_PCT_RANGE.max} step={THRESHOLD_PCT_RANGE.step} value={th.always_pct} format={ofShows} onchange={(n) => saveThreshold({ always_pct: n })} />
  <SliderRow label="“Usually” from" min={THRESHOLD_PCT_RANGE.min} max={THRESHOLD_PCT_RANGE.max} step={THRESHOLD_PCT_RANGE.step} value={th.usually_pct} format={ofShows} onchange={(n) => saveThreshold({ usually_pct: n })} />
  <SliderRow
    label="“Sometimes” from"
    hint="Songs played in fewer shows than this are left out."
    min={THRESHOLD_PCT_RANGE.min}
    max={THRESHOLD_PCT_RANGE.max}
    step={THRESHOLD_PCT_RANGE.step}
    value={th.sometimes_pct}
    format={ofShows}
    onchange={(n) => saveThreshold({ sometimes_pct: n })}
  />
  <SliderRow
    label="Encore when it closed at least"
    hint="The share of its plays that came in the encore."
    min={THRESHOLD_PCT_RANGE.min}
    max={THRESHOLD_PCT_RANGE.max}
    step={THRESHOLD_PCT_RANGE.step}
    value={th.encore_pct}
    format={(n) => `${n}% of its plays`}
    onchange={(n) => saveThreshold({ encore_pct: n })}
  />
  <SliderRow
    label="Web searches per playlist"
    hint="More can find more shows; each search costs a little on your AI account."
    min={SEARCH_BUDGET_RANGE.min}
    max={SEARCH_BUDGET_RANGE.max}
    step={1}
    value={pl.search_budget}
    onchange={(n) => save({ search_budget: n })}
  />
  <SwitchRow label="Skip recorded intros" checked={pl.skip_tape} onchange={(on) => save({ skip_tape: on })} />
  <SwitchRow label="Find covers by the original artist" checked={pl.covers_original_artist} onchange={(on) => save({ covers_original_artist: on })} />
  <SwitchRow
    label="Show the pages the AI read"
    hint="Google's search suggestions are always shown with a Gemini result, as Google requires."
    checked={pl.show_sources}
    onchange={(on) => save({ show_sources: on })}
  />
</SettingsGroup>

<SettingsGroup title="Party playlists" description="Default answers on the party screen.">
  <SliderRow
    label="Length"
    hint="Default answer on the party screen"
    min={PARTY_MINUTES_RANGE.min}
    max={PARTY_MINUTES_RANGE.max}
    step={PARTY_MINUTES_RANGE.step}
    value={pl.party_minutes}
    format={(n) => partyLength(n)}
    onchange={(n) => save({ party_minutes: n })}
  />
  <SegmentedRow label="Energy" options={ENERGY_CURVES} value={pl.party_curve} onchange={(id) => save({ party_curve: id })} />
  <SwitchRow label="Explicit lyrics allowed" checked={pl.party_allow_explicit} onchange={(on) => save({ party_allow_explicit: on })} />
  <SliderRow
    label="Songs per artist, at most"
    hint="So no one artist takes over (party and chat playlists)"
    min={PER_ARTIST_RANGE.min}
    max={PER_ARTIST_RANGE.max}
    step={1}
    value={pl.party_max_per_artist}
    onchange={(n) => save({ party_max_per_artist: n })}
  />
</SettingsGroup>

<SettingsGroup title="AI playlists" description="Defaults of “Make a playlist with AI”. Say otherwise in the chat and the AI changes them.">
  <SliderRow
    label="Length"
    hint="Default length of a chat playlist"
    min={PARTY_MINUTES_RANGE.min}
    max={PARTY_MINUTES_RANGE.max}
    step={PARTY_MINUTES_RANGE.step}
    value={pl.chat_minutes}
    format={(n) => partyLength(n)}
    onchange={(n) => save({ chat_minutes: n })}
  />
  <SegmentedRow label="Energy" options={ENERGY_CURVES} value={pl.chat_curve} onchange={(id) => save({ chat_curve: id })} />
  <SwitchRow label="Explicit lyrics allowed" checked={pl.chat_allow_explicit} onchange={(on) => save({ chat_allow_explicit: on })} />
  <SwitchRow label="“Playlist like this” in song menus" hint="Make an AI playlist from a song you found" checked={pl.chat_from_song} onchange={(on) => save({ chat_from_song: on })} />
</SettingsGroup>

<SettingsGroup title="Playlists">
  <SwitchRow label="“Made by HomeFi” on Playlists" checked={pl.home_section} onchange={(on) => save({ home_section: on })} />
  <SwitchRow label="Play my own copies first" hint="Songs in your library play from the file instead of streaming." checked={pl.prefer_library} onchange={(on) => save({ prefer_library: on })} />
</SettingsGroup>

<SettingsGroup title="Save to TIDAL" description="A playlist saved to TIDAL shows up on every device signed in to your TIDAL account. Songs TIDAL doesn't have are left out.">
  <SwitchRow
    label="Save to TIDAL automatically"
    hint="Playlists you save in HomeFi are added to your TIDAL account, and kept the same when you change them."
    checked={pl.tidal_auto_save}
    onchange={(on) => save({ tidal_auto_save: on })}
  />
  <SwitchRow
    label="Delete the TIDAL copy too"
    hint="Deleting a playlist in HomeFi also deletes the copy HomeFi saved in your TIDAL account."
    checked={pl.tidal_delete_with}
    onchange={(on) => save({ tidal_delete_with: on })}
  />
</SettingsGroup>
