<script lang="ts">
  // The song's lyrics over the room: NORMAL (the whole text, the line being
  // sung lit and kept a third from the top) or KARAOKE (that line big, filling
  // with the accent as it's sung, and the mascot singing beside it).
  // Escape or ✕ goes back to the room.
  import { untrack } from "svelte";
  import type { LyricsMode, RoomSettings } from "../lib/api";
  import { creditOf, karaokeAt, lineAt, lyrics } from "../lib/lyrics.svelte";
  import { useSource } from "../lib/source.svelte";
  import { app } from "../lib/state.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";
  import MascotHost from "./MascotHost.svelte";
  import { fade, motionLevel, pop, rise, swap } from "../lib/motion";

  const src = useSource();
  const room = $derived(app.settings?.room);
  const mode = $derived<LyricsMode>(room?.lyrics_mode === "karaoke" ? "karaoke" : "normal");
  const size = $derived(room?.lyrics_size ?? "medium");
  const offset = $derived(room?.lyrics_offset_ms ?? 0);
  const track = $derived(src.current);
  const playing = $derived(src.player?.status === "playing");

  const req = $derived(lyrics.request(src));
  // the request is a new object on every player update: follow its key only
  const reqKey = $derived(req?.key ?? null);
  $effect(() => {
    reqKey;
    const r = untrack(() => req);
    if (r) lyrics.load(r);
  });
  const entry = $derived(lyrics.entry(req));
  const data = $derived(entry?.status === "ready" ? entry.lyrics : null);
  const synced = $derived(!!data?.synced);
  const karaoke = $derived(mode === "karaoke" && synced);
  const credit = $derived(data ? creditOf(data.provider) : null);
  const canSeek = $derived(synced && src.can.seek);

  // ------------------------------------------------ position, smooth between player updates
  let base = { pos: 0, at: 0 };
  let now = $state(0);
  $effect(() => {
    const reported = src.player?.position_ms ?? 0;
    const t = performance.now();
    const predicted = base.pos + (t - base.at);
    // a report a little behind what we already show would make the fill jump back
    if (playing && reported <= predicted && predicted - reported < 400) return;
    base = { pos: reported, at: t };
    now = reported;
  });
  $effect(() => {
    if (!playing) return;
    let raf = 0;
    const step = () => {
      now = base.pos + (performance.now() - base.at);
      raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  });
  // positive offset: the lyrics come later (the sound reaches the ears late over Bluetooth)
  const pos = $derived(now - offset);
  const index = $derived(synced && data ? lineAt(data.lines, pos) : -1);
  const frame = $derived(karaoke && data ? karaokeAt(data.lines, pos) : null);

  // the mascot sings beside the lines (or the floating one, wherever it is)
  const singer = $derived(karaoke && (room?.lyrics_mascot ?? true) && (room?.mascot ?? true));
  $effect(() => {
    lyrics.singing = singer && playing;
    return () => (lyrics.singing = false);
  });

  function seekTo(time: number | null) {
    if (time == null || !canSeek) return;
    src.seek(Math.max(0, time + offset));
    resume();
  }

  // ------------------------------------------------ normal mode: following the song
  let scroller = $state<HTMLDivElement | null>(null);
  let following = $state(true);
  let resumeTimer = 0;

  function byHand() {
    if (!synced) return;
    following = false;
    clearTimeout(resumeTimer);
    resumeTimer = window.setTimeout(resume, 4000);
  }
  function resume() {
    clearTimeout(resumeTimer);
    following = true;
    follow(index, true);
  }
  function follow(i: number, smooth: boolean) {
    const box = scroller;
    if (!box) return;
    const el = i >= 0 ? box.querySelector<HTMLElement>(`[data-i="${i}"]`) : null;
    const top = el ? el.offsetTop + el.offsetHeight / 2 - box.clientHeight / 3 : 0;
    box.scrollTo({ top: Math.max(0, top), behavior: smooth && motionLevel() === "full" ? "smooth" : "auto" });
  }
  $effect(() => {
    const i = index;
    if (following && synced && scroller && !karaoke) follow(i, true);
  });
  // a new song starts at the top, following again
  $effect(() => {
    reqKey;
    following = true;
    untrack(() => scroller?.scrollTo({ top: 0 }));
  });
  $effect(() => () => clearTimeout(resumeTimer));

  // ------------------------------------------------ settings from the gear menu
  let menu = $state(false);
  function saveRoom(patch: Partial<RoomSettings>) {
    if (app.settings) app.saveSettings({ room: { ...app.settings.room, ...patch } });
  }
  const sizes = [
    ["small", "S"],
    ["medium", "M"],
    ["large", "L"],
  ] as const;
  const offsetText = $derived(offset === 0 ? "In time" : `${offset > 0 ? "+" : "−"}${(Math.abs(offset) / 1000).toFixed(1)} s`);
  const shift = (d: number) => saveRoom({ lyrics_offset_ms: Math.max(-2000, Math.min(2000, offset + d)) });

  function openSettings() {
    try {
      localStorage.setItem("homefi.settings.page", "lyrics");
    } catch {}
    app.view = { name: "settings" };
  }

  function close() {
    lyrics.setOpen(false);
  }
  function onKey(e: KeyboardEvent) {
    if (e.key !== "Escape" || app.confirmDialog || app.playlistPicker || app.queueMenu) return;
    if (menu) menu = false;
    else close();
  }

  const lit = (n: number, wait: number | null) => wait !== null && Math.ceil(wait / 1000) >= n;
</script>

<svelte:window onkeydown={onKey} />

<section class="lyrics size-{size}" class:karaoke aria-label="Lyrics" in:rise={{ y: 18, duration: 360 }}>
  <header class="bar">
    {#if track}
      <div class="now">
        <Cover path={track.path} size={42} radius={8} />
        {#key track.path}
        <div class="meta" in:swap={{ x: 10 }}>
          <div class="title ellipsis">{track.title}</div>
          <div class="artist ellipsis">{track.artist}</div>
        </div>
        {/key}
      </div>
    {/if}
    <div class="modes" role="radiogroup" aria-label="Lyrics mode">
      <button role="radio" aria-checked={mode === "normal"} class:on={mode === "normal"} onclick={() => saveRoom({ lyrics_mode: "normal" })}>
        <Icon name="lyrics" size={14} /> Normal
      </button>
      <button role="radio" aria-checked={mode === "karaoke"} class:on={mode === "karaoke"} onclick={() => saveRoom({ lyrics_mode: "karaoke" })}>
        <Icon name="mic" size={14} /> Karaoke
      </button>
    </div>
    <div class="tools">
      <div class="gear">
        <button class="tool" class:on={menu} title="Lyrics options" aria-label="Lyrics options" aria-expanded={menu} onclick={() => (menu = !menu)}>
          <Icon name="settings" size={16} />
        </button>
        {#if menu}
          <button class="scrim" aria-label="Close" onclick={() => (menu = false)}></button>
          <div class="menu" role="dialog" aria-label="Lyrics options" transition:pop>
            <div class="row">
              <span>Text size</span>
              <div class="seg">
                {#each sizes as [id, label]}
                  <button class:on={size === id} aria-label="{id} text" onclick={() => saveRoom({ lyrics_size: id })}>{label}</button>
                {/each}
              </div>
            </div>
            <div class="row">
              <span>Timing <small>{offsetText}</small></span>
              <div class="seg">
                <button aria-label="Lyrics earlier" disabled={offset <= -2000} onclick={() => shift(-100)}><Icon name="minus" size={13} /></button>
                <button aria-label="Back in time" disabled={offset === 0} onclick={() => saveRoom({ lyrics_offset_ms: 0 })}>0</button>
                <button aria-label="Lyrics later" disabled={offset >= 2000} onclick={() => shift(100)}><Icon name="plus" size={13} /></button>
              </div>
            </div>
            <p class="hint">Lyrics late or early? Bluetooth speakers lag: move them later with +.</p>
            <label class="check">
              <input type="checkbox" checked={room?.lyrics_mascot ?? true} onchange={(e) => saveRoom({ lyrics_mascot: e.currentTarget.checked })} />
              HomeFito sings in karaoke
            </label>
            <button class="more" onclick={openSettings}>All lyrics settings</button>
          </div>
        {/if}
      </div>
      <button class="tool" title="Back to the room (Esc)" aria-label="Close the lyrics" onclick={close}><Icon name="x" size={16} /></button>
    </div>
  </header>

  <div class="body">
    {#if !entry || entry.status === "loading"}
      <div class="skeleton" aria-label="Loading the lyrics" role="status">
        {#each [72, 54, 86, 40, 66, 78, 48] as w, i}<span style:width="{w}%" style:--d="{i * 90}ms"></span>{/each}
      </div>
    {:else if entry.status === "error"}
      <div class="empty" in:rise>
        <Icon name="alert" size={30} />
        <p>{entry.message}</p>
        <button class="btn tonal" onclick={() => req && lyrics.load(req, true)}><Icon name="refresh" size={15} /> Retry</button>
      </div>
    {:else if entry.status === "none" || !data}
      <div class="empty" in:rise>
        <Icon name="music" size={30} />
        <p>No lyrics for this song</p>
        <small>Instrumentals have none, and not every song has them yet.</small>
      </div>
    {:else if karaoke && frame}
      <div class="stage" in:rise>
        {#if singer && !room?.mascot_free}
          <div class="singer"><MascotHost size={size === "large" ? 230 : 190} singing /></div>
        {/if}
        <div class="lines" aria-live="off">
          <p class="prev">{frame.previous >= 0 ? data.lines[frame.previous].text : ""}</p>
          {#if frame.current >= 0}
            {#key frame.current}
              <p class="sung">
                <span class="fill" style:--f="{(frame.fill * 100).toFixed(2)}%">{data.lines[frame.current].text}</span>
              </p>
            {/key}
          {:else}
            <div class="interlude" aria-label="Instrumental">
              <span class="notes">♪ ♪ ♪</span>
              <span class="dots" class:far={frame.waitMs !== null && frame.waitMs > 3000}>
                {#each [3, 2, 1] as n}<i class:lit={lit(n, frame.waitMs)}></i>{/each}
              </span>
            </div>
          {/if}
          <p class="next">{frame.next >= 0 && frame.next !== frame.current ? data.lines[frame.next].text : ""}</p>
        </div>
      </div>
    {:else}
      {#if mode === "karaoke" && !synced}
        <p class="note"><Icon name="info" size={14} /> These lyrics aren’t synced, so karaoke can’t follow them. Here they are whole.</p>
      {:else if !synced}
        <p class="note"><Icon name="info" size={14} /> These lyrics aren’t synced</p>
      {/if}
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions (scrolling by hand pauses the follow) -->
      <div
        class="scroller"
        class:plain={!synced}
        in:rise={{ y: 16, duration: 380 }}
        bind:this={scroller}
        onwheel={byHand}
        ontouchmove={byHand}
        onpointerdown={(e) => e.target === scroller && byHand()}
        onkeydown={(e) => ["ArrowUp", "ArrowDown", "PageUp", "PageDown", "Home", "End"].includes(e.key) && byHand()}
        role="region"
        aria-label="Lyrics text"
        tabindex="-1"
      >
        {#each data.lines as line, i}
          {#if !line.text.trim()}
            <div class="gap" class:on={i === index} data-i={i}>{i === index ? "♪" : ""}</div>
          {:else if canSeek}
            <button class="line" class:on={i === index} class:past={i < index} data-i={i} onclick={() => seekTo(line.time_ms)} title="Play from here">{line.text}</button>
          {:else}
            <p class="line" class:on={i === index} class:past={synced && i < index} data-i={i}>{line.text}</p>
          {/if}
        {/each}
      </div>
      {#if !following && synced}
        <button class="pill" onclick={resume} out:fade><Icon name="down" size={14} /> Back to the current line</button>
      {/if}
    {/if}
  </div>

  {#if credit && entry?.status === "ready"}<footer class="credit">{credit}</footer>{/if}
</section>

<style>
  .lyrics {
    /* the skins override these few */
    --ly-text: var(--text);
    --ly-dim: color-mix(in srgb, var(--text) 38%, transparent);
    --ly-faint: color-mix(in srgb, var(--text) 20%, transparent);
    --ly-fill: var(--accent-text);
    --ly-glow: color-mix(in srgb, var(--room-accent) 45%, transparent);
    --ly-panel: var(--glass);
    --ly-line: 26px;
    --ly-big: clamp(30px, 5.2cqw, 56px);
    --ly-font: var(--font-display);
    position: relative;
    flex: 1;
    min-height: min(560px, calc(100vh - 210px));
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 10px;
    color: var(--ly-text);
    container-type: inline-size;
  }
  .size-small {
    --ly-line: 19px;
    --ly-big: clamp(24px, 4cqw, 40px);
  }
  .size-large {
    --ly-line: 34px;
    --ly-big: clamp(36px, 6.6cqw, 74px);
  }

  /* ------------------------------------------------ header */
  .bar {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 8px 10px;
    border-radius: min(14px, var(--r-max));
    background: var(--ly-panel);
    border: 1px solid var(--glass-border);
    backdrop-filter: blur(18px) saturate(1.3);
    /* the blur makes the bar its own layer: lift it, menu included, over the lines */
    position: relative;
    z-index: 5;
  }
  .now {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    flex: 1;
  }
  .meta {
    min-width: 0;
  }
  .title {
    font-weight: 700;
    font-size: 14px;
  }
  .artist {
    font-size: 12px;
    color: var(--muted);
  }
  .modes,
  .seg {
    display: inline-flex;
    padding: 3px;
    gap: 2px;
    border-radius: min(999px, var(--r-max));
    background: var(--glass-strong);
    border: 1px solid var(--glass-border);
  }
  .modes button,
  .seg button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 12px;
    border-radius: min(999px, var(--r-max));
    font-size: 12.5px;
    font-weight: 600;
    color: var(--muted);
  }
  .seg button {
    padding: 4px 10px;
    min-width: 30px;
    justify-content: center;
  }
  .modes button:hover,
  .seg button:hover:not(:disabled) {
    color: var(--text);
  }
  .modes button.on,
  .seg button.on {
    color: var(--on-primary);
    background: var(--primary);
  }
  .seg button:disabled {
    opacity: 0.4;
  }
  .tools {
    display: flex;
    gap: 4px;
    flex: none;
  }
  .tool {
    width: 34px;
    height: 34px;
    display: grid;
    place-items: center;
    border-radius: min(999px, var(--r-max));
    color: var(--muted);
  }
  .tool:hover,
  .tool.on {
    color: var(--text);
    background: var(--glass-strong);
  }
  .gear {
    position: relative;
  }
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 19;
    cursor: default;
  }
  .menu {
    position: absolute;
    right: 0;
    top: calc(100% + 8px);
    z-index: 20;
    width: 280px;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    border-radius: min(14px, var(--r-max));
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
    color: var(--text);
    font-size: 13px;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }
  .row small {
    color: var(--muted);
    margin-left: 4px;
    font-variant-numeric: tabular-nums;
  }
  .hint {
    margin: -4px 0 0;
    font-size: 11.5px;
    color: var(--muted);
  }
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .more {
    align-self: flex-start;
    font-size: 12.5px;
    color: var(--accent-text);
  }
  .more:hover {
    text-decoration: underline;
  }

  /* ------------------------------------------------ body */
  .body {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .note {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0 0 6px;
    font-size: 12.5px;
    color: var(--muted);
  }
  .scroller {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 8px 4px 50vh;
    outline: none;
    /* soft edges top and bottom */
    mask-image: linear-gradient(180deg, transparent 0, #000 7%, #000 82%, transparent 100%);
    scrollbar-width: thin;
  }
  .scroller.plain {
    padding-bottom: 40px;
    mask-image: none;
  }
  .line {
    display: block;
    width: 100%;
    margin: 0;
    padding: 0.22em 0.3em;
    text-align: left;
    font: 750 var(--ly-line) / 1.3 var(--ly-font);
    letter-spacing: -0.01em;
    color: var(--ly-dim);
    border-radius: min(10px, var(--r-max));
    transform-origin: left center;
    transition: color 0.35s, transform 0.35s cubic-bezier(0.2, 0.8, 0.2, 1), opacity 0.35s;
    overflow-wrap: anywhere;
  }
  .scroller.plain .line {
    color: var(--ly-text);
    font-weight: 600;
  }
  .line.past {
    color: var(--ly-faint);
  }
  .line.on {
    color: var(--ly-text);
    transform: scale(1.06);
    text-shadow: 0 0 28px var(--ly-glow);
  }
  button.line:hover {
    background: var(--glass-strong);
    color: var(--ly-text);
  }
  .gap {
    height: calc(var(--ly-line) * 0.9);
    padding-left: 0.3em;
    font-size: var(--ly-line);
    line-height: 1;
    color: var(--ly-fill);
  }
  .pill {
    position: absolute;
    left: 50%;
    bottom: 16px;
    transform: translateX(-50%);
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 16px;
    border-radius: min(999px, var(--r-max));
    font-size: 13px;
    font-weight: 700;
    color: var(--on-primary);
    background: var(--primary);
    box-shadow: var(--shadow);
    animation: rise 0.25s ease-out;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translate(-50%, 8px);
    }
  }

  /* ------------------------------------------------ karaoke */
  .stage {
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: clamp(12px, 3cqw, 40px);
    padding: 0 clamp(8px, 2cqw, 32px);
  }
  .singer {
    flex: none;
    align-self: center;
  }
  .lines {
    flex: 1;
    max-width: 980px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 0.6em;
    font-family: var(--ly-font);
  }
  .prev,
  .next {
    margin: 0;
    min-height: 1.3em;
    font-weight: 700;
    font-size: calc(var(--ly-big) * 0.46);
    line-height: 1.3;
  }
  .prev {
    color: var(--ly-faint);
  }
  .next {
    color: var(--ly-dim);
  }
  .sung {
    margin: 0;
    font: 850 var(--ly-big) / 1.16 var(--ly-font);
    letter-spacing: -0.02em;
    overflow-wrap: anywhere;
    animation: enter 0.3s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  /* inline: the fill runs across the wrapped rows one after the other */
  .fill {
    background-image: linear-gradient(90deg, var(--ly-fill) 0 var(--f), var(--ly-dim) var(--f) 100%);
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
    filter: drop-shadow(0 0 18px var(--ly-glow));
  }
  @keyframes enter {
    from {
      opacity: 0.2;
      transform: translateY(0.35em) scale(0.96);
    }
  }
  .interlude {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    color: var(--ly-fill);
    font-size: var(--ly-big);
    line-height: 1.16;
  }
  .notes {
    letter-spacing: 0.25em;
    animation: bob 1.6s ease-in-out infinite;
  }
  .dots {
    display: flex;
    gap: 12px;
  }
  .dots i {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--ly-faint);
    transition: background 0.2s, transform 0.2s;
  }
  .dots i.lit {
    background: var(--ly-fill);
    transform: scale(1.1);
  }
  .dots.far i.lit {
    opacity: 0.55;
    animation: pulse 1.4s ease-in-out infinite;
  }
  @keyframes bob {
    50% {
      transform: translateY(-6px);
    }
  }
  @keyframes pulse {
    50% {
      opacity: 0.25;
    }
  }

  /* ------------------------------------------------ states */
  .skeleton {
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding: 24px 6px;
  }
  .skeleton span {
    height: calc(var(--ly-line) * 0.9);
    border-radius: min(8px, var(--r-max));
    background: linear-gradient(90deg, var(--glass-strong), color-mix(in srgb, var(--text) 12%, transparent), var(--glass-strong));
    background-size: 200% 100%;
    animation: shimmer 1.3s linear infinite;
    animation-delay: var(--d);
  }
  @keyframes shimmer {
    to {
      background-position: -200% 0;
    }
  }
  .empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    text-align: center;
    color: var(--muted);
  }
  .empty p {
    margin: 0;
    font: 700 20px var(--font-display);
    color: var(--text);
    max-width: 460px;
  }
  .credit {
    flex: none;
    text-align: center;
    font-size: 11px;
    color: var(--faint);
    letter-spacing: 0.02em;
  }

  /* subtle motion (or the system asking for less): the words stop bobbing and gliding */
  :global(:root[data-motion="subtle"]) :is(.line, .sung, .notes, .dots i, .skeleton span, .pill) {
    animation: none !important;
    transition: none !important;
  }

  @container (max-width: 640px) {
    .now {
      display: none;
    }
    .bar {
      justify-content: space-between;
    }
    .stage {
      flex-direction: column;
    }
    .singer :global(.host) {
      transform: scale(0.7);
    }
  }

  /* ------------------------------------------------ skins */
  /* Retro Amp: green LCD on black, like the player's display */
  :global(:root[data-skin="retro_amp"]) .lyrics {
    --ly-text: #00e000;
    --ly-dim: #0a6a0a;
    --ly-faint: #0a3f0a;
    --ly-fill: #00ff00;
    --ly-glow: rgb(0 255 0 / 0.35);
    --ly-font: var(--font-mono);
  }
  :global(:root[data-skin="retro_amp"]) .body {
    background: #000;
    box-shadow: var(--bevel-in);
    padding: 10px;
  }
  :global(:root[data-skin="retro_amp"]) .body::after {
    content: "";
    position: absolute;
    inset: 0;
    pointer-events: none;
    background: repeating-linear-gradient(0deg, rgb(0 0 0 / 0.28) 0 1px, transparent 1px 3px);
  }
  :global(:root[data-skin="retro_amp"]) .bar {
    background: linear-gradient(180deg, #34344a, #1e1e2c);
    box-shadow: var(--bevel);
    border: 0;
  }
  :global(:root[data-skin="retro_amp"]) .sung,
  :global(:root[data-skin="retro_amp"]) .line {
    font-weight: 700;
    letter-spacing: 0;
    text-transform: uppercase;
  }
  :global(:root[data-skin="retro_amp"]) .fill {
    background-image: linear-gradient(90deg, #00ff00 0 var(--f), #0a6a0a var(--f) 100%);
  }
  :global(:root[data-skin="retro_amp"]) .empty p,
  :global(:root[data-skin="retro_amp"]) .empty {
    color: #00e000;
  }
  :global(:root[data-skin="retro_amp"]) .credit {
    color: #0a8a0a;
    font-family: var(--font-mono);
  }
  /* light skins: paper behind the words, dark ink, the skin's accent for the fill */
  :global(:root[data-skin="luna"]) .lyrics,
  :global(:root[data-skin="aluminum"]) .lyrics,
  :global(:root[data-skin="web2000"]) .lyrics {
    --ly-dim: color-mix(in srgb, var(--text) 45%, transparent);
    --ly-faint: color-mix(in srgb, var(--text) 26%, transparent);
    --ly-glow: transparent;
  }
  :global(:root[data-skin="luna"]) .body,
  :global(:root[data-skin="aluminum"]) .body,
  :global(:root[data-skin="web2000"]) .body {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: min(12px, var(--r-max));
    padding: 10px 14px;
    box-shadow: var(--shadow);
  }
  :global(:root[data-skin="luna"]) .bar {
    background: var(--titlebar);
    color: #fff;
    border-color: #0047c4;
  }
  :global(:root[data-skin="luna"]) .bar .artist,
  :global(:root[data-skin="luna"]) .bar .tool {
    color: rgb(255 255 255 / 0.85);
  }
  :global(:root[data-skin="luna"]) .bar .tool:hover {
    background: rgb(255 255 255 / 0.2);
    color: #fff;
  }
  :global(:root[data-skin="luna"]) .modes {
    background: rgb(255 255 255 / 0.2);
    border-color: rgb(255 255 255 / 0.35);
  }
  :global(:root[data-skin="luna"]) .modes button {
    color: #fff;
  }
  :global(:root[data-skin="luna"]) .modes button.on {
    background: #3c9a3c;
    color: #fff;
  }
  :global(:root[data-skin="aluminum"]) .bar {
    background: var(--metal);
    border-color: #8d8d8d;
  }
  :global(:root[data-skin="aluminum"]) .modes button.on {
    background: var(--aqua);
    color: #0d2240;
  }
  :global(:root[data-skin="web2000"]) .modes button.on {
    background: var(--gloss-orange);
    color: #fff;
  }
  :global(:root[data-skin="web2000"]) .lyrics {
    --ly-fill: #e06a00;
  }
</style>
