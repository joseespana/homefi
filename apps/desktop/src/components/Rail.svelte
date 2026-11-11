<script lang="ts">
  import { tick } from "svelte";
  import { disc } from "../lib/disc.svelte";
  import { share } from "../lib/share.svelte";
  import { dj } from "../lib/dj.svelte";
  import { exercise } from "../lib/exercise.svelte";
  import { app, type View } from "../lib/state.svelte";
  import { pop } from "../lib/motion";
  import { conversations } from "../lib/party.svelte";
  import ConversationsMenu from "./playlists/ConversationsMenu.svelte";
  import Icon from "./Icon.svelte";

  const base: { view: View["name"]; label: string; icon: string }[] = [
    { view: "room", label: "Listening room", icon: "headphones" },
    { view: "albums", label: "Albums", icon: "disc" },
    { view: "songs", label: "Songs", icon: "music" },
    { view: "artists", label: "Artists", icon: "user" },
    { view: "playlists", label: "Playlists", icon: "queue" },
    { view: "sound", label: "Sound", icon: "eq" },
    { view: "tidal", label: "My Tidal", icon: "heart" },
    { view: "search", label: "Search Tidal", icon: "search" },
    { view: "downloads", label: "Downloads", icon: "download" },
  ];
  // the Disc item follows the "Disc item in the sidebar" setting (a disc in, always or never)
  const withDisc = $derived(disc.railVisible ? [...base.slice(0, -1), { view: "disc" as const, label: "Disc", icon: "cd" }, ...base.slice(-1)] : base);
  // DJ follows "DJ in the sidebar" (Settings → DJ); it stays while DJ mode is on
  const withDj = $derived(dj.settings.show_entry || dj.on ? [...withDisc.slice(0, 6), { view: "dj" as const, label: dj.on ? "DJ · on" : "DJ", icon: "dj" }, ...withDisc.slice(6)] : withDisc);
  // Exercise follows "Show Exercise in the sidebar" (Settings → Exercise), after Downloads; it stays while a ride or a build goes on
  const items = $derived(exercise.settings.show_entry || exercise.busy ? [...withDj, { view: "exercise" as const, label: exercise.riding ? "Exercise · ride" : exercise.busy ? "Exercise · building" : "Exercise", icon: "bike" }] : withDj);

  const active = (name: string) =>
    app.view.name === name ||
    (name === "albums" && app.view.name === "album") ||
    (name === "artists" && app.view.name === "artist") ||
    (name === "playlists" && (app.view.name === "playlist" || app.view.name === "concert" || app.view.name === "party" || app.view.name === "chat-playlist" || app.view.name === "home-playlist" || (app.view.name === "tidal-item" && app.view.back?.name === "playlists"))) ||
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

  // A short window: the places that don't fit fold into a "More" button at the end of the list,
  // whose menu opens beside the rail. Measured from the rail itself, so every skin's sizes count.
  let fit = $state(Infinity);
  let bottom = $state<HTMLElement>();
  $effect(() => {
    if (!rail) return;
    const measure = () => {
      const firstItem = rail?.querySelector<HTMLElement>(".item");
      if (!rail || !firstItem || !bottom) return;
      const gap = parseFloat(getComputedStyle(rail).rowGap) || 0;
      const pitch = firstItem.offsetHeight + gap;
      const room = rail.clientHeight - firstItem.offsetTop - bottom.offsetHeight - gap;
      fit = Math.max(1, Math.floor((room + gap) / pitch));
    };
    const ro = new ResizeObserver(measure);
    ro.observe(rail);
    measure();
    return () => ro.disconnect();
  });
  // all of them when they fit; otherwise one place less, for the More button
  const shown = $derived(fit >= items.length ? items : items.slice(0, Math.max(0, fit - 1)));
  const folded = $derived(items.slice(shown.length));
  const foldedActive = $derived(folded.some((i) => active(i.view)));
  const foldedBusy = $derived(folded.some((i) => busy(i.view)));

  let more = $state<{ y: number } | null>(null);
  let moreBtn = $state<HTMLElement>();
  function toggleMore() {
    hide();
    if (more) return (more = null);
    const r = moreBtn!.getBoundingClientRect();
    // the menu grows up from the button when it's low in the window
    more = { y: Math.min(r.top, window.innerHeight - 16 - folded.length * 40 - 12) };
  }
  // the folded list changed under an open menu (window resized): it closes rather than lie
  $effect(() => {
    folded.length;
    more = null;
  });

  function go(view: View["name"]) {
    more = null;
    app.view = (view === "tidal" ? { name: "tidal", tab: "playlists" } : { name: view }) as View;
  }
  const busy = (view: string) => (view === "downloads" && app.busy) || (view === "disc" && disc.ripping) || (view === "dj" && dj.on) || (view === "exercise" && exercise.busy);

  let chatsBtn = $state<HTMLElement>();
  const chatsLabel = $derived(`AI chats · ${conversations.live.length}`);
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
  {#each shown as item}
    <button
      class="item"
      class:active={active(item.view)}
      onclick={() => go(item.view)}
      aria-label={item.label}
      onmouseenter={(e) => show(e, item.label)}
      onmouseleave={hide}
      onfocus={(e) => show(e, item.label)}
      onblur={hide}
    >
      <Icon name={item.icon} size={20} />
      {#if busy(item.view)}<span class="dot"></span>{/if}
    </button>
  {/each}
  {#if folded.length}
    <button
      class="item"
      class:active={foldedActive || !!more}
      bind:this={moreBtn}
      onclick={toggleMore}
      aria-label="More places"
      aria-haspopup="menu"
      aria-expanded={!!more}
      onmouseenter={(e) => !more && show(e, foldedActive ? `More · ${folded.find((i) => active(i.view))?.label}` : "More")}
      onmouseleave={hide}
      onfocus={(e) => !more && show(e, "More")}
      onblur={hide}
    >
      <Icon name="more" size={20} />
      {#if foldedBusy}<span class="dot"></span>{/if}
    </button>
  {/if}
  <div class="spacer"></div>
  <div class="bottom" bind:this={bottom}>
  {#if conversations.live.length}
    <!-- AI playlist chats keep going when their screen is left; this brings them back -->
    <button
      class="item"
      class:active={conversations.menuOpen || conversations.list.some((c) => conversations.isShown(c.id))}
      bind:this={chatsBtn}
      onclick={() => {
        hide();
        conversations.menuOpen = !conversations.menuOpen;
      }}
      aria-label="AI chats"
      aria-haspopup="menu"
      aria-expanded={conversations.menuOpen}
      onmouseenter={(e) => !conversations.menuOpen && show(e, chatsLabel)}
      onmouseleave={hide}
      onfocus={(e) => !conversations.menuOpen && show(e, chatsLabel)}
      onblur={hide}
    >
      <Icon name="ai" size={20} />
      {#if conversations.busy}<span class="dot"></span>{/if}
    </button>
  {/if}
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
    <span class="tidal" class:on={!!app.account}><svg viewBox="0 0 22 22" width="22" height="22" aria-hidden="true"><path d="M6 6h10v2.5h-3.75V16h-2.5V8.5H6z" /></svg></span>
  </button>
  </div>
</nav>
{#if more}
  <button class="scrim" aria-label="Close" tabindex="-1" onclick={() => (more = null)}></button>
  <div class="more-menu" role="menu" aria-label="More places" style:top="{more.y}px" transition:pop={{ origin: "top left", y: 0 }}>
    {#each folded as item}
      <button role="menuitem" class:on={active(item.view)} onclick={() => go(item.view)}>
        <Icon name={item.icon} size={17} />
        <span>{item.label}</span>
        {#if busy(item.view)}<span class="mdot"></span>{/if}
      </button>
    {/each}
  </div>
{/if}
<ConversationsMenu anchor={chatsBtn} />
<svelte:window onkeydown={(e) => more && e.key === "Escape" && (more = null)} />
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
    /* places that don't fit fold into "More" instead of scrolling */
    overflow: hidden;
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
    border-radius: var(--r-sm);
    transition: transform 0.2s;
  }
  .logo:hover {
    transform: rotate(-6deg) scale(1.06);
  }
  .logo img {
    display: block;
    border-radius: var(--r-sm);
  }
  .item {
    position: relative;
    width: 46px;
    height: 46px;
    display: grid;
    place-items: center;
    border-radius: var(--r-md);
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
    border-radius: var(--r-xs);
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
    border-radius: var(--r-sm);
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
  .bottom {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: inherit;
    flex: none;
  }
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 49;
    background: transparent;
    cursor: default;
  }
  .more-menu {
    position: fixed;
    left: 84px;
    z-index: 50;
    min-width: 190px;
    padding: 6px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    border-radius: var(--r-md);
    background: color-mix(in srgb, var(--surface) 82%, transparent);
    border: 1px solid var(--glass-border);
    backdrop-filter: blur(24px) saturate(1.4);
    box-shadow: var(--shadow);
  }
  .more-menu button {
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    height: 38px;
    padding: 0 12px;
    border-radius: var(--r-sm);
    color: var(--muted);
    font-size: 13.5px;
    text-align: left;
    transition: background 0.15s, color 0.15s;
  }
  .more-menu button:hover,
  .more-menu button:focus-visible {
    background: var(--glass-strong);
    color: var(--text);
  }
  .more-menu button.on {
    background: color-mix(in srgb, var(--room-accent) 22%, transparent);
    color: var(--accent-text);
  }
  .mdot {
    margin-left: auto;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--room-accent);
    animation: pulse 1.2s infinite;
  }
  .tidal {
    width: 26px;
    height: 26px;
    display: grid;
    place-items: center;
    border-radius: var(--r-sm);
    box-sizing: border-box;
    border: 2px solid var(--faint);
    color: var(--faint);
  }
  /* a drawn T, centred by geometry: a font letter sat off centre (its ascent and side bearings) */
  .tidal svg {
    fill: currentColor;
  }
  .tidal.on {
    border-color: var(--ok);
    color: var(--ok);
  }
</style>
