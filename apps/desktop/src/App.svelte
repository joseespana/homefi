<script lang="ts">
  import { coverUrl } from "./lib/api";
  import { applyPalette, BRAND, paletteOf } from "./lib/palette";
  import { app, applyTheme } from "./lib/state.svelte";
  import { visual } from "./lib/visual.svelte";
  import Deck from "./components/Deck.svelte";
  import Downloads from "./components/Downloads.svelte";
  import Library from "./components/Library.svelte";
  import Login from "./components/Login.svelte";
  import TermsDialog from "./components/TermsDialog.svelte";
  import QueuePanel from "./components/QueuePanel.svelte";
  import Rail from "./components/Rail.svelte";
  import Room from "./components/Room.svelte";
  import MascotHost from "./components/MascotHost.svelte";
  import Folder from "./components/Folder.svelte";
  import Playlists from "./components/Playlists.svelte";
  import QueueMenu from "./components/QueueMenu.svelte";
  import VersionPicker from "./components/VersionPicker.svelte";
  import Skeleton from "./components/Skeleton.svelte";
  import { busy } from "./lib/busy.svelte";
  import MyTidal from "./components/MyTidal.svelte";
  import Search from "./components/Search.svelte";
  import TidalItem from "./components/TidalItem.svelte";
  import Settings from "./components/Settings.svelte";
  import Equalizer from "./components/eq/Equalizer.svelte";
  import Assistant from "./components/eq/Assistant.svelte";
  import PlaylistPicker from "./components/PlaylistPicker.svelte";
  import NewPlaylist from "./components/NewPlaylist.svelte";
  import ConfirmDialog from "./components/ConfirmDialog.svelte";
  import SoundFor from "./components/sound/SoundFor.svelte";
  import { sound } from "./lib/sound.svelte";
  import Notifications from "./components/Notifications.svelte";
  import UpdateBanner from "./components/UpdateBanner.svelte";
  import { updates } from "./lib/update.svelte";
  import { fillsRoom, layoutOf, skinOf } from "./lib/layouts";
  import { share } from "./lib/share.svelte";
  import { shownSource } from "./lib/source.svelte";
  import { lyrics } from "./lib/lyrics.svelte";
  import GuestView from "./components/share/GuestView.svelte";
  import JoinDialog from "./components/share/JoinDialog.svelte";
  import NearbyBanner from "./components/share/NearbyBanner.svelte";
  import SharePanel from "./components/share/SharePanel.svelte";
  import { applyMotion, page } from "./lib/motion";

  // dev: ?queue opens the queue panel (screenshots)
  let queueOpen = $state(import.meta.env.DEV && new URLSearchParams(location.search).has("queue"));
  let ready = $state(false);

  $effect(() => {
    app.init().then(() => {
      share.init();
      updates.init();
      // dev: open a view from the URL, e.g. http://localhost:1420/#albums
      if (import.meta.env.DEV && location.hash.length > 1) app.view = { name: location.hash.slice(1) } as typeof app.view;
      ready = true;
    });
  });

  // the room takes the colors of the record that's playing
  // (someone else's room while this computer is a guest there)
  const coverPath = $derived(shownSource.current?.path ?? null);
  $effect(() => {
    const path = coverPath;
    if (!path || app.settings?.room.album_colors === false) return applyPalette(BRAND);
    paletteOf(coverUrl(path)).then((p) => coverPath === path && applyPalette(p));
  });

  // the retro layouts take the whole height: no page scroll, they size themselves
  const fill = $derived(
    (app.view.name === "room" || (app.view.name === "guest" && share.tab === "player")) && (fillsRoom(layoutOf(app.settings?.room.layout), !!shownSource.current) || (lyrics.open && !!shownSource.current)),
  );

  // "Open the lyrics on their own": when a song that has them starts
  const lyricsReq = $derived(lyrics.request(shownSource));
  let lastLyricsKey: string | null = null;
  $effect(() => {
    const req = lyricsReq;
    if (!req || req.key === lastLyricsKey) return;
    lastLyricsKey = req.key;
    if (!app.settings?.room.lyrics_auto_open || lyrics.open) return;
    lyrics.load(req).then((e) => {
      if (e.status === "ready" && lastLyricsKey === req.key && app.settings?.room.lyrics_auto_open) lyrics.setOpen(true);
    });
  });
  // a session found nearby: offered in the room, or on any screen
  const offerHere = $derived(
    !!app.settings?.share.discover && (app.settings.share.discover_everywhere || app.view.name === "room") && !share.panelOpen && !share.joinForm,
  );

  // player errors (unplayable file, device lost) become floating notes
  let lastError: string | null = null;
  $effect(() => {
    const e = app.player?.error ?? null;
    if (e && e !== lastError) app.note(e, "error", "player");
    lastError = e;
  });

  // visualizer / VU data only flows while playing
  $effect(() => {
    if (shownSource.player?.status === "playing") visual.start();
    else visual.stop();
  });

  // each track gets its own tempo / energy reading for the mascot
  $effect(() => {
    coverPath;
    visual.reset();
  });

  // Space = play/pause, ←/→ = seek 10 s, Ctrl/⌘ + ←/→ = previous / next
  function onKey(e: KeyboardEvent) {
    // a confirmation is open: keys belong to it (Space presses its focused button)
    if (app.confirmDialog) return;
    const target = e.target as HTMLElement;
    if (target.closest("input, textarea, select") && (target as HTMLInputElement).type !== "range") return;
    const src = shownSource;
    const p = src.player;
    const mod = e.metaKey || e.ctrlKey;
    if (e.code === "Space") {
      e.preventDefault();
      if (src.can.toggle) src.toggle();
    } else if (e.key === "ArrowRight" && mod && src.can.next) src.next();
    else if (e.key === "ArrowLeft" && mod && src.can.previous) src.previous();
    else if (e.key === "ArrowRight" && p && src.can.seek) src.seek(p.position_ms + 10_000);
    else if (e.key === "ArrowLeft" && p && src.can.seek) src.seek(p.position_ms - 10_000);
  }

  // light / dark follows the setting wherever it is changed from, right away;
  // the room's layout is the skin of the whole app (unless it only dresses the room)
  $effect(() => {
    if (app.settings) applyTheme(app.settings.theme, skinOf(app.settings.room));
  });
  // how much the app moves; the skin gives the motion its character
  $effect(() => {
    if (app.settings) applyMotion(app.settings.room.motion, skinOf(app.settings.room));
  });

  // each view arrives with its own entrance; tabs of one view (My Tidal) and the
  // listening room's own changes stay put, their contents move instead
  const pageKey = $derived.by(() => {
    if (!ready) return "loading";
    const v = app.view;
    switch (v.name) {
      case "album":
        return `album:${v.key}`;
      case "artist":
        return `artist:${v.name_}`;
      case "playlist":
      case "folder":
        return `${v.name}:${v.path}`;
      case "tidal-item":
        return `tidal-item:${v.kind}:${v.id}`;
      default:
        return v.name;
    }
  });
