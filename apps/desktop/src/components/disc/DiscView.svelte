<script lang="ts">
  // The Disc screen: the drive and the disc in it, what to call the files, which songs, the
  // quality, and the Rip button; then the progress of the rip. A disc that isn't an audio CD
  // (none, data, copy-protected) gets a plain explanation instead.
  import { onMount } from "svelte";
  import { formatTime } from "../../lib/api";
  import { disc, formatLabel } from "../../lib/disc.svelte";
  import { app } from "../../lib/state.svelte";
  import Segmented from "../settings/Segmented.svelte";
  import SettingsNote from "../settings/SettingsNote.svelte";
  import Icon from "../Icon.svelte";
  import DiscIdentify from "./DiscIdentify.svelte";
  import DiscNotAudio from "./DiscNotAudio.svelte";
  import DiscProgress from "./DiscProgress.svelte";
  import DiscQuality from "./DiscQuality.svelte";
  import DiscTracks from "./DiscTracks.svelte";

  const view = $derived(disc.current);
  const drive = $derived(disc.drive);
  const settings = $derived(disc.settings);
  const showRip = $derived(Object.keys(disc.rows).length > 0);
  const ripLabel = $derived(`Rip ${disc.selection.size} ${disc.selection.size === 1 ? "song" : "songs"}`);
  const target = $derived(settings.ripDir ?? app.settings?.download_dir ?? "");

  onMount(() => {
    disc.enter();
  });

  // dev (mock): ?ripping starts the rip by itself, for screenshots
  let autoStarted = false;
  $effect(() => {
    if (import.meta.env.DEV && new URLSearchParams(location.search).has("ripping") && view && !autoStarted && !disc.ripping) {
      autoStarted = true;
      disc.rip();
    }
  });
</script>

<div class="disc">
  <header class="head">
    <div>
      <h1>Disc</h1>
      <div class="muted">{drive ? drive.name : "No drive"}</div>
    </div>
    <div class="actions">
      {#if disc.drives.length > 1}
        <Segmented options={disc.drives.map((d) => ({ id: d.id, label: d.name }))} value={drive?.id ?? ""} onchange={(id) => disc.show(id)} label="Drive" />
      {/if}
      <button type="button" class="icon-btn" title="Look at the drive again" aria-label="Look at the drive again" onclick={() => disc.enter()} disabled={disc.ripping}><Icon name="refresh" size={18} /></button>
      {#if view && !disc.ripping}<button type="button" class="btn tonal" onclick={() => disc.eject()}><Icon name="eject" size={16} /> Eject</button>{/if}
    </div>
  </header>

  {#if disc.error}
    <SettingsNote kind="error">{disc.error}</SettingsNote>
  {/if}

  {#if !view}
    {#if disc.loading || (drive?.media === "audio" && !disc.error)}
      <div class="reading" aria-busy="true"><span class="spinner" aria-hidden="true"></span> Reading the disc…</div>
    {:else}
      <DiscNotAudio {drive} />
    {/if}
  {:else}
    <section class="facts" aria-label="The disc">
      <div class="art" aria-hidden="true"><Icon name="cd" size={30} /></div>
      <div class="grow">
        <div class="title">Audio CD · {view.toc.tracks.length} tracks{drive?.lengthMs ? ` · ${formatTime(drive.lengthMs)}` : ""}</div>
        <div class="muted">16-bit / 44.1 kHz stereo{view.mcn ? ` · barcode ${view.mcn}` : ""}</div>
        <details>
          <summary>Details</summary>
          <dl>
            <dt>MusicBrainz disc id</dt><dd>{view.ids.musicbrainz}</dd>
            <dt>FreeDB id</dt><dd>{view.ids.freedb}</dd>
            <dt>CUETools id</dt><dd>{view.ids.ctdb}</dd>
          </dl>
        </details>
      </div>
    </section>

    {#if showRip}
      <DiscProgress />
    {:else}
      <DiscIdentify />
      <DiscTracks />
      <div class="quality">
        <DiscQuality {settings} driveId={view.drive.id} save={(patch) => disc.save(patch)} />
        <button type="button" class="more link" onclick={() => (app.view = { name: "settings", page: "discs" })}>More reading options in Settings</button>
      </div>
      <div class="ripbar">
        <div class="what">
          <b>{formatLabel(settings.format)} · {settings.mode === "secure" ? "Secure" : "Fast"} reading</b>
          <span class="muted ellipsis" title={target}>to {target}</span>
        </div>
        <button type="button" class="btn primary go" onclick={() => disc.rip()} disabled={!disc.selection.size}><Icon name="download" size={16} /> {ripLabel}</button>
      </div>
    {/if}
  {/if}
</div>

<style>
  .disc {
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 880px;
    container-type: inline-size;
    padding-bottom: 8px;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    gap: 12px;
    flex-wrap: wrap;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .reading {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 40px 8px;
    color: var(--muted);
  }
  .spinner {
    width: 20px;
    height: 20px;
    border: 3px solid var(--surface-3);
    border-top-color: var(--primary);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .facts {
    display: flex;
    gap: 16px;
    align-items: flex-start;
    padding: 16px 20px;
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    background: var(--surface);
  }
  .art {
    display: grid;
    place-items: center;
    flex: none;
    width: 56px;
    height: 56px;
    border-radius: 50%;
    background: var(--gradient);
    color: #fff;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .title {
    font-weight: 600;
    font-size: 16px;
  }
  details {
    margin-top: 6px;
    font-size: 13px;
  }
  summary {
    cursor: pointer;
    color: var(--muted);
    min-height: 24px;
  }
  dl {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: 4px 14px;
    margin: 6px 0 0;
  }
  dt {
    color: var(--muted);
  }
  dd {
    margin: 0;
    font: 12px var(--font-mono);
    overflow-wrap: anywhere;
  }
  .quality {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .more {
    align-self: flex-start;
    color: var(--primary);
    font-weight: 600;
    min-height: 32px;
  }
  .ripbar {
    position: sticky;
    bottom: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 12px 16px;
    border: 1px solid var(--glass-border);
    border-radius: var(--r-md);
    background: var(--glass);
    backdrop-filter: blur(24px) saturate(1.3);
    box-shadow: var(--shadow);
  }
  .what {
    display: flex;
    flex-direction: column;
    min-width: 0;
    font-size: 13.5px;
  }
  .go {
    flex: none;
    height: 44px;
    padding: 0 22px;
  }
</style>
