<script lang="ts">
  import { tick } from "svelte";
  import { share } from "../lib/share.svelte";
  import { app, type View } from "../lib/state.svelte";
  import Icon from "./Icon.svelte";

  const items: { view: View["name"]; label: string; icon: string }[] = [
    { view: "room", label: "Listening room", icon: "headphones" },
    { view: "albums", label: "Albums", icon: "disc" },
    { view: "songs", label: "Songs", icon: "music" },
    { view: "artists", label: "Artists", icon: "user" },
    { view: "playlists", label: "Playlists", icon: "queue" },
    { view: "tidal", label: "My Tidal", icon: "heart" },
    { view: "search", label: "Search Tidal", icon: "search" },
    { view: "downloads", label: "Downloads", icon: "download" },
  ];

  const active = (name: string) =>
    app.view.name === name ||
    (name === "albums" && app.view.name === "album") ||
    (name === "artists" && app.view.name === "artist") ||
    (name === "playlists" && (app.view.name === "playlist" || (app.view.name === "tidal-item" && app.view.back?.name === "playlists"))) ||
    (name === "tidal" && app.view.name === "tidal-item" && app.view.back?.name !== "playlists");

  // One tooltip outside the rail: inside it would be clipped by the rail's
  // scrolling and drawn under the content next to it.
  let tip = $state<{ text: string; y: number } | null>(null);
  function show(e: Event, text: string) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    tip = { text, y: r.top + r.height / 2 };
  }
  const hide = () => (tip = null);
  // a pill along the edge glides to the active place (glass skins; the retro ones keep their own selection)
  let rail = $state<HTMLElement>();
  let marker = $state<{ y: number; h: number } | null>(null);
  $effect(() => {
    app.view;
    share.panelOpen;
    tick().then(() => {
      const on = rail?.querySelector<HTMLElement>(".item.active");
      marker = on ? { y: on.offsetTop + on.offsetHeight * 0.25, h: on.offsetHeight * 0.5 } : null;
    });
  });

  const tidalLabel = $derived(app.account ? `Tidal · ${app.account.countryCode}` : "Connect Tidal");
  const shareLabel = $derived(
    share.sharing ? `Sharing · ${share.hub.guests.length} guests` : share.inRoom ? `In ${share.guest.session_name}` : "Share or join music",
  );
</script>