</script>

<svelte:window onkeydown={onKey} />

<div class="room-bg" aria-hidden="true">
  {#if coverPath}
    {#key coverPath}<img src={coverUrl(coverPath)} alt="" />{/key}
  {/if}
  {#if app.settings?.room.album_colors === false}{:else}<div class="tint"></div>{/if}
  <div class="grain"></div>
</div>

<!-- room for macOS's window buttons where a skin floats the rail (still drags the window) -->
<div class="titlebar" data-tauri-drag-region aria-hidden="true"></div>

<div class="shell" class:queue={queueOpen}>
  <Rail />
  <main class:fill>
    <div class="drag" data-tauri-drag-region>
      {#if busy.count > 0 || app.scanning}<div class="loadbar" role="progressbar" aria-label="Loading"></div>{/if}
    </div>
    {#key pageKey}
    <div class="content" in:page>
      {#if !ready}
        <Skeleton kind="grid" count={10} />
      {:else if app.view.name === "room"}
        <Room />
      {:else if app.view.name === "search"}
        <Search />
      {:else if app.view.name === "playlists" || app.view.name === "playlist"}
        <Playlists />
      {:else if app.view.name === "tidal"}
        <MyTidal />
      {:else if app.view.name === "tidal-item"}
        <TidalItem />
      {:else if app.view.name === "folder"}
        <Folder />
      {:else if app.view.name === "downloads"}
        <Downloads />
      {:else if app.view.name === "settings"}
        <Settings />
      {:else if app.view.name === "eq"}
        <Equalizer />
      {:else if app.view.name === "assistant"}
        <Assistant />
      {:else if app.view.name === "guest"}
        {#if share.inRoom}<GuestView />{:else}<Room />{/if}
      {:else}
        <Library />
      {/if}
    </div>
    {/key}
  </main>
  {#if queueOpen}<QueuePanel onclose={() => (queueOpen = false)} />{/if}
  <Deck bind:queueOpen />
</div>

{#if app.loginOpen}<Login />
{:else if ready && app.account && app.settings && !app.termsAccepted}<TermsDialog />{/if}
{#if app.playlistPicker}<PlaylistPicker />{/if}
{#if app.queueMenu}<QueueMenu />{/if}
{#if app.versionPicker}<VersionPicker />{/if}
{#if app.newPlaylist}<NewPlaylist />{/if}
{#if share.panelOpen}<SharePanel />{/if}
{#if share.joinForm}<JoinDialog />{/if}
{#if offerHere}<NearbyBanner />{/if}
{#if sound.forScope}<SoundFor />{/if}
{#if updates.banner}<UpdateBanner />{/if}
{#if app.confirmDialog}<ConfirmDialog />{/if}
<Notifications />

<!-- dragged out of the room: it floats over the room, or over everything -->
{#if app.settings?.room.mascot && app.settings.room.mascot_free && (app.settings.room.mascot_everywhere || app.view.name === "room")}
  <MascotHost floating singing={lyrics.singing} />
{/if}

<style>
  .room-bg {
    position: fixed;
    inset: 0;
    z-index: 0;
    overflow: hidden;
    background: var(--room-bg);
    transition: background 1.2s;
  }
  .room-bg img {
    position: absolute;
    inset: -10%;
    width: 120%;
    height: 120%;
    object-fit: cover;
    filter: blur(70px) saturate(1.5);
    opacity: 0.45;
    animation: fadein 1.2s ease;
  }
  @keyframes fadein {
    from {
      opacity: 0;
    }
  }
  .tint {
    position: absolute;
    inset: 0;
    background: radial-gradient(1200px 700px at 30% 30%, color-mix(in srgb, var(--room-glow) 55%, transparent), transparent 70%),
      linear-gradient(180deg, transparent 40%, var(--room-bg));
    transition: background 1.2s;
  }
  /* light theme: the cover only tints the paper, it must not darken it */
  @media (prefers-color-scheme: light) {
    :global(:root:not([data-theme="dark"])) .room-bg img {
      opacity: 0.22;
      filter: blur(80px) saturate(1.2) brightness(1.25);
    }
    :global(:root:not([data-theme="dark"])) .tint {
      background: radial-gradient(1200px 700px at 30% 30%, color-mix(in srgb, var(--room-glow) 14%, transparent), transparent 70%),
        linear-gradient(180deg, transparent 30%, var(--room-bg));
    }
  }
  :global(:root[data-theme="light"]) .room-bg img {
    opacity: 0.22;
    filter: blur(80px) saturate(1.2) brightness(1.25);
  }
  :global(:root[data-theme="light"]) .tint {
    background: radial-gradient(1200px 700px at 30% 30%, color-mix(in srgb, var(--room-glow) 14%, transparent), transparent 70%),
      linear-gradient(180deg, transparent 30%, var(--room-bg));
  }
  .grain {
    position: absolute;
    inset: 0;
    opacity: 0.06;
    background-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='160' height='160'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2' stitchTiles='stitch'/></filter><rect width='100%' height='100%' filter='url(%23n)'/></svg>");
  }
  .shell {
    position: relative;
    z-index: 1;
    display: grid;
    grid-template-columns: 76px 1fr;
    /* minmax(0, …): tall content (rail, room) scrolls instead of pushing the deck off screen */
    grid-template-rows: minmax(0, 1fr) auto;
    height: 100vh;
  }
  .shell.queue {
    grid-template-columns: 76px 1fr 320px;
  }
  /* narrow window: the queue slides over the content instead of squeezing it */
  @media (max-width: 1180px) {
    .shell.queue {
      grid-template-columns: 76px 1fr;
    }
    .shell.queue :global(.panel.queue-panel) {
      position: fixed;
      top: 0;
      right: 0;
      bottom: 100px;
      z-index: 6;
      box-shadow: -24px 0 48px rgb(0 0 0 / 0.45);
    }
  }
  main {
    position: relative;
    overflow-y: auto;
    min-width: 0;
  }
  .titlebar {
    display: none;
  }
  .drag {
    position: sticky;
    top: 0;
    height: 34px;
    z-index: 5;
  }
  /* indeterminate bar along the top while anything slow is loading */
  .loadbar {
    position: absolute;
    inset: 0 0 auto;
    height: 2px;
    overflow: hidden;
    background: color-mix(in srgb, var(--accent-text) 15%, transparent);
  }
  .loadbar::after {
    content: "";
    position: absolute;
    inset: 0 auto 0 0;
    width: 40%;
    background: linear-gradient(90deg, transparent, #1d4ed8, #0891b2, #10b981, transparent);
    animation: loadbar 1.2s ease-in-out infinite;
  }
  @keyframes loadbar {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(250%);
    }
  }
  .content {
    padding: 0 clamp(18px, 3.2vw, 40px) 40px;
    /* views lay out by the space they get (rail and queue taken out), not the window */
    container-type: inline-size;
  }
  main.fill {
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  main.fill .content {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding-bottom: 16px;
  }
</style>
