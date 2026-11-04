<script lang="ts">
  // Karaoke in the mini player: the line being sung, filling with the accent as
  // it goes, and the next one under it. The same lyrics store and clock as the
  // room's LyricsView, in a strip that fits the small window.
  import { untrack } from "svelte";
  import { karaokeAt, lyrics } from "../lib/lyrics.svelte";
  import { useSongClock } from "../lib/songClock.svelte";
  import { useSource } from "../lib/source.svelte";
  import { app } from "../lib/state.svelte";

  /** Lines longer than this many characters use the smaller type, so two rows hold them. */
  const LONG_LINE = 54;

  const src = useSource();
  const offset = $derived(app.settings?.room.lyrics_offset_ms ?? 0);
  const clock = useSongClock(() => src.player);

  const req = $derived(lyrics.request(src));
  // the request is a new object on every player update: follow its key only
  const reqKey = $derived(req?.key ?? null);
  $effect(() => {
    reqKey;
    const r = untrack(() => req);
    if (r) lyrics.load(r);
  });
  const entry = $derived(lyrics.entry(req));
  const data = $derived(entry?.status === "ready" && entry.lyrics.synced ? entry.lyrics : null);
  const frame = $derived(data ? karaokeAt(data.lines, clock.now - offset) : null);

  const message = $derived(
    !entry || entry.status === "loading"
      ? "Loading the lyrics…"
      : entry.status === "error"
        ? "Lyrics unavailable right now"
        : entry.status === "none"
          ? "No lyrics for this song"
          : !entry.lyrics.synced
            ? "These lyrics aren’t synced"
            : null,
  );
  const current = $derived(data && frame && frame.current >= 0 ? data.lines[frame.current].text : "");
  const next = $derived(data && frame && frame.next >= 0 && frame.next !== frame.current ? data.lines[frame.next].text : "");
</script>

<div class="lyrics" aria-label="Lyrics" aria-live="off">
  {#if message || !frame}
    <p class="note">{message}</p>
  {:else if frame.current >= 0}
    <p class="sung" class:long={current.length > LONG_LINE}>
      <span class="fill" style:--f="{(frame.fill * 100).toFixed(2)}%">{current}</span>
    </p>
    <p class="next ellipsis">{next}</p>
  {:else}
    <p class="notes" aria-label="Instrumental">♪ ♪ ♪</p>
    <p class="next ellipsis">{next}</p>
  {/if}
</div>

<style>
  .lyrics {
    position: relative;
    flex: none;
    height: var(--mini-lyrics-h, 76px);
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
    gap: 4px;
    min-width: 0;
    text-align: center;
    border-top: 1px solid var(--m-groove);
  }
  .sung {
    margin: 0;
    max-width: 100%;
    font-size: 18px;
    font-weight: 800;
    line-height: 1.2;
    letter-spacing: -0.01em;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
  }
  .sung.long {
    font-size: 14.5px;
  }
  .fill {
    background-image: linear-gradient(90deg, var(--ly-fill, var(--room-accent)) 0 var(--f), color-mix(in srgb, var(--m-text) 38%, transparent) var(--f) 100%);
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }
  .next {
    margin: 0;
    max-width: 100%;
    font-size: 12px;
    font-weight: 600;
    color: var(--m-muted);
  }
  .notes {
    margin: 0;
    font-size: 18px;
    letter-spacing: 0.25em;
    color: var(--room-accent);
  }
  .note {
    margin: 0;
    font-size: 12px;
    color: var(--m-muted);
  }
  :global(:root[data-skin="retro_amp"]) .fill {
    background-image: linear-gradient(90deg, #00ff00 0 var(--f), #0a6a0a var(--f) 100%);
  }
</style>
