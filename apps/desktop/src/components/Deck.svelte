<script lang="ts">
  import SeekBar from "./SeekBar.svelte";
  import { api, formatTime, khz, outputIcon, type Repeat } from "../lib/api";
  import { share } from "../lib/share.svelte";
  import { shownSource as src } from "../lib/source.svelte";
  import { app } from "../lib/state.svelte";
  import { visual } from "../lib/visual.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";
  import Knob from "./Knob.svelte";
  import OutputMenu from "./OutputMenu.svelte";
  import NowActions from "./NowActions.svelte";
  import { lyrics } from "../lib/lyrics.svelte";
  import { pop, swap } from "../lib/motion";
  import { sound } from "../lib/sound.svelte";
  import HowSounds from "./sound/HowSounds.svelte";

  let { queueOpen = $bindable(false) }: { queueOpen?: boolean } = $props();

  // in someone else's room: its song ("From <session>"), and the controls the hub allows
  const p = $derived(src.player);
  const track = $derived(src.current);
  const playing = $derived(p?.status === "playing");
  const duration = $derived(p?.duration_ms ?? track?.durationMs ?? 0);

  let dragging = $state<number | null>(null);
  const hash = (text: string) => [...text].reduce((h, c) => (h * 31 + c.charCodeAt(0)) | 0, 7);
  const position = $derived(dragging ?? p?.position_ms ?? 0);
  let outputsOpen = $state(false);

  // narrow windows: a smaller knob, and sharing / lyrics / mini player in a "more" menu
  let narrow = $state(false);
  let moreOpen = $state(false);
  $effect(() => {
    const mq = matchMedia("(max-width: 920px)");
    const update = () => {
      narrow = mq.matches;
      if (!narrow) moreOpen = false;
    };
    update();
    mq.addEventListener("change", update);
    return () => mq.removeEventListener("change", update);
  });
  const lyricsShown = $derived(lyrics.open && (app.view.name === "room" || app.view.name === "guest"));
  const toggleShare = () => (share.inRoom ? share.open("room") : (share.panelOpen = !share.panelOpen));
  const toggleLyrics = () => (lyricsShown ? lyrics.setOpen(false) : lyrics.show());
  const shareTitle = $derived(share.sharing ? "Sharing this music" : share.inRoom ? `In ${share.guest.session_name}` : "Share this music");

  let volume = $state(0.8);
  let touched = false;
  $effect(() => {
    // the knob is always this computer's volume (the party played here follows it)
    if (app.player && !touched) volume = app.player.volume;
  });
  function setVolume(v: number) {
    touched = true;
    volume = v;
    api.volume(v);
  }

  const nextRepeat: Record<Repeat, Repeat> = { off: "all", all: "one", one: "off" };
  const outputLabel = $derived(p?.stream?.device ?? p?.preferred_device ?? "System default");
  // 0..1 mini level bars in the deck
  const level = $derived(Math.max(visual.vuLeft, visual.vuRight));
  // "HD 650 + Electronic": the output's profile and the taste layer playing
  const layers = $derived.by(() => {
    if (!p?.eq_active) return null;
    const rule = p.taste_rule ? sound.taste.rules.find((r) => r.id === p.taste_rule) : null;
    return [p.eq_profile, rule?.scope.name ?? p.taste_label].filter(Boolean).join(" + ") || null;
  });
  const loudness = $derived(Math.abs(p?.loudness_db ?? 0) >= 0.05 ? (p?.loudness_db ?? 0) : null);
</script>

