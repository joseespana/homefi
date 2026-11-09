<script lang="ts">
  // One deck: the song, its waveforms, Cue / Play / Sync, hot cues, loops and the tempo strip
  // (on the outer edge: deck B is mirrored). Simple view hides hot cues, loops and the tempo strip.
  import { formatTime } from "../../lib/api";
  import { dj } from "../../lib/dj.svelte";
  import { deckIndex, DJ_DRAWER_WIDTH_PX, DJ_HOT_CUES, DJ_TEMPO_RANGES, type DeckId } from "../../lib/dj-api";
  import { HOT_CUE_COLORS } from "../../lib/djColors.svelte";
  import Icon from "../Icon.svelte";
  import DjFader from "./DjFader.svelte";
  import DjWaveOverview from "./DjWaveOverview.svelte";
  import DjWaveZoom from "./DjWaveZoom.svelte";

  let { deck }: { deck: DeckId } = $props();

  const LOOP_BEATS = [1, 2, 4, 8] as const;
  const BPM_DECIMALS = 1;
  const PERCENT_DECIMALS = 1;

  const i = $derived(deckIndex(deck));
  const letter = $derived(deck.toUpperCase());
  const s = $derived(dj.deck(deck));
  const track = $derived(s?.track ?? null);
  const meta = $derived(dj.meta[i]);
  const phase = $derived(dj.phase(deck));
  const ready = $derived(phase === "ready");
  const simple = $derived(dj.settings.simple_view);
  const range = $derived(dj.settings.tempo_range_pct);
  const hasGrid = $derived(!!track?.has_grid);
  const linked = $derived(dj.linked);
  const bpm = $derived(track ? (s && s.current_bpm > 0 ? s.current_bpm : track.bpm) : 0);
  const halfTime = $derived(!!track && !!s?.synced && track.bpm > 0 && Math.abs(s.current_bpm / track.bpm - 1) > 0.5);
  const tempoText = (p: number) => `${p > 0 ? "+" : p < 0 ? "−" : "±"}${Math.abs(p).toFixed(PERCENT_DECIMALS)}%`;
  const keyHint = (action: string) => {
    const k = dj.settings.keys_enabled ? dj.keyFor(`deck_${deck}.${action}`) : null;
    return k ? ` [${k}]` : "";
  };
  const needsSong = "Load a song first";

  let dropping = $state(false);
  function drop(e: DragEvent) {
    e.preventDefault();
    dropping = false;
    if (dj.dragging) dj.loadCrate(deck, dj.dragging);
    dj.dragging = null;
  }

  function hotCue(e: MouseEvent, n: number) {
    if (!ready || !s) return;
    const set = s.hot_cues[n] !== null;
    if (set && (e.shiftKey || e.type === "contextmenu")) {
      e.preventDefault();
      dj.send({ type: "clear_hot_cue", deck, index: n });
    } else if (e.type === "click") {
      dj.send({ type: set ? "hot_cue" : "set_hot_cue", deck, index: n });
    }
  }
  const loopBeats = $derived(s?.loop_ms && track && track.beat_ms > 0 ? Math.round((s.loop_ms[1] - s.loop_ms[0]) / track.beat_ms) : 0);

  function cycleRange() {
    const at = DJ_TEMPO_RANGES.indexOf(range as (typeof DJ_TEMPO_RANGES)[number]);
    dj.saveSettings({ tempo_range_pct: DJ_TEMPO_RANGES[(at + 1) % DJ_TEMPO_RANGES.length] });
  }
  function bend(amount: number) {
    if (ready) dj.send({ type: "bend", deck, amount });
  }
</script>

<article
  class="deck {deck}"
  class:simple
  class:dropping
  class:playing={!!s?.playing}
  aria-label="Deck {letter}"
  ondragover={(e) => {
    if (!dj.dragging) return;
    e.preventDefault();
    dropping = true;
  }}
  ondragleave={() => (dropping = false)}
  ondrop={drop}
