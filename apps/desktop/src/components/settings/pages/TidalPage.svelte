<script lang="ts">
  import { api, type AlbumVersionPrefs, type Settings } from "../../../lib/api";
  import { app } from "../../../lib/state.svelte";
  import { useSettings } from "../model.svelte";
  import ActionRow from "../ActionRow.svelte";
  import SegmentedRow from "../SegmentedRow.svelte";
  import SettingsGroup from "../SettingsGroup.svelte";
  import SwitchRow from "../SwitchRow.svelte";

  let { settings }: { settings: Settings } = $props();
  const m = useSettings();

  const VERSION_DEFAULTS: AlbumVersionPrefs = { group: true, explicit: "explicit", spatial: "stereo", edition: "original" };
  const versionPrefs = $derived<AlbumVersionPrefs>({ ...VERSION_DEFAULTS, ...settings.album_versions });
  function saveVersions(patch: Partial<AlbumVersionPrefs>) {
    // My Tidal's albums come grouped from Rust: load them again the new way
    app.tidalCache.delete("albums");
    m.save({ album_versions: { ...versionPrefs, ...patch } });
  }

  const EXPLICIT = [
    { id: "explicit", label: "Explicit" },
    { id: "clean", label: "Clean" },
  ] as const;
  const SPATIAL = [
    { id: "stereo", label: "Stereo" },
    { id: "atmos", label: "Dolby Atmos" },
  ] as const;
  const EDITION = [
    { id: "original", label: "Original" },
    { id: "remaster", label: "Latest remaster" },
  ] as const;

  async function logout() {
    const ok = await app.confirm({
      title: "Log out of Tidal?",
      text: "Your library and downloaded files stay. You'll need to connect again to search, stream and download.",
      confirmLabel: "Log out",
      danger: true,
    });
    if (!ok) return;
    await api.logout();
    app.account = null;
  }
</script>

<SettingsGroup title="Account">
  {#if app.account}
    <ActionRow label="Connected" hint="User {app.account.userId} · {app.account.countryCode}">
      <button type="button" class="btn tonal danger" onclick={logout}>Log out</button>
    </ActionRow>
  {:else}
    <ActionRow label="Not connected" hint="The player works without an account.">
      <button type="button" class="btn primary" onclick={() => (app.loginOpen = true)}>Connect Tidal</button>
    </ActionRow>
  {/if}
</SettingsGroup>

<SettingsGroup title="Lists">
  <SwitchRow
    label="Load lists as you scroll"
    hint="Playlists, albums and favorites show their first tracks right away and bring more as you scroll. Off, each list loads whole before it shows. Play, Shuffle and Download always use the whole list."
    checked={settings.lazy_lists ?? true}
    onchange={(on) => m.save({ lazy_lists: on })}
  />
</SettingsGroup>

<SettingsGroup title="Album versions">
  <SwitchRow
    label="One card per album"
    hint="Tidal lists some albums several times: explicit and clean, stereo and Dolby Atmos, 16 and 24-bit, remasters, deluxe box sets. On, they share one card with a “versions” chip to pick another; the album page shows “Other versions”. Off, every release shows as Tidal lists it."
    checked={versionPrefs.group}
    onchange={(on) => saveVersions({ group: on })}
  />
  {#if versionPrefs.group}
    <SegmentedRow label="Explicit or clean" hint="When both exist; explicit is the artist's original" options={EXPLICIT} value={versionPrefs.explicit} onchange={(id) => saveVersions({ explicit: id })} />
    <SegmentedRow label="Stereo or Dolby Atmos" hint="Atmos needs an Atmos device, and downloads skip Atmos tracks" options={SPATIAL} value={versionPrefs.spatial} onchange={(id) => saveVersions({ spatial: id })} />
    <SegmentedRow label="Original or remaster" hint="The highest quality wins among the rest; deluxe editions and box sets only when there is nothing else" options={EDITION} value={versionPrefs.edition} onchange={(id) => saveVersions({ edition: id })} />
  {/if}
</SettingsGroup>
