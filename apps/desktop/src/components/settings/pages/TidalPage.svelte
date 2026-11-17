<script lang="ts">
  import { api, errorText, type AlbumVersionPrefs, type Settings } from "../../../lib/api";
  import { app } from "../../../lib/state.svelte";
  import { useSettings } from "../model.svelte";
  import ActionRow from "../ActionRow.svelte";
  import SegmentedRow from "../SegmentedRow.svelte";
  import SettingsGroup from "../SettingsGroup.svelte";
  import SwitchRow from "../SwitchRow.svelte";
  import Spinner from "../../Spinner.svelte";

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

  // Saved Tidal answers (the API cache): its size, what it takes now, and emptying it.
  const DEFAULT_CACHE_MB = 200;
  const CACHE_SIZES = [
    { id: "0", label: "Off" },
    { id: "50", label: "50 MB" },
    { id: "100", label: "100 MB" },
    { id: "200", label: "200 MB" },
    { id: "500", label: "500 MB" },
    { id: "1024", label: "1 GB" },
  ] as const;
  type CacheSize = (typeof CACHE_SIZES)[number]["id"];
  const BYTES_PER_MB = 1_048_576;
  const cacheMb = $derived(settings.api_cache_mb ?? DEFAULT_CACHE_MB);
  let cacheBytes = $state<number | null>(null);
  let clearingCache = $state(false);

  function sizeText(bytes: number): string {
    if (bytes === 0) return "Nothing saved";
    const mb = bytes / BYTES_PER_MB;
    return mb < 1 ? "Less than 1 MB" : `${mb.toFixed(mb < 10 ? 1 : 0)} MB`;
  }

  async function readCacheUsage() {
    try {
      cacheBytes = await api.apiCacheUsage();
    } catch {
      cacheBytes = null;
    }
  }
  $effect(() => {
    void readCacheUsage();
  });

  async function saveCacheSize(id: CacheSize) {
    // the backend frees what no longer fits before answering
    await m.save({ api_cache_mb: Number(id) });
    await readCacheUsage();
  }

  async function clearCache() {
    if (clearingCache) return;
    clearingCache = true;
    try {
      await api.clearApiCache();
    } catch (e) {
      m.error = errorText(e);
    } finally {
      await readCacheUsage();
      clearingCache = false;
    }
  }

  let signingOut = $state(false);

  async function logout() {
    const ok = await app.confirm({
      title: "Log out of Tidal?",
      text: "Your library and downloaded files stay. You'll need to connect again to search, stream and download.",
      confirmLabel: "Log out",
      danger: true,
    });
    if (!ok || signingOut) return;
    signingOut = true;
    try {
      await api.logout();
      app.account = null;
    } finally {
      signingOut = false;
    }
  }
</script>

<SettingsGroup title="Account">
  {#if app.account}
    <ActionRow label="Connected" hint="Country: {app.account.countryCode}">
      <button type="button" class="btn tonal danger" disabled={signingOut} aria-busy={signingOut} onclick={logout}>{#if signingOut}<Spinner size={16} /> Logging out…{:else}Log out{/if}</button>
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

<SettingsGroup title="Saved Tidal answers">
  <SegmentedRow
    label="Space for saved answers"
    hint="HomeFi keeps what Tidal answers (lists, albums, artists, searches) for a while, so pages open faster and Tidal is asked less. The oldest ones are deleted by themselves to stay under this size. Off keeps none. Your downloads and library are never touched."
    options={CACHE_SIZES}
    value={String(cacheMb) as CacheSize}
    onchange={saveCacheSize}
  />
  <ActionRow label="Saved now" hint={cacheBytes === null ? "Checking…" : `${sizeText(cacheBytes)}. Clearing only makes the next pages ask Tidal again.`}>
    <button type="button" class="btn tonal" disabled={clearingCache || cacheBytes === 0} aria-busy={clearingCache} onclick={clearCache}>{#if clearingCache}<Spinner size={16} /> Clearing…{:else}Clear{/if}</button>
  </ActionRow>
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
