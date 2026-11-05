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
  import { pop, swap } from "../lib/motion";
  import { buttonsOnTop } from "../lib/playerButtons.svelte";
  import { layersWords, soundName, sound } from "../lib/sound.svelte";
  import HowSounds from "./sound/HowSounds.svelte";
  import PlayerButtons from "./PlayerButtons.svelte";

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

  // narrow windows: a smaller knob (and, with the buttons in the player bar, a "more" menu)
  let narrow = $state(false);
  $effect(() => {
    const mq = matchMedia("(max-width: 920px)");
    const update = () => (narrow = mq.matches);
    update();
    mq.addEventListener("change", update);
    return () => mq.removeEventListener("change", update);
  });
  const buttonsTop = $derived(buttonsOnTop());

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
  // "HD 650 + Warm and airy · Eagles": the output's profile and the sound playing
  const layers = $derived(p?.eq_active ? layersWords(p) : null);
  // the small tag under the song: the sound's name (or just "Sound"), the whole sentence on hover
  const soundTag = $derived(sound.ui.layer_chip ? soundName(p) : "Sound");
  const soundTagTitle = $derived(
    `${p?.auditioning ? "Trying" : "Sound on"}: ${layers ?? "on"}. Playback is not bit-perfect.\nOpen the room to see the signal chain.`,
  );
  const showHow = $derived(sound.ui.react_deck && !src.remote);
  const showSound = $derived(app.settings?.eq_ui?.deck_button ?? true);

  // the one Sound button: what it says depends on the master switch, the output and this song
  const soundState = $derived(src.remote ? "remote" : p?.eq_active ? (p.auditioning ? "trying" : "on") : app.settings?.eq.enabled ? "flat" : "off");
  const soundButtonTitle = $derived(
    {
      remote: "Sound (this computer)",
      trying: `Trying: ${soundName(p)}`,
      on: `Sound: ${layers ?? "on"} (not bit-perfect)`,
      flat: "Sound: on, flat for this output",
      off: "Sound: off (bit-perfect)",
    }[soundState],
  );
  const soundOpen = $derived(app.view.name === "sound");
  let soundMenuOpen = $state(false);
  const toggleSound = () => (app.view = soundOpen ? { name: "room" } : { name: "sound" });
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
              {#if p.eq_active}<span class="eq-tag sound-tag" class:trying={p.auditioning} title={soundTagTitle}><Icon name="eq" size={10} /><span class="name ellipsis">{p.auditioning ? `Trying · ${soundTag}` : soundTag}</span></span>{/if}
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
        <button class="deck-scrim" aria-label="Close" onclick={() => (outputsOpen = false)}></button>
        <OutputMenu onclose={() => (outputsOpen = false)} />
      {/if}
    </div>
    {#if buttonsTop && (showHow || showSound)}<i class="sep" aria-hidden="true"></i>{/if}
    {#if showHow}
      <HowSounds place="deck" look="hw" up />
    {/if}
    {#if showSound}
      <div class="sound-wrap">
        <button
          class="hw small snd {soundState}"
          class:on={soundState === "on" || soundState === "trying"}
          class:active={soundOpen}
          title="{soundButtonTitle}\nSound (A: ask the AI)"
          aria-label="Sound"
          aria-pressed={soundOpen}
          aria-haspopup="menu"
          aria-keyshortcuts="A"
          onclick={toggleSound}
          oncontextmenu={(e) => (e.preventDefault(), (soundMenuOpen = true))}
        >
          <Icon name="eq" size={16} />
          {#if soundState === "on" || soundState === "trying"}<span class="snd-dot" class:ring={soundState === "trying"} aria-hidden="true"></span>{/if}
          <span class="sr-only">{soundState === "off" ? "off" : soundState === "trying" ? "trying" : "on"}</span>
        </button>
        {#if soundMenuOpen}
          <button class="deck-scrim" aria-label="Close" onclick={() => (soundMenuOpen = false)}></button>
          <div class="deck-menu" role="menu" tabindex="-1" onkeydown={(e) => e.key === "Escape" && (soundMenuOpen = false)} transition:pop={{ origin: "bottom right", y: 8 }}>
            <button role="menuitem" onclick={() => ((soundMenuOpen = false), (app.view = { name: "sound" }))}><Icon name="eq" size={16} /> Open Sound</button>
            <button role="menuitem" onclick={() => ((soundMenuOpen = false), sound.openAsk())}><Icon name="ai" size={16} /> Ask the AI <kbd>A</kbd></button>
          </div>
        {/if}
      </div>
    {/if}
    {#if buttonsTop && (showHow || showSound)}<i class="sep" aria-hidden="true"></i>{/if}
    <Knob value={volume} onchange={setVolume} size={narrow ? 40 : 48} />
    {#if !buttonsTop}
      <PlayerButtons look="hw" {narrow} bind:queueOpen />
    {/if}
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
    border-radius: var(--r-sm);
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
    border-radius: var(--r-pill);
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
  .led {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--ok);
    box-shadow: 0 0 6px var(--ok);
  }
  /* when the column is short the sound's name gives way first (down to its icon),
     the format and the loudness keep their text */
  .fmt,
  .eq-tag.loud,
  .led {
    flex: none;
  }
  .eq-tag {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    flex: 0 1 auto;
    min-width: 22px;
    box-sizing: border-box;
    white-space: nowrap;
    padding: 0 7px;
    border-radius: var(--r-pill);
    background: color-mix(in srgb, var(--muted) 16%, transparent);
    color: var(--muted);
    font-weight: 600;
  }
  .eq-tag :global(svg) {
    flex: none;
  }
  .eq-tag.sound-tag {
    max-width: 130px;
    background: color-mix(in srgb, var(--accent-text) 14%, transparent);
    color: var(--accent-text);
  }
  .eq-tag.sound-tag.trying {
    background: transparent;
    outline: 1px dashed var(--accent-text);
  }
  .eq-tag .name {
    min-width: 0;
  }
  .snd.snd.flat {
    color: var(--text);
  }
  .snd.active {
    box-shadow: inset 0 0 0 1px var(--accent-text), 0 2px 0 rgb(0 0 0 / 0.35);
  }
  .snd:focus-visible {
    outline: 2px solid var(--accent-text);
    outline-offset: 2px;
  }
  .snd-dot {
    position: absolute;
    top: 4px;
    right: 4px;
    width: 6px;
    height: 6px;
    box-sizing: border-box;
    border-radius: 50%;
    background: var(--accent-text);
    box-shadow: 0 0 0 2px var(--hw-bottom);
  }
  .snd-dot.ring {
    border: 1.5px solid var(--accent-text);
    background: transparent;
  }
  .sound-wrap {
    position: relative;
  }
  /* a thin rule between the groups of the right side */
  .sep {
    width: 1px;
    height: 20px;
    background: var(--glass-border);
  }
  .led.warn {
    background: var(--warn);
    box-shadow: 0 0 6px var(--warn);
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
    gap: 10px;
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
    border-radius: var(--r-pill);
    background: var(--glass-strong);
    border: 1px solid var(--glass-border);
    color: var(--text);
    font-size: 12.5px;
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
  /* a shorter title column: the loudness tag goes first, then the sound's
     name (its icon stays) and the codec */
  @media (max-width: 1280px) {
    .eq-tag.loud {
      display: none;
    }
  }
  @media (max-width: 1040px) {
    .eq-tag .name,
    .codec {
      display: none;
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