<footer class="deck">
  <div class="left">
    <button class="now" onclick={() => (src.remote ? share.open() : (app.view = { name: "room" }))} disabled={!track} aria-label={src.remote ? "Open the room" : "Open the listening room"}>
      {#if track}
        <Cover path={track.path} size={58} radius={10} />
        {#key track.path}
        <div class="meta" in:swap={{ x: 10 }}>
          <div class="title ellipsis">{track.title}</div>
          <div class="artist ellipsis">{track.artist}</div>
          {#if src.remote}
            <div class="chip from"><Icon name="cast" size={11} /> {src.label}</div>
          {:else if p?.stream}
            <div class="chip">
              <span class="led" class:warn={p.stream.resampled}></span>
              <span class="fmt"><span class="codec">{p.stream.codec.toUpperCase()}&nbsp;</span>{p.stream.bits_per_sample ? `${p.stream.bits_per_sample}/` : ""}{khz(p.stream.source_rate)}{#if p.stream.resampled}&nbsp;→ {khz(p.stream.output_rate)}{/if}</span>
              {#if p.eq_active}<span class="eq-tag ellipsis" title={`Equalizer on: ${layers ?? "on"} (not bit-perfect)`}>EQ{#if sound.ui.layer_chip && layers}<span class="layers">&nbsp;· {layers}</span>{/if}</span>{/if}
              {#if loudness !== null}<span class="eq-tag loud" title="Loudness normalization: not bit-perfect">{loudness > 0 ? "+" : "−"}{Math.abs(loudness).toFixed(1)} dB</span>{/if}
              {#if p.fading}<span class="eq-tag fade" style:--p={p.fading.progress} title={`Crossfading (${p.fading.seconds.toFixed(1)} s): not bit-perfect until it ends`}>Fade</span>{/if}
            </div>
          {/if}
        </div>
        {/key}
      {:else}
        <span class="muted">Nothing playing</span>
      {/if}
    </button>

    <NowActions compact />
  </div>

  <div class="center">
    <div class="transport">
      <button class="hw small" class:on={p?.shuffle} aria-label="Shuffle" title="Shuffle" onclick={() => src.shuffle(!p?.shuffle)} disabled={!src.can.shuffle}>
        <Icon name="shuffle" size={16} />
      </button>
      <button class="hw" aria-label="Previous" title="Previous" onclick={() => src.previous()} disabled={!src.can.previous}><Icon name="prev" size={18} /></button>
      <button class="hw play" aria-label={playing ? "Pause" : "Play"} title={playing ? "Pause" : "Play"} onclick={() => src.toggle()} disabled={!p?.queue.length || !src.can.toggle}>
        <Icon name={playing ? "pause" : "play"} size={22} />
      </button>
      <button class="hw" aria-label={src.remote && share.votes ? "Vote to skip" : "Next"} title={src.remote && share.votes ? "Vote to skip" : "Next"} onclick={() => src.next()} disabled={!src.can.next}><Icon name="next" size={18} /></button>
      <button class="hw small" class:on={p?.repeat !== "off"} aria-label="Repeat: {p?.repeat}" title="Repeat: {p?.repeat}" onclick={() => p && src.repeat(nextRepeat[p.repeat])} disabled={!src.can.repeat}>
        <Icon name={p?.repeat === "one" ? "repeat1" : "repeat"} size={16} />
      </button>
    </div>
    <div class="seek">
      <span class="time">{formatTime(position)}</span>
      <SeekBar
        fraction={duration ? (p?.position_ms ?? 0) / duration : 0}
        style={app.settings?.progress_style}
        thickness={app.settings?.progress_thickness}
        animated={app.settings?.progress_animated ?? true}
        {playing}
        seed={hash(track?.path ?? "")}
        disabled={!duration || !src.can.seek}
        onscrub={(f) => (dragging = f === null ? null : f * duration)}
        onseek={(f) => {
          src.seek(Math.round(f * duration));
          dragging = null;
        }}
      />
      <span class="time">{formatTime(duration)}</span>
    </div>
  </div>

  <div class="right">
    <div class="levels" aria-hidden="true">
      {#each [0, 1, 2, 3, 4, 5, 6, 7] as i}
        <i class:lit={level > i / 8} class:hot={i >= 6}></i>
      {/each}
    </div>
    <div class="output">
      <button class="out-btn" title="Audio output" onclick={() => (outputsOpen = !outputsOpen)}>
        <Icon name={outputIcon(outputLabel)} size={16} />
        <span class="ellipsis">{outputLabel}</span>
      </button>
      {#if outputsOpen}
        <button class="scrim" aria-label="Close" onclick={() => (outputsOpen = false)}></button>
        <OutputMenu onclose={() => (outputsOpen = false)} />
      {/if}
    </div>
    {#if sound.ui.react_deck && !src.remote}
      <HowSounds place="deck" look="hw" up />
    {/if}
    {#if app.settings?.eq_ui?.deck_button ?? true}
      <button
        class="hw small eq-btn"
        class:on={!src.remote && p?.eq_active}
        class:active={app.view.name === "eq"}
        title={p?.eq_active ? `Equalizer: ${layers ?? "on"} (not bit-perfect)` : app.settings?.eq.enabled ? "Equalizer: on, flat for this output" : "Equalizer: off (bit-perfect)"}
        aria-label="Equalizer"
        onclick={() => (app.view = app.view.name === "eq" ? { name: "room" } : { name: "eq" })}
      >
        <Icon name="eq" size={16} />
        {#if !src.remote && p?.eq_active}<span class="eq-dot"></span>{/if}
      </button>
    {/if}
    <Knob value={volume} onchange={setVolume} size={narrow ? 40 : 52} />
    {#if narrow}
      <div class="more">
        <button class="hw small" class:on={moreOpen || share.sharing || share.inRoom || lyricsShown} title="More" aria-label="More" aria-expanded={moreOpen} onclick={() => (moreOpen = !moreOpen)}>
          <Icon name="more" size={16} />
          {#if share.sharing && share.hub.guests.length}<span class="count">{share.hub.guests.length}</span>{/if}
        </button>
        {#if moreOpen}
          <button class="scrim" aria-label="Close" onclick={() => (moreOpen = false)}></button>
          <div class="menu" role="menu" transition:pop={{ origin: "bottom right", y: 8 }}>
            <button role="menuitem" class:on={share.sharing || share.inRoom} onclick={() => ((moreOpen = false), toggleShare())}>
              <Icon name="cast" size={16} /> {shareTitle}
            </button>
            <button role="menuitem" class:on={lyricsShown} disabled={!track} onclick={() => ((moreOpen = false), toggleLyrics())}>
              <Icon name="lyrics" size={16} /> Lyrics
            </button>
            <button role="menuitem" onclick={() => ((moreOpen = false), api.miniPlayer(true))}>
              <Icon name="pip" size={16} /> Mini player
            </button>
          </div>
        {/if}
      </div>
    {:else}
      <button class="hw small" class:on={share.sharing || share.inRoom} title={shareTitle} aria-label="Shared control and party" onclick={toggleShare}>
        <Icon name="cast" size={16} />
        {#if share.sharing && share.hub.guests.length}<span class="count">{share.hub.guests.length}</span>{/if}
      </button>
      <button class="hw small" class:on={lyricsShown} title="Lyrics" aria-label="Lyrics" disabled={!track} onclick={toggleLyrics}>
        <Icon name="lyrics" size={16} />
      </button>
      <button class="hw small" title="Mini player (always on top)" aria-label="Mini player" onclick={() => api.miniPlayer(true)}>
        <Icon name="pip" size={16} />
      </button>
    {/if}
    <button class="hw small" class:on={queueOpen} title="Queue" aria-label="Queue" onclick={() => (queueOpen = !queueOpen)}>
      <Icon name="queue" size={16} />
    </button>
  </div>
</footer>

<style>
  .deck {
    grid-column: 1 / -1;
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(300px, 1.3fr) minmax(min-content, 1fr);
    align-items: center;
    gap: 20px;
    height: 100px;
    padding: 0 22px;
    background: linear-gradient(180deg, color-mix(in srgb, var(--surface) 82%, transparent), var(--surface));
    border-top: 1px solid var(--glass-border);
    backdrop-filter: blur(24px) saturate(1.3);
    z-index: 4;
  }
  .left {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .now {
    flex: 0 1 auto;
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
    max-width: 100%;
    text-align: left;
    border-radius: min(12px, var(--r-max));
  }
  /* the title column: nothing in it (the format / EQ chip above all) may
     paint over the seek bar and its times, whatever the engine's sizing */
  .meta {
    min-width: 0;
    overflow: hidden;
  }
  .title {
    font-weight: 700;
  }
  .artist {
    font-size: 12.5px;
    color: var(--muted);
  }
  .chip {
    display: flex;
    align-items: center;
    gap: 6px;
    width: fit-content;
    max-width: 100%;
    box-sizing: border-box;
    overflow: hidden;
    white-space: nowrap;
    margin-top: 4px;
    padding: 2px 8px;
    border-radius: min(999px, var(--r-max));
    background: var(--glass-strong);
    font: 600 10.5px var(--font-mono);
    color: var(--muted);
  }
  .chip.from {
    font-family: inherit;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--accent-text);
  }
  .count {
    position: absolute;
    top: -4px;
    right: -4px;
    min-width: 16px;
    height: 16px;
    padding: 0 4px;
    border-radius: min(8px, var(--r-max));
    background: var(--primary-strong);
    color: var(--on-primary-strong);
    font-size: 10px;
    font-weight: 700;
    line-height: 16px;
  }
  .led {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--ok);
    box-shadow: 0 0 6px var(--ok);
  }
  /* when the column is short the EQ name gives way first ("EQ · Senn…"),
     the format and the loudness keep their text */
  .fmt,
  .eq-tag.loud,
  .led {
    flex: none;
  }
  .eq-tag {
    flex: 0 1 auto;
    min-width: 2.6em;
    max-width: 200px;
    white-space: nowrap;
    padding: 0 5px;
    border-radius: min(4px, var(--r-max));
    background: color-mix(in srgb, #fbbf24 22%, transparent);
    color: #d97706;
    font-weight: 700;
  }
  .eq-btn.active {
    box-shadow: inset 0 0 0 1px var(--accent-text), 0 2px 0 rgb(0 0 0 / 0.35);
  }
  .eq-dot {
    position: absolute;
    top: 4px;
    right: 4px;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #fbbf24;
    box-shadow: 0 0 6px #fbbf24;
  }
  .led.warn {
    background: #fbbf24;
    box-shadow: 0 0 6px #fbbf24;
  }
  .center {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .transport {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  /* hardware-like buttons */
  .hw {
    width: 40px;
    height: 40px;
    display: grid;
    place-items: center;
    border-radius: min(12px, var(--r-max));
    color: var(--text);
    background: linear-gradient(180deg, var(--hw-top), var(--hw-bottom));
    box-shadow: inset 0 1px 0 rgb(255 255 255 / 0.12), 0 2px 0 rgb(0 0 0 / 0.35), 0 6px 14px rgb(0 0 0 / 0.25);
    transition: transform 0.08s, box-shadow 0.08s, color 0.15s;
  }
  .hw:active:not(:disabled) {
    transform: translateY(2px);
    box-shadow: inset 0 1px 0 rgb(255 255 255 / 0.08), 0 0 0 rgb(0 0 0 / 0.35);
  }
  .hw.small {
    position: relative;
    width: 34px;
    height: 34px;
    color: var(--muted);
  }
  .hw.on {
    color: var(--accent-text);
  }
  .hw.play {
    width: 54px;
    height: 54px;
    border-radius: 50%;
    color: #fff;
    background: radial-gradient(circle at 35% 30%, color-mix(in srgb, var(--room-accent) 90%, white), var(--room-glow));
    box-shadow: 0 0 24px color-mix(in srgb, var(--room-accent) 45%, transparent), inset 0 1px 0 rgb(255 255 255 / 0.3);
  }
  .seek {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .time {
    font: 500 11.5px var(--font-mono);
    color: var(--muted);
    width: 46px;
    text-align: center;
  }
  .right {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 12px;
    min-width: 0;
  }
  /* everything keeps its size; only the output name gets shorter */
  .right > :global(*) {
    flex: none;
  }
  .right > .output {
    flex: 0 1 auto;
  }
  .levels {
    display: flex;
    gap: 2px;
    align-items: flex-end;
    height: 22px;
  }
  .levels i {
    width: 4px;
    border-radius: min(2px, var(--r-max));
    background: var(--glass-border);
  }
  .levels i:nth-child(n) {
    height: calc(30% + var(--h, 0%));
  }
  .levels i:nth-child(1) { --h: 0%; }
  .levels i:nth-child(2) { --h: 10%; }
  .levels i:nth-child(3) { --h: 20%; }
  .levels i:nth-child(4) { --h: 30%; }
  .levels i:nth-child(5) { --h: 40%; }
  .levels i:nth-child(6) { --h: 50%; }
  .levels i:nth-child(7) { --h: 60%; }
  .levels i:nth-child(8) { --h: 70%; }
  .levels i.lit {
    background: var(--room-accent);
  }
  .levels i.lit.hot {
    background: #ef4444;
  }
  .output {
    position: relative;
    min-width: 0;
  }
  .out-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    max-width: min(170px, 100%);
    min-width: 34px;
    height: 34px;
    padding: 0 12px;
    border-radius: min(999px, var(--r-max));
    background: var(--glass-strong);
    border: 1px solid var(--glass-border);
    color: var(--text);
    font-size: 12.5px;
  }
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 10;
    cursor: default;
  }
  .more {
    position: relative;
  }
  .menu {
    /* an ordinary surface, whatever colours the skin gives the deck */
    --text: var(--window-text);
    --muted: var(--window-muted);
    --surface: var(--window-surface);
    --surface-2: var(--window-surface-2);
    --border: var(--window-border);
    --accent-text: var(--window-accent-text);
    color: var(--text);
    position: absolute;
    right: 0;
    bottom: calc(100% + 10px);
    z-index: 11;
    display: flex;
    flex-direction: column;
    min-width: 210px;
    padding: 6px;
    border-radius: min(14px, var(--r-max));
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: var(--shadow, 0 12px 32px rgb(0 0 0 / 0.35));
  }
  .menu button {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 36px;
    padding: 0 12px;
    border-radius: min(9px, var(--r-max));
    color: var(--text);
    font-size: 13px;
    text-align: left;
    white-space: nowrap;
  }
  .menu button:hover:not(:disabled) {
    background: var(--surface-2);
  }
  .menu button.on {
    color: var(--accent-text);
  }
  .menu button:disabled {
    opacity: 0.45;
  }
  .out-btn :global(svg) {
    flex: none;
  }
  /* the right side gives up space in steps as the window narrows */
  @media (max-width: 1280px) {
    .levels {
      display: none;
    }
  }
  @media (max-width: 1120px) {
    .deck {
      gap: 14px;
      padding: 0 16px;
    }
    .right {
      gap: 8px;
    }
    .out-btn {
      width: 34px;
      padding: 0;
      justify-content: center;
    }
    .out-btn span {
      display: none;
    }
  }
  /* a shorter title column: the loudness tag goes first, then the layer
     names and the codec ("24/96 · EQ") */
  @media (max-width: 1280px) {
    .eq-tag.loud {
      display: none;
    }
  }
  @media (max-width: 1040px) {
    .eq-tag .layers,
    .codec {
      display: none;
    }
    .eq-tag {
      min-width: 0;
    }
  }
  @media (max-width: 920px) {
    .deck {
      grid-template-columns: minmax(0, 1fr) minmax(260px, 1.4fr) auto;
    }
    .chip {
      display: none;
    }
    .transport {
      gap: 6px;
    }
  }
  /* crossfade in progress: the tag fills as the next song takes over */
  .eq-tag.fade {
    background: linear-gradient(90deg, color-mix(in srgb, var(--primary) 30%, transparent) calc(var(--p, 0) * 100%), transparent 0);
  }
</style>
