<script lang="ts">
  // Every release of an album: what tells each apart (explicit / clean, Atmos,
  // Hi-Res, remaster, deluxe...) and Play / Open / Download for each.
  import { pop } from "../lib/motion";
  import { errorText, type VersionHit } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";

  const picker = $derived(app.versionPicker!);
  const WIDTH = 420;
  const left = $derived(Math.max(12, Math.min(picker.x, window.innerWidth - WIDTH - 12)));
  // about as tall as its rows (it scrolls past 560 px): opens upwards when there is no room below
  const height = $derived(Math.min(560, window.innerHeight - 24, 92 + picker.versions.length * 84));
  const top = $derived(Math.max(12, Math.min(picker.y, window.innerHeight - height - 12)));
  const close = () => (app.versionPicker = null);
  // a 52 px row needs Tidal's 160 px image
  const thumb = (url: string | null) => url?.replace(/\/320x320\.jpg$/, "/160x160.jpg") ?? null;

  function open(v: VersionHit) {
    close();
    app.view = { name: "tidal-item", kind: "album", id: v.id, back: app.view.name === "tidal-item" && app.view.kind === "album" ? app.view.back : app.view };
  }
  function play(v: VersionHit) {
    close();
    app.playAlbum(v.id, v.title);
  }
  async function download(v: VersionHit) {
    close();
    try {
      await app.download(v.title, [`album/${v.id}`]);
    } catch (e) {
      app.notify(errorText(e), true);
    }
  }
  // the explicit mark is the "E" badge; the other labels are chips
  const chips = (v: VersionHit) => v.labels.filter((_, i) => v.label_codes[i] !== "explicit");
  // what the chips don't say: size and year (the whole description is the row's tooltip)
  function meta(v: VersionHit) {
    const tracks = v.tracks ? `${v.tracks} ${v.tracks === 1 ? "track" : "tracks"}${v.extra_tracks ? ` (${v.extra_tracks} extra)` : ""}` : "";
    return [tracks, v.year ?? "", v.atmos ? "needs an Atmos device" : ""].filter(Boolean).join(" · ");
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

<button class="scrim" aria-label="Close" onclick={close} oncontextmenu={(e) => (e.preventDefault(), close())}></button>
<div class="picker" style:left="{left}px" style:top="{top}px" style:width="{WIDTH}px" role="dialog" aria-label="Versions of {picker.title}" transition:pop>
  <div class="head">
    <b class="ellipsis">{picker.title}</b>
    <span class="muted">{picker.versions.length} versions</span>
  </div>
  <div class="rows">
    {#each picker.versions as v (v.id)}
      {@const here = v.id === picker.current}
      <div class="row" class:here title={v.description}>
        <button class="cover" onclick={() => open(v)} title="Open {v.title}">
          <Cover url={thumb(v.cover_url)} size={52} radius={8} icon="disc" />
        </button>
        <div class="grow">
          <div class="title">
            <span class="ellipsis">{v.title}</span>
            {#if v.explicit}<span class="badge e" title="Explicit">E</span>{/if}
            {#if here}<span class="tag">Viewing</span>{:else if v.primary}<span class="tag">Default</span>{/if}
          </div>
          <div class="chips">
            {#each chips(v) as label, i (i)}
              <span class="badge" class:atmos={label === "Dolby Atmos"}>{label}</span>
            {/each}
          </div>
          <div class="desc muted">{meta(v)}</div>
        </div>
        <div class="acts">
          <button class="icon-btn quiet" title="Play {v.title}" aria-label="Play {v.title}" onclick={() => play(v)}><Icon name="play" size={16} /></button>
          <button class="icon-btn quiet" title="Open" aria-label="Open {v.title}" onclick={() => open(v)} disabled={here}><Icon name="disc" size={16} /></button>
          <button class="icon-btn quiet" title="Download this version" aria-label="Download {v.title}" onclick={() => download(v)}><Icon name="download" size={16} /></button>
        </div>
      </div>
    {/each}
  </div>
  <button class="foot muted" onclick={() => (close(), (app.view = { name: "settings", page: "tidal" }))}>
    <Icon name="settings" size={14} /> Which one shows first: Settings → Tidal
  </button>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
    cursor: default;
  }
  .picker {
    position: fixed;
    z-index: 41;
    display: flex;
    flex-direction: column;
    max-height: min(560px, calc(100vh - 24px));
    padding: 6px;
    border-radius: min(14px, var(--r-max));
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
  }
  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 10px;
    padding: 6px 8px 8px;
    font-size: 13px;
  }
  .head .muted {
    flex: none;
    font-size: 12px;
  }
  .rows {
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 6px 7px 8px;
    border-radius: min(10px, var(--r-max));
  }
  .row:hover {
    background: var(--surface-2);
  }
  .row.here {
    background: color-mix(in srgb, var(--room-accent) 14%, transparent);
  }
  .cover {
    flex: none;
  }
  .grow {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .title {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
    font-size: 13.5px;
    min-width: 0;
  }
  .title .ellipsis {
    min-width: 0;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .chips .badge {
    font-size: 10.5px;
    padding: 0 6px;
  }
  .badge.atmos {
    background: color-mix(in srgb, #8b5cf6 22%, transparent);
  }
  .badge.e {
    flex: none;
    padding: 0 5px;
    font-size: 10px;
    line-height: 16px;
    background: var(--surface-3);
    color: var(--muted);
  }
  .tag {
    flex: none;
    font: 600 10px var(--font-mono);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--accent-text);
  }
  .desc {
    font-size: 11.5px;
    line-height: 1.35;
  }
  .acts {
    flex: none;
    display: flex;
    margin-left: auto;
  }
  .foot {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 4px;
    padding: 8px;
    font-size: 12px;
    border-top: 1px solid var(--border);
    text-align: left;
  }
  .foot:hover {
    color: var(--text);
  }
</style>
