<script lang="ts">
  // A Tidal album (or playlist / artist) in a grid. Albums Tidal lists several
  // times (explicit / clean, stereo / Atmos, remasters, deluxe) come as one card:
  // the click opens the default release, the "N versions" chip (or a right-click)
  // lists them all.
  import { item } from "../lib/motion";
  import type { Hit } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";

  let { hit, contained = false }: { hit: Hit; /** Long grids: off-screen cards skip layout and paint. */ contained?: boolean } = $props();
  const versions = $derived(hit.versions ?? []);

  function open() {
    app.view = { name: "tidal-item", kind: hit.kind as "album", id: hit.id, back: app.view };
  }
  const pick = (e: MouseEvent) => versions.length > 1 && app.openVersions(e, hit.title, versions);
</script>

<div class="card" class:contained in:item|global oncontextmenu={pick} role="group" aria-label={hit.title}>
  <button class="main" onclick={open} title={hit.title}>
    <Cover url={hit.cover_url} size="100%" radius={hit.kind === "artist" ? 999 : 12} icon={hit.kind === "artist" ? "user" : hit.kind === "album" ? "disc" : "queue"} />
    <div class="t">
      <span class="ellipsis">{hit.title}</span>
      {#if hit.explicit}<span class="badge e" title="Explicit">E</span>{/if}
    </div>
    <div class="s ellipsis">{hit.subtitle}</div>
  </button>
  {#if versions.length > 1}
    <!-- a square over the cover, so the chip sits on its lower left corner -->
    <div class="over">
      <button class="chip" title="Explicit, clean, Dolby Atmos, remasters… pick one" onclick={pick}>
        <Icon name="layers" size={13} />
        {versions.length} versions
      </button>
    </div>
  {/if}
</div>

<style>
  .card {
    position: relative;
    min-width: 0;
    padding: 8px;
    margin: -8px;
    border-radius: min(14px, var(--r-max));
  }
  .card.contained {
    content-visibility: auto;
    contain-intrinsic-size: auto 168px auto 230px;
  }
  .card:hover {
    background: var(--glass-strong);
  }
  .main {
    display: flex;
    flex-direction: column;
    gap: 3px;
    width: 100%;
    text-align: left;
    min-width: 0;
  }
  .t {
    display: flex;
    align-items: center;
    gap: 5px;
    margin-top: 6px;
    font-weight: 600;
    min-width: 0;
  }
  .t .ellipsis {
    min-width: 0;
  }
  .s {
    font-size: 12.5px;
    color: var(--muted);
  }
  .badge.e {
    flex: none;
    padding: 0 5px;
    font-size: 10px;
    line-height: 16px;
    background: var(--surface-3);
    color: var(--muted);
  }
  .over {
    position: absolute;
    left: 8px;
    right: 8px;
    top: 8px;
    aspect-ratio: 1;
    pointer-events: none;
  }
  .chip {
    pointer-events: auto;
    position: absolute;
    left: 8px;
    bottom: 8px;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 3px 9px 3px 7px;
    border-radius: min(999px, var(--r-max));
    font-size: 11.5px;
    font-weight: 600;
    color: #fff;
    background: rgb(0 0 0 / 0.62);
    backdrop-filter: blur(6px);
    border: 1px solid rgb(255 255 255 / 0.18);
  }
  .chip:hover {
    background: rgb(0 0 0 / 0.8);
  }
</style>
