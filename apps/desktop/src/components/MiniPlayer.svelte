<script lang="ts">
  // Always-on-top floating player (its own window, index.html?mini). It wears
  // the same skin and theme as the main window, and follows them when they
  // change there (the "settings" event reaches every window).
  import { api, coverUrl, formatTime } from "../lib/api";
  import { skinOf } from "../lib/layouts";
  import { applyPalette, BRAND, paletteOf } from "../lib/palette";
  import { share } from "../lib/share.svelte";
  import { shownSource as src } from "../lib/source.svelte";
  import { app, applyTheme } from "../lib/state.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";

  $effect(() => {
    // only what this window shows, then it appears (see `mini_ready`)
    app
      .initMini()
      .catch((e) => console.warn("[homefi] mini player", e))
      .finally(() => {
        requestAnimationFrame(() => api.miniReady().catch(() => {}));
        share.init();
      });
  });

  const skin = $derived(app.settings ? skinOf(app.settings.room) : "classic");
  $effect(() => {
    if (app.settings) applyTheme(app.settings.theme, skin);
  });

  // someone else's room while this computer is a guest there
  const p = $derived(src.player);
  const track = $derived(src.current);
  const playing = $derived(p?.status === "playing");
  const duration = $derived(p?.duration_ms ?? track?.durationMs ?? 0);
  const path = $derived(track?.path ?? null);
  const fraction = $derived(duration ? Math.min(1, (p?.position_ms ?? 0) / duration) : 0);
  // the record turns in Vinyl; the era skins have square or small covers
  const radius = $derived(skin === "vinyl" ? 999 : skin === "retro_amp" ? 0 : skin === "luna" || skin === "aluminum" ? 4 : 10);
  const coverSize = $derived(skin === "retro_amp" ? 76 : 88);

  $effect(() => {
    const current = path;
    if (!current || app.settings?.room.album_colors === false) return applyPalette(BRAND);
    paletteOf(coverUrl(current)).then((pal) => path === current && applyPalette(pal));
  });

  function seek(e: MouseEvent) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    if (duration && src.can.seek) src.seek(((e.clientX - r.left) / r.width) * duration);
  }

  function onKey(e: KeyboardEvent) {
    if (e.code === "Space") {
      e.preventDefault();
      if (src.can.toggle) src.toggle();
    } else if (e.code === "KeyM") {
      app.toggleMute();
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="mini skin-{skin}" class:playing data-tauri-drag-region>
  {#if path}<div class="bg" style:background-image="url('{coverUrl(path)}')"></div>{/if}
  <div class="cover" data-tauri-drag-region>
    <Cover {path} size={coverSize} {radius} />
    {#if skin === "vinyl"}<span class="hole"></span>{/if}
  </div>
  <div class="body" data-tauri-drag-region>
    <div class="top" data-tauri-drag-region>
      <div class="meta" data-tauri-drag-region>
        <div class="title ellipsis" data-tauri-drag-region>{track?.title ?? "Nothing playing"}</div>
        <div class="artist ellipsis" data-tauri-drag-region>{track ? `${track.artist} · ${src.remote ? src.label : track.quality}` : "HomeFi"}</div>
      </div>
      <button class="icon-btn small" title="Back to HomeFi" aria-label="Back to the full window" onclick={() => api.miniPlayer(false)}>
        <Icon name="expand" size={14} />
      </button>
    </div>
    <div class="controls">
      <button class="icon-btn key" aria-label="Previous" onclick={() => src.previous()} disabled={!src.can.previous}><Icon name="prev" size={16} /></button>
      <button class="play" aria-label={playing ? "Pause" : "Play"} onclick={() => src.toggle()} disabled={!src.can.toggle}>
        <Icon name={playing ? "pause" : "play"} size={18} />
      </button>
      <button class="icon-btn key" aria-label="Next" onclick={() => src.next()} disabled={!src.can.next}><Icon name="next" size={16} /></button>
      <button
        class="icon-btn key mute"
        class:on={app.muted}
        aria-label={app.muted ? "Unmute" : "Mute"}
        aria-pressed={app.muted}
        title={app.muted ? "Unmute (M)" : "Mute (M)"}
        onclick={() => app.toggleMute()}
      >
        <Icon name={app.muted ? "mute" : "volume"} size={15} />
      </button>
      <span class="time">{formatTime(p?.position_ms ?? 0)} / {formatTime(duration)}</span>
    </div>
    <button class="bar" aria-label="Seek" onclick={seek}>
      <span style:width="{fraction * 100}%"></span>
    </button>
  </div>
</div>

<style>
  :global(body) {
    background: var(--room-deep);
    overflow: hidden;
  }
  .mini {
    /* each skin repaints these */
    --m-bg: radial-gradient(circle at 15% 50%, color-mix(in srgb, var(--room-glow) 55%, transparent), transparent 70%), var(--room-deep);
    --m-text: #fff;
    --m-muted: rgb(255 255 255 / 0.7);
    --m-key: #fff;
    --m-play-bg: var(--room-accent);
    --m-play-fg: #06161d;
    --m-groove: rgb(255 255 255 / 0.18);
    --m-fill: var(--room-accent);
    position: relative;
    height: 100vh;
    display: flex;
    gap: 14px;
    align-items: center;
    padding: 0 14px;
    overflow: hidden;
    background: var(--m-bg);
    color: var(--m-text);
    user-select: none;
    cursor: default;
  }
  .bg {
    position: absolute;
    inset: -40px;
    background-size: cover;
    background-position: center;
    filter: blur(40px) saturate(1.4);
    opacity: 0.35;
    pointer-events: none;
  }
  .cover,
  .body {
    position: relative;
  }
  .cover {
    flex: none;
    filter: drop-shadow(0 8px 18px rgb(0 0 0 / 0.45));
  }
  .body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .top {
    display: flex;
    gap: 6px;
    align-items: flex-start;
  }
  .meta {
    flex: 1;
    min-width: 0;
  }
  .title {
    font-weight: 700;
    font-size: 14px;
    color: var(--m-text);
  }
  .artist {
    font-size: 11.5px;
    color: var(--m-muted);
  }
  .controls {
    display: flex;
    align-items: center;
    gap: 4px;
    color: var(--m-key);
  }
  /* silenced: the button says so at a glance */
  .mute.on {
    color: #f87171;
  }
  .key {
    color: var(--m-key);
  }
  .play {
    width: 34px;
    height: 34px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--m-play-bg);
    color: var(--m-play-fg);
    box-shadow: 0 0 16px color-mix(in srgb, var(--room-accent) 60%, transparent);
  }
  .time {
    margin-left: auto;
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    color: var(--m-muted);
  }
  .small {
    width: 26px;
    height: 26px;
    color: var(--m-muted);
  }
  .bar {
    height: 4px;
    border-radius: 2px;
    background: var(--m-groove);
    overflow: hidden;
    padding: 0;
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--m-fill);
    transition: width 0.25s linear;
  }

  /* classic in a light theme: the album's paper instead of its night */
  :global(:root[data-mode="light"]) .skin-classic {
    --m-bg: radial-gradient(circle at 15% 50%, color-mix(in srgb, var(--room-glow) 22%, transparent), transparent 70%), var(--room-paper);
    --m-text: #10202a;
    --m-muted: rgb(16 32 42 / 0.65);
    --m-key: #10202a;
    --m-groove: rgb(16 32 42 / 0.14);
    --m-play-fg: #fff;
    --m-play-bg: var(--room-ink);
    --m-fill: var(--room-ink);
  }
  :global(:root[data-mode="light"]) .skin-classic .bg {
    opacity: 0.15;
  }

  /* Immersive: the album is the window */
  .skin-immersive .bg {
    inset: -20px;
    filter: blur(14px) saturate(1.3) brightness(0.55);
    opacity: 1;
  }

  /* Vinyl: walnut, and the cover turns like a record */
  .skin-vinyl {
    --m-bg: linear-gradient(180deg, #3a2a1d, #241910);
    --m-text: #f4ecdf;
    --m-muted: #d2bea3;
    --m-key: #f4ecdf;
    --m-play-bg: #e09a58;
    --m-play-fg: #2a1d14;
    --m-groove: rgb(244 236 223 / 0.16);
    --m-fill: #e09a58;
    font-family: var(--font-display);
  }
  .skin-vinyl .bg {
    display: none;
  }
  .skin-vinyl .cover {
    padding: 6px;
    border-radius: 50%;
    background: repeating-radial-gradient(circle, #111 0 2px, #1c1c1c 2px 4px);
  }
  .skin-vinyl.playing .cover {
    animation: turn 4s linear infinite;
  }
  .skin-vinyl .hole {
    position: absolute;
    left: 50%;
    top: 50%;
    width: 8px;
    height: 8px;
    margin: -4px 0 0 -4px;
    border-radius: 50%;
    background: #241910;
  }
  @keyframes turn {
    to {
      transform: rotate(360deg);
    }
  }

  /* Studio: a dark rack unit with a green signal */
  .skin-studio {
    --m-bg: linear-gradient(180deg, #23272c, #16181b);
    --m-text: #e4e7ea;
    --m-muted: #9aa2ab;
    --m-key: #e4e7ea;
    --m-play-bg: #3ddc84;
    --m-play-fg: #06180d;
    --m-groove: #2c3036;
    --m-fill: #3ddc84;
    border-top: 1px solid #2f343a;
  }
  .skin-studio .bg {
    display: none;
  }
  .skin-studio .time {
    font-family: var(--font-mono);
    color: #ffb02e;
  }

  /* Retro Amp: the late-90s mini bar, bevelled, with a green LCD */
  .skin-retro_amp {
    --m-bg: linear-gradient(180deg, #34344a, #1e1e2c);
    --m-text: #00e000;
    --m-muted: #00a000;
    --m-key: #22222e;
    --m-groove: #000;
    --m-fill: #00e000;
    padding: 0 10px;
    gap: 10px;
    font-family: var(--font-mono);
    box-shadow: inset 1px 1px 0 #5c5c7a, inset -1px -1px 0 #050508;
  }
  .skin-retro_amp .bg {
    display: none;
  }
  .skin-retro_amp .cover {
    filter: none;
    box-shadow: inset 1px 1px 0 #050508, inset -1px -1px 0 #5c5c7a;
    padding: 2px;
    background: #000;
  }
  .skin-retro_amp .meta {
    padding: 3px 6px;
    background: #000;
    box-shadow: inset 1px 1px 0 #050508, inset -1px -1px 0 #5c5c7a;
  }
  .skin-retro_amp .title {
    font-size: 12.5px;
    text-transform: uppercase;
  }
  .skin-retro_amp .key,
  .skin-retro_amp .play {
    width: 28px;
    height: 22px;
    border-radius: 0;
    color: #22222e;
    background: linear-gradient(180deg, #c8c8d6, #9a9aac);
    box-shadow: inset 1px 1px 0 #fff, inset -1px -1px 0 #4a4a5a;
  }
  .skin-retro_amp .play {
    width: 34px;
    background: linear-gradient(180deg, #e8d080, #b0902a);
  }
  .skin-retro_amp .time {
    color: #00e000;
  }
  .skin-retro_amp .small {
    border-radius: 0;
    color: #a8a8c0;
  }
  .skin-retro_amp .bar {
    height: 8px;
    border-radius: 0;
    box-shadow: inset 1px 1px 0 #050508, inset -1px -1px 0 #5c5c7a;
  }

  /* Luna: blue glass with the green orb */
  .skin-luna {
    --m-bg: linear-gradient(180deg, #3a93ff 0%, #0a6cf5 10%, #0058ee 45%, #0047c4 100%);
    --m-text: #fff;
    --m-muted: rgb(255 255 255 / 0.85);
    --m-key: #fff;
    --m-groove: rgb(0 0 40 / 0.45);
    --m-fill: linear-gradient(180deg, #a6f08c, #3c9a3c);
    font-family: Tahoma, "Segoe UI", sans-serif;
  }
  .skin-luna .bg {
    display: none;
  }
  .skin-luna .cover {
    padding: 3px;
    border-radius: 4px;
    background: #fff;
  }
  .skin-luna .play {
    background: radial-gradient(circle at 35% 30%, #c8f7b4, #4fbf3f 55%, #2a7d22);
    color: #fff;
    border: 2px solid #fff;
    box-shadow: 0 1px 4px rgb(0 0 0 / 0.4);
  }
  .skin-luna .key {
    border-radius: 50%;
    background: radial-gradient(circle at 35% 30%, #c9dcff, #3a7be8 60%, #1b4fb7);
    border: 1px solid rgb(255 255 255 / 0.7);
    width: 26px;
    height: 26px;
  }
  .skin-luna .bar {
    height: 6px;
    border-radius: 3px;
  }

  /* Aluminum: brushed metal around a pale LCD */
  .skin-aluminum {
    --m-bg: repeating-linear-gradient(90deg, rgb(255 255 255 / 0.07) 0 1px, rgb(0 0 0 / 0.035) 1px 2px), linear-gradient(180deg, #e6e6e6, #b3b3b3);
    --m-text: #1e1e1e;
    --m-muted: #4a4a4a;
    --m-key: #2e2e2e;
    --m-groove: #9da190;
    --m-fill: #3b3d33;
    font-family: "Lucida Grande", "Helvetica Neue", sans-serif;
    border: 1px solid #8d8d8d;
  }
  .skin-aluminum .bg {
    display: none;
  }
  .skin-aluminum .meta {
    padding: 3px 8px;
    border-radius: 5px;
    text-align: center;
    background: linear-gradient(180deg, #f2f4e6, #d8dcc4);
    border: 1px solid #8c8c8c;
    box-shadow: inset 0 1px 3px rgb(0 0 0 / 0.18);
  }
  .skin-aluminum .key,
  .skin-aluminum .play {
    border-radius: 50%;
    color: #2e2e2e;
    background: linear-gradient(180deg, #fdfdfd, #c9c9c9 55%, #a0a0a0);
    border: 1px solid #6e6e6e;
    box-shadow: inset 0 1px 0 #fff, 0 1px 2px rgb(0 0 0 / 0.25);
  }
  .skin-aluminum .key {
    width: 28px;
    height: 28px;
  }
  .skin-aluminum .bar {
    height: 5px;
  }

  /* Web 2000: charcoal, glossy orange play, lime progress */
  .skin-web2000 {
    --m-bg: linear-gradient(180deg, #4a4a4e, #333336);
    --m-text: #f2f2f2;
    --m-muted: #c4c4c4;
    --m-key: #3a3a3a;
    --m-groove: #222;
    --m-fill: linear-gradient(180deg, #d4f09c, #8dc63f);
    font-family: "Helvetica Neue", Arial, sans-serif;
  }
  .skin-web2000 .bg {
    display: none;
  }
  .skin-web2000 .cover {
    padding: 3px;
    border-radius: 8px;
    background: #f4f4f4;
  }
  .skin-web2000 .play {
    background: linear-gradient(180deg, #ffc07a 0%, #ff9124 48%, #ff7a00 52%, #ff9a3a 100%);
    color: #fff;
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.5);
  }
  .skin-web2000 .key {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    background: linear-gradient(180deg, #ffffff 0%, #f3f3f3 48%, #e4e4e4 52%, #f1f1f1 100%);
  }
  .skin-web2000 .bar {
    height: 6px;
    border-radius: 3px;
  }
</style>