<nav class="rail" aria-label="Main" bind:this={rail}>
  <span class="marker" class:shown={!!marker} style:transform="translateY({marker?.y ?? 0}px)" style:height="{marker?.h ?? 0}px" aria-hidden="true"></span>
  <div class="drag" data-tauri-drag-region></div>
  <button class="logo" onclick={() => (app.view = { name: "room" })} aria-label="HomeFi">
    <img src="/logo.svg" alt="" width="42" height="42" />
  </button>
  {#each items as item}
    <button
      class="item"
      class:active={active(item.view)}
      onclick={() => (app.view = (item.view === "tidal" ? { name: "tidal", tab: "playlists" } : { name: item.view }) as View)}
      aria-label={item.label}
      onmouseenter={(e) => show(e, item.label)}
      onmouseleave={hide}
      onfocus={(e) => show(e, item.label)}
      onblur={hide}
    >
      <Icon name={item.icon} size={20} />
      {#if item.view === "downloads" && app.busy}<span class="dot"></span>{/if}
    </button>
  {/each}
  <div class="spacer"></div>
  <button
    class="item"
    class:active={app.view.name === "guest" || share.panelOpen}
    onclick={() => (share.inRoom ? share.open("room") : (share.panelOpen = !share.panelOpen))}
    aria-label="Shared control and party"
    onmouseenter={(e) => show(e, shareLabel)}
    onmouseleave={hide}
    onfocus={(e) => show(e, shareLabel)}
    onblur={hide}
  >
    <Icon name="cast" size={20} />
    {#if share.sharing || share.inRoom}<span class="dot live"></span>{/if}
  </button>
  <button class="item" class:active={app.view.name === "settings"} onclick={() => (app.view = { name: "settings" })} aria-label="Settings" onmouseenter={(e) => show(e, "Settings")} onmouseleave={hide} onfocus={(e) => show(e, "Settings")} onblur={hide}>
    <Icon name="settings" size={20} />
  </button>
  <button
    class="item"
    onclick={() => (app.account ? (app.view = { name: "settings" }) : (app.loginOpen = true))}
    aria-label={app.account ? "Tidal connected" : "Connect Tidal"}
    onmouseenter={(e) => show(e, tidalLabel)}
    onmouseleave={hide}
    onfocus={(e) => show(e, tidalLabel)}
    onblur={hide}
  >
    <span class="tidal" class:on={!!app.account}>T</span>
  </button>
</nav>
{#if tip}<div class="tip" role="tooltip" style:top="{tip.y}px">{tip.text}</div>{/if}

<style>
  .rail {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 0 0 14px;
    min-height: 0;
    overflow-y: auto;
    scrollbar-width: none;
    background: var(--glass);
    border-right: 1px solid var(--glass-border);
    backdrop-filter: blur(24px) saturate(1.3);
    z-index: 3;
  }
  .drag {
    height: 34px;
    width: 100%;
    flex: none;
  }
  .logo {
    margin-bottom: 14px;
    border-radius: min(12px, var(--r-max));
    transition: transform 0.2s;
  }
  .logo:hover {
    transform: rotate(-6deg) scale(1.06);
  }
  .logo img {
    display: block;
    border-radius: min(11px, var(--r-max));
  }
  .item {
    position: relative;
    width: 46px;
    height: 46px;
    display: grid;
    place-items: center;
    border-radius: min(14px, var(--r-max));
    color: var(--muted);
    transition: background 0.15s, color 0.15s;
  }
  .item:hover {
    background: var(--glass-strong);
    color: var(--text);
  }
  .item.active {
    background: color-mix(in srgb, var(--room-accent) 22%, transparent);
    color: var(--accent-text);
    animation: hf-pick calc(0.36s * var(--motion-speed)) var(--ease-pop);
  }
  .marker {
    position: absolute;
    top: 0;
    left: 3px;
    width: 4px;
    border-radius: 4px;
    background: var(--accent-text);
    box-shadow: 0 0 10px color-mix(in srgb, var(--accent-text) 70%, transparent);
    opacity: 0;
    pointer-events: none;
    transition:
      transform calc(0.42s * var(--motion-speed)) var(--ease-pop),
      height 0.3s,
      opacity 0.2s;
  }
  .marker.shown {
    opacity: 1;
  }
  /* subtle motion: it fades to the new place instead of travelling */
  :global(:root[data-motion="subtle"]) .marker {
    transition: opacity 0.2s;
  }
  :global(:root:is([data-skin="retro_amp"], [data-skin="luna"], [data-skin="aluminum"], [data-skin="web2000"])) .marker {
    display: none;
  }
  .tip {
    position: fixed;
    left: 86px;
    z-index: 50;
    padding: 5px 10px;
    border-radius: min(8px, var(--r-max));
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--text);
    font-size: 12.5px;
    white-space: nowrap;
    pointer-events: none;
    transform: translateY(-50%);
    box-shadow: var(--shadow);
    animation: tip-in 0.12s ease-out;
  }
  @keyframes tip-in {
    from {
      opacity: 0;
      transform: translate(-4px, -50%);
    }
  }
  .dot {
    position: absolute;
    top: 9px;
    right: 9px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--room-accent);
    animation: pulse 1.2s infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.3;
    }
  }
  .spacer {
    flex: 1;
  }
  .tidal {
    width: 26px;
    height: 26px;
    display: grid;
    place-items: center;
    border-radius: min(8px, var(--r-max));
    font: 800 13px var(--font-display);
    border: 2px solid var(--faint);
    color: var(--faint);
  }
  .tidal.on {
    border-color: var(--ok);
    color: var(--ok);
  }
</style>