>
  <div class="dh">
    <div class="letter" aria-hidden="true">{letter}</div>
    {#if phase === "fetching" || phase === "loading"}
      <div class="cover skel"></div>
      <div class="meta" aria-busy="true"><span class="skel-line"></span><span class="skel-line short"></span></div>
    {:else}
      <div class="cover" role="img" aria-label={meta ? `Cover of ${meta.title}` : "No song"}>
        {#if meta?.cover && track}<img src={meta.cover} alt="" />{/if}
      </div>
      <div class="meta">
        <div class="t">{track ? (meta?.title ?? track.title) : "No song"}</div>
        <div class="a">{track ? (meta?.artist ?? "") : `Deck ${letter}`}</div>
      </div>
    {/if}
    <div class="bpm" title={track && !hasGrid ? "No beat found in this song" : "Beats per minute"}>
      <div class="n">{bpm > 0 ? bpm.toFixed(BPM_DECIMALS) : "--.-"}</div>
      <div class="u">BPM</div>
    </div>
    {#if ready}
      <button class="eject icon-btn" type="button" aria-label="Unload deck {letter}" title="Unload this song" onclick={() => dj.eject(deck)}><Icon name="eject" size={16} /></button>
    {/if}
  </div>

  <div class="inner">
    <div class="main">
      <div class="tm">
        <span>{formatTime(s?.position_ms ?? 0)}</span>
        <span>−{formatTime(Math.max(0, (track?.duration_ms ?? 0) - (s?.position_ms ?? 0)))}</span>
        <span class="sp"></span>
        {#if ready && linked}<span class="sync" title={halfTime ? "Matched at half speed" : "Following the other deck's tempo"}>{halfTime ? "HALF SPEED" : "SYNCED"}</span>{/if}
        {#if ready}<span>{tempoText(s?.tempo_percent ?? 0)}</span>{/if}
      </div>

      <div class="waves">
        {#if phase === "empty" || dropping}
          <div class="empty-d" class:over={dropping}>
            <b>{dropping ? `Drop to load on deck ${letter}` : "Drag a song here"}</b>
            {#if !dropping}<span>or press {letter} on a song in the crate</span>{/if}
          </div>
        {:else if phase === "fetching" || phase === "loading"}
          <div class="pending" role="status">
            <span>{phase === "fetching" ? "Getting the song ready…" : "Finding the beat…"}</span>
            <i aria-hidden="true"></i>
          </div>
        {:else if phase === "error"}
          <div class="err" role="alert">
            <Icon name="alert" size={18} />
            <p>{s?.error ? `${s.error.charAt(0).toUpperCase()}${s.error.slice(1)}.` : "This song could not be opened."}</p>
            <div class="err-actions">
              {#if meta?.source}<button class="btn tonal" type="button" onclick={() => dj.retry(deck)}>Try again</button>{/if}
              <button class="btn tonal" type="button" onclick={() => dj.showCrate(window.innerWidth < DJ_DRAWER_WIDTH_PX)}>Choose another</button>
            </div>
          </div>
        {:else}
          <DjWaveZoom {deck} />
          <DjWaveOverview {deck} />
        {/if}
      </div>

      <div class="tp" role="group" aria-label="Transport, deck {letter}">
        <button
          class="big"
          type="button"
          disabled={!ready}
          title={ready ? `Go back to the start point. Hold it to listen from there.${keyHint("cue")}` : needsSong}
          onpointerdown={(e) => {
            if (!ready || e.button !== 0) return;
            (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
            dj.send({ type: "cue", deck, pressed: true });
          }}
          onpointerup={() => ready && dj.send({ type: "cue", deck, pressed: false })}
          onkeydown={(e) => (e.key === " " || e.key === "Enter") && !e.repeat && (e.preventDefault(), dj.send({ type: "cue", deck, pressed: true }))}
          onkeyup={(e) => (e.key === " " || e.key === "Enter") && dj.send({ type: "cue", deck, pressed: false })}>CUE{#if keyHint("cue")}<small>{dj.keyFor(`deck_${deck}.cue`)}</small>{/if}</button
        >
        <button class="big play" type="button" disabled={!ready} aria-label={s?.playing ? `Pause deck ${letter}` : `Play deck ${letter}`} aria-pressed={!!s?.playing} title={ready ? `Play or pause this deck${keyHint("play")}` : needsSong} onclick={() => dj.send({ type: "toggle_play", deck })}>
          <Icon name={s?.playing ? "pause" : "play"} size={16} /><span class="word">{s?.playing ? "PAUSE" : "PLAY"}</span>{#if keyHint("play")}<small>{dj.keyFor(`deck_${deck}.play`)}</small>{/if}
        </button>
        <button
          class="big sync"
          class:on={ready && linked}
          type="button"
          disabled={!ready || !hasGrid}
          aria-pressed={ready && linked}
          title={!ready ? needsSong : !hasGrid ? "No beat found in this song, so it can't be synced" : `Match this song's speed and beat to the other deck${keyHint("sync")}`}
          onclick={() => dj.toggleSync(deck)}>SYNC{#if keyHint("sync")}<small>{dj.keyFor(`deck_${deck}.sync`)}</small>{/if}</button
        >
      </div>

      {#if !simple}
        <div class="pro">
          <span class="cap-t">Hot cues</span>
          <div class="pads" role="group" aria-label="Hot cues, deck {letter}">
            {#each { length: DJ_HOT_CUES } as _, n (n)}
              {@const set = s?.hot_cues[n] != null}
              <button
                class="hc"
                class:set
                type="button"
                style:--h={HOT_CUE_COLORS[n]}
                disabled={!ready}
                aria-label="Hot cue {n + 1}, deck {letter}{set ? '' : ', empty'}"
                title={ready ? (set ? "Jump to this saved spot. Shift-click or right-click to delete it." : "Empty: click to save the current spot.") : needsSong}
                onclick={(e) => hotCue(e, n)}
                oncontextmenu={(e) => hotCue(e, n)}><i>{n + 1}</i>{set ? "" : "+"}</button
              >
            {/each}
          </div>
          <div class="loops" role="group" aria-label="Loops, deck {letter}">
            <span class="lbl">Loop</span>
            {#each LOOP_BEATS as beats (beats)}
              <button class="btn lp" class:on={loopBeats === beats} type="button" disabled={!ready || !hasGrid} aria-pressed={loopBeats === beats} title={hasGrid ? `Repeat the next ${beats} beat${beats === 1 ? "" : "s"}` : "Loops need the beat"} onclick={() => dj.loop(deck, beats)}>{beats}</button>
            {/each}
            <button class="btn lp" type="button" disabled={!ready} title="Set the loop start" onclick={() => dj.send({ type: "loop_in", deck })}>In</button>
            <button class="btn lp" type="button" disabled={!ready} title="Set the loop end" onclick={() => dj.send({ type: "loop_out", deck })}>Out</button>
            <button class="btn lp" type="button" disabled={!ready || !s?.loop_ms} title="Leave the loop" onclick={() => dj.send({ type: "loop_exit", deck })}>Exit</button>
          </div>
        </div>
      {/if}
    </div>

    {#if !simple}
      <div class="side">
        <span class="cap-t">Tempo</span>
        <DjFader
          value={s?.tempo_percent ?? 0}
          min={-range}
          max={range}
          reset={0}
          zero={0}
          centerLine
          label="Tempo, deck {letter}"
          tip="Speed of this song. Up = faster and higher pitch. Double-click to reset."
          valueText={tempoText}
          onchange={(percent) => dj.send({ type: "tempo", deck, percent })}
        />
        <span class="pct">{tempoText(s?.tempo_percent ?? 0)}</span>
        <div class="mini">
          <button class="btn" type="button" aria-label="Slow down while held, deck {letter}" title="Hold to slow down a little, to line up beats" disabled={!ready} onpointerdown={() => bend(-1)} onpointerup={() => bend(0)} onpointerleave={() => s?.bend && bend(0)}>−</button>
          <button class="btn" type="button" aria-label="Speed up while held, deck {letter}" title="Hold to speed up a little, to line up beats" disabled={!ready} onpointerdown={() => bend(1)} onpointerup={() => bend(0)} onpointerleave={() => s?.bend && bend(0)}>+</button>
        </div>
        <div class="mini">
          <button class="btn" type="button" title="Reset the tempo to 0 (double-click the fader too)" onclick={() => dj.send({ type: "tempo", deck, percent: 0 })}>0</button>
          <button class="btn" type="button" title="Tempo range: 8 %, 16 % or 50 %" aria-label="Tempo range ±{range} %" onclick={cycleRange}>±{range}</button>
        </div>
      </div>
    {/if}
  </div>
</article>

<style>
  .deck {
    --c: var(--dj-a);
    --ci: var(--dj-a-ink);
    position: relative;
    container: deck / inline-size;
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
    min-height: 0;
    padding: 12px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
    box-shadow: inset 0 3px 0 var(--c);
    transition: background calc(0.12s * var(--motion-speed, 1));
  }
  .deck.b {
    --c: var(--dj-b);
    --ci: var(--dj-b-ink);
  }
  .deck.dropping {
    border: 2px dashed var(--c);
    background: color-mix(in srgb, var(--c) 8%, var(--surface));
  }
  .dh {
    display: flex;
    gap: 12px;
    align-items: center;
  }
  .letter {
    flex: none;
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border-radius: var(--r-sm);
    background: var(--c);
    color: var(--ci);
    font-weight: 800;
    font-size: 18px;
  }
  .cover {
    flex: none;
    width: 64px;
    height: 64px;
    border-radius: var(--r-sm);
    overflow: hidden;
    background: linear-gradient(135deg, color-mix(in srgb, var(--c) 70%, var(--surface-3)), var(--surface-3));
    box-shadow: var(--shadow);
  }
  .cover.skel {
    background: var(--surface-3);
    box-shadow: none;
  }
  .skel-line {
    display: block;
    height: 12px;
    width: 80%;
    margin: 4px 0;
    border-radius: var(--r-xs);
    background: var(--surface-3);
  }
  .skel-line.short {
    width: 50%;
  }
  .playing .cover {
    outline: 2px solid var(--c);
    outline-offset: 2px;
  }
  .cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .meta {
    flex: 1;
    min-width: 0;
  }
  .t {
    font-weight: 700;
    font-size: 15px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .a {
    color: var(--muted);
    font-size: 13px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .bpm {
    flex: none;
    text-align: right;
    font-family: var(--font-mono);
  }
  .bpm .n {
    font-size: 26px;
    font-weight: 700;
    line-height: 1;
  }
  .bpm .u {
    font-size: 11px;
    color: var(--muted);
  }
  .eject {
    flex: none;
  }
  /* narrow windows: the song's name gets the room, the buttons share what's left */
  @container deck (max-width: 439px) {
    .tp .big {
      min-width: 0;
      gap: 4px;
      font-size: 13px;
      letter-spacing: 0;
    }
    .big small,
    .lbl,
    .word {
      display: none;
    }
    .tp {
      gap: 6px;
    }
    .pro .loops {
      gap: 4px;
    }
    .loops .lp {
      font-size: 12px;
    }
    .dh {
      gap: 8px;
    }
    .cover {
      width: 44px;
      height: 44px;
    }
    .letter {
      width: 30px;
      height: 30px;
      font-size: 15px;
    }
    .bpm .n {
      font-size: 20px;
    }
  }
  .inner {
    display: flex;
    gap: 10px;
    flex: 1;
    min-height: 0;
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .b .main {
    order: 2;
  }
  .b .side {
    order: 1;
  }
  .tm {
    display: flex;
    gap: 10px;
    align-items: center;
    min-height: 20px;
    font: 12px var(--font-mono);
    color: var(--muted);
  }
  .sp {
    flex: 1;
  }
  .sync {
    padding: 2px 8px;
    border-radius: var(--r-pill);
    background: var(--c);
    color: var(--ci);
    font-weight: 700;
    font-size: 11px;
  }
  .waves {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  /* simple view has fewer controls: the song's waveform takes the room they leave */
  .simple .waves {
    flex: 1;
    min-height: 0;
  }
  .simple .waves > :global(.wz) {
    flex: 1;
    height: auto;
    min-height: var(--dj-deck-h);
  }
  .empty-d,
  .pending,
  .err {
    height: calc(var(--dj-deck-h) + 54px);
    border-radius: var(--r-sm);
  }
  .empty-d {
    display: grid;
    place-content: center;
    gap: 4px;
    padding: 8px;
    text-align: center;
    color: var(--muted);
    border: 2px dashed var(--border);
  }
  .empty-d.over {
    border-color: var(--c);
    color: var(--text);
  }
  .empty-d span {
    font-size: 12px;
    color: var(--faint);
  }
  .pending {
    position: relative;
    display: grid;
    place-items: center;
    overflow: hidden;
    color: var(--muted);
    font-size: 13px;
    background: color-mix(in srgb, var(--dj-wave-bg) 78%, transparent);
    border: 1px solid var(--border);
  }
  .pending i {
    position: absolute;
    left: 0;
    bottom: 0;
    width: 40%;
    height: 3px;
    background: var(--c);
    animation: sweep calc(1.4s * var(--motion-speed, 1)) var(--ease-out, ease-in-out) infinite;
  }
  @keyframes sweep {
    from {
      left: -40%;
    }
    to {
      left: 100%;
    }
  }
  :global(:root:is([data-motion="subtle"], [data-motion="off"])) .pending i {
    animation: none;
    left: 0;
    width: 100%;
    opacity: 0.6;
  }
  .err {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 10px;
    text-align: center;
    color: var(--danger);
    border: 1px solid color-mix(in srgb, var(--danger) 45%, var(--border));
    background: color-mix(in srgb, var(--danger) 8%, var(--surface));
  }
  .err p {
    margin: 0;
    color: var(--text);
    font-size: 13px;
  }
  .err-actions {
    display: flex;
    gap: 8px;
  }
  .tp {
    display: flex;
    gap: 10px;
  }
  .big {
    flex: 1;
    min-width: 84px;
    height: 56px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    border-radius: var(--r-md);
    border: 1px solid var(--border);
    background: linear-gradient(var(--hw-top), var(--hw-bottom));
    font-weight: 700;
    font-size: 15px;
    letter-spacing: 0.04em;
  }
  .big:hover:not(:disabled) {
    border-color: var(--c);
  }
  .big:disabled {
    opacity: 1;
    color: var(--faint);
  }
  .big:disabled :global(svg) {
    opacity: 0.45;
  }
  .big.play {
    flex: 1.4;
    background: var(--c);
    color: var(--ci);
    border-color: transparent;
  }
  .big.play:disabled {
    background: color-mix(in srgb, var(--c) 30%, var(--surface-2));
    color: var(--faint);
  }
  .big.sync.on {
    color: var(--c);
    box-shadow: inset 0 0 0 2px var(--c);
  }
  .big :global(svg) {
    flex: none;
  }
  .big small {
    font-weight: 500;
    font-size: 10px;
    opacity: 0.7;
    letter-spacing: 0;
  }
  .pro {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .cap-t {
    font-size: 10px;
    color: var(--faint);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .pads {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 8px;
  }
  .hc {
    position: relative;
    height: var(--dj-hit);
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--r-sm);
    border: 1px dashed var(--border);
    background: var(--surface-2);
    color: var(--faint);
    font-weight: 700;
  }
  .hc i {
    position: absolute;
    top: 3px;
    left: 7px;
    font-style: normal;
    font-weight: 500;
    font-size: 11px;
    color: var(--faint);
  }
  .hc.set {
    border-style: solid;
    border-color: var(--h);
    background: color-mix(in srgb, var(--h) 26%, var(--surface-2));
    color: var(--text);
  }
  .hc.set::after {
    content: "";
    position: absolute;
    left: 8px;
    right: 8px;
    bottom: 5px;
    height: 3px;
    border-radius: var(--r-xs);
    background: var(--h);
  }
  .hc:disabled {
    opacity: 0.45;
  }
  .loops {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .lbl {
    flex: none;
    width: 34px;
    font-size: 11px;
    color: var(--muted);
  }
  .lp {
    flex: 1;
    min-width: 0;
    height: 44px;
    padding: 0 4px;
    justify-content: center;
    border-radius: var(--r-sm);
    border: 1px solid var(--border);
    background: var(--surface-2);
    font-weight: 700;
  }
  .lp.on {
    background: var(--c);
    color: var(--ci);
    border-color: transparent;
  }
  .side {
    flex: none;
    width: 64px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
  }
  .pct {
    font: 12px var(--font-mono);
  }
  .mini {
    display: flex;
    gap: 4px;
    width: 100%;
  }
  .mini .btn {
    flex: 1;
    min-width: 0;
    height: 40px;
    padding: 0;
    justify-content: center;
    border-radius: var(--r-sm);
    border: 1px solid var(--border);
    background: var(--surface-2);
    font-size: 12px;
  }
</style>
