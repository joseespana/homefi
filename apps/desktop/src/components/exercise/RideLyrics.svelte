<script lang="ts">
  // Karaoke in the ride, where the mountain goes: the line being sung fills in the part's colour (--c), the one
  // before fades above, the next waits below. The ride plays in the app's player, so the song, its lyrics (the
  // same store as the room's, nothing fetched twice) and the lyrics shift are the player's own; this only draws.
  import { untrack } from "svelte";
  import { karaokeAt, lyrics } from "../../lib/lyrics.svelte";
  import { useSongClock } from "../../lib/songClock.svelte";
  import { useSource } from "../../lib/source.svelte";
  import { app } from "../../lib/state.svelte";

  /** Lines longer than this many characters use the smaller type, so two rows hold them. */
  const LONG_LINE = 48;

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
      ? "Looking for the lyrics…"
      : entry.status === "error"
        ? "Lyrics unavailable right now"
        : entry.status === "none"
          ? "No lyrics for this song"
          : !entry.lyrics.synced
            ? "These lyrics aren’t synced, so karaoke can’t follow them"
            : null,
  );
  const textOf = (i: number | undefined) => (data && i !== undefined && i >= 0 ? data.lines[i].text : "");
  const current = $derived(frame ? textOf(frame.current) : "");
  const previous = $derived(frame ? textOf(frame.previous) : "");
  const next = $derived(frame && frame.next !== frame.current ? textOf(frame.next) : "");
</script>

<div class="karaoke" aria-label="Lyrics" aria-live="off">
  {#if message || !frame}
    <p class="note">{message}</p>
  {:else}
    <p class="prev ellipsis">{previous}</p>
    {#if frame.current >= 0}
      {#key frame.current}
        <p class="sung" class:long={current.length > LONG_LINE}>
          <span class="fill" style:--f="{(frame.fill * 100).toFixed(2)}%">{current}</span>
        </p>
      {/key}
    {:else}
      <p class="notes" aria-label="Instrumental">♪ ♪ ♪</p>
    {/if}
    <p class="next ellipsis">{next}</p>
  {/if}
</div>

<style>
  .karaoke {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
    gap: 14px;
    padding: 44px 24px 12px;
    text-align: center;
    min-width: 0;
  }
  p {
    margin: 0;
    max-width: 100%;
  }
  .prev,
  .next {
    min-height: 1.3em;
    font-size: clamp(14px, 1.6vw, 20px);
    font-weight: 600;
    color: var(--muted);
  }
  .prev {
    opacity: 0.55;
  }
  .sung {
    font-size: clamp(24px, 3.4vw, 46px);
    font-weight: 800;
    line-height: 1.18;
    letter-spacing: -0.01em;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
    animation: rise 0.28s ease-out;
  }
  .sung.long {
    font-size: clamp(20px, 2.6vw, 34px);
  }
  .fill {
    background-image: linear-gradient(90deg, var(--c) 0 var(--f), color-mix(in srgb, var(--text) 40%, transparent) var(--f) 100%);
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }
  .notes {
    font-size: clamp(24px, 3.4vw, 46px);
    letter-spacing: 0.25em;
    color: var(--c);
  }
  .note {
    font-size: 15px;
    color: var(--muted);
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(10px);
    }
  }
  :global(:root[data-motion="off"]) .sung {
    animation: none;
  }
</style>
