<script lang="ts">
  // The iPod's Cover Flow, as in the Android Aluminum layout: every album of the
  // library in a row, the one in front face-on with its reflection, the rest turned
  // away. Dragging, the trackpad and ←/→ only browse (the music never changes);
  // clicking a side album brings it forward; clicking the one in front (or Enter)
  // turns it over to its songs. Sized from the space it gets, centred, at any size.
  import { untrack } from "svelte";
  import { api, formatTime, type Track } from "../../lib/api";
  import { albumKey, app, groupAlbums } from "../../lib/state.svelte";
  import Cover from "../Cover.svelte";
  import Icon from "../Icon.svelte";

  interface FlowAlbum {
    key: string;
    title: string;
    artist: string;
    /** File whose artwork is shown. */
    cover: string | null;
    tracks: Track[];
  }

  const REFLECTION = 0.35;
  const CAPTION = 52;
  /** Browsing pauses following the album playing for this long. */
  const FOLLOW_AFTER_MS = 8000;
  /** Compose's Spring.StiffnessLow. */
  const STIFFNESS_LOW = 200;

  const current = $derived(app.current);
  const lower = (s: string) => s.toLowerCase();

  // the album playing: by file, else by name (a stream, a file opened from elsewhere...)
  const playingKey = $derived.by(() => {
    const c = current;
    if (!c) return null;
    const artist = lower(c.albumArtist || c.artist);
    const found =
      app.byPath.get(c.path) ??
      (c.album ? app.tracks.find((t) => lower(t.album) === lower(c.album) && lower(t.albumArtist || t.artist) === artist) : undefined);
    return found ? albumKey(found) : `now\u0000${c.artist}\u0000${c.album}`;
  });

  const cmp = (a: string | number, b: string | number) => (a < b ? -1 : a > b ? 1 : 0);
  const library = $derived(
    groupAlbums(app.tracks)
      .sort((a, b) => cmp(lower(a.artist), lower(b.artist)) || cmp(a.year ?? 0, b.year ?? 0) || cmp(lower(a.title), lower(b.title)))
      .map((a): FlowAlbum => ({ key: a.key, title: a.title, artist: a.artist, cover: a.tracks[0]?.path ?? null, tracks: a.tracks })),
  );
  // a Tidal stream (or a file outside the library) gets its place in front of the others
  const albums = $derived.by((): FlowAlbum[] => {
    const c = current;
    if (!c || library.some((a) => a.key === playingKey)) return library;
    return [{ key: playingKey!, title: c.album || c.title, artist: c.artist, cover: c.path, tracks: [] }, ...library];
  });
  const last = $derived(albums.length - 1);
  const playingIndex = $derived(Math.max(0, albums.findIndex((a) => a.key === playingKey)));

  let pos = $state(0);
  let flip = $state(0); // 0 the cover, 180 its songs
  let opened = $state<string | null>(null);
  /** The album whose back is showing, kept while it turns over again. */
  let backKey = $state<string | null>(null);
  let dragging = false;
  let touchedAt = -Infinity;
  let started = false;

  const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));
  const front = $derived(clamp(Math.round(pos), 0, Math.max(0, last)));

  // ---------------------------------------------------------------- size
  let W = $state(0);
  let H = $state(0);
  const side = $derived(Math.max(40, Math.min(W * 0.52, (H - CAPTION) / (1 + REFLECTION + 0.12))));
  const spacing = $derived(side * 0.66); // centre to the first neighbour
  const step = $derived(side * 0.16); // between the ones further away, stacked
  const top = $derived((H - (side * (1 + REFLECTION) + CAPTION)) / 2);
  const visible = $derived(Math.floor((W / 2 - spacing) / step) + 3);
  const backW = $derived(Math.min(W - 24, side * 1.5));
  const backH = $derived(Math.min(H - 24, side * (1 + REFLECTION) + CAPTION));
  // the camera of the Android version (10 inches at 72 dpi), kept in proportion on big covers
  const perspective = $derived(Math.max(720, side * 3.6));

  // ---------------------------------------------------------------- motion
  let raf = 0;
  let target: number | null = null;

  function stop() {
    cancelAnimationFrame(raf);
    raf = 0;
    target = null;
  }

  /** A spring like Compose's: `damping` is the ratio, velocity in albums per second. */
  function springTo(to: number, velocity = 0, damping = 1, done?: () => void) {
    stop();
    target = to;
    let x = pos;
    let v = velocity;
    let lastT = performance.now();
    const c = 2 * damping * Math.sqrt(STIFFNESS_LOW);
    const frame = (t: number) => {
      let dt = Math.min(0.05, (t - lastT) / 1000);
      lastT = t;
      while (dt > 0) {
        const h = Math.min(dt, 1 / 240);
        v += (-STIFFNESS_LOW * (x - to) - c * v) * h;
        x += v * h;
        dt -= h;
      }
      if (Math.abs(x - to) < 0.001 && Math.abs(v) < 0.01) {
        pos = to;
        raf = 0;
        target = null;
        done?.();
        return;
      }
      pos = x;
      raf = requestAnimationFrame(frame);
    };
    raf = requestAnimationFrame(frame);
  }

  // FastOutSlowIn, Compose's default tween easing
  function easing(t: number): number {
    const [x1, y1, x2, y2] = [0.4, 0, 0.2, 1];
    let s = t;
    for (let i = 0; i < 8; i++) {
      const x = 3 * (1 - s) ** 2 * s * x1 + 3 * (1 - s) * s * s * x2 + s ** 3 - t;
      const dx = 3 * (1 - s) ** 2 * x1 + 6 * (1 - s) * s * (x2 - x1) + 3 * s * s * (1 - x2);
      if (Math.abs(x) < 1e-5 || dx === 0) break;
      s = clamp(s - x / dx, 0, 1);
    }
    return 3 * (1 - s) ** 2 * s * y1 + 3 * (1 - s) * s * s * y2 + s ** 3;
  }

  let flipRaf = 0;
  function turnTo(to: number) {
    cancelAnimationFrame(flipRaf);
    const from = flip;
    if (from === to) return;
    const t0 = performance.now();
    const frame = (t: number) => {
      const f = Math.min(1, (t - t0) / 450);
      flip = from + (to - from) * easing(f);
      if (f < 1) flipRaf = requestAnimationFrame(frame);
      else if (to === 0) backKey = null;
    };
    flipRaf = requestAnimationFrame(frame);
  }

  $effect(() => {
    const open = opened;
    untrack(() => {
      if (open) backKey = open;
      turnTo(open ? 180 : 0);
    });
  });

  // a new album started: glide to it, unless you are browsing
  $effect(() => {
    const to = playingIndex;
    albums.length;
    untrack(() => {
      if (!started) {
        started = true;
        pos = to;
      } else if (!dragging && opened == null && performance.now() - touchedAt > FOLLOW_AFTER_MS) {
        springTo(to);
      }
    });
  });

  $effect(() => () => {
    stop();
    cancelAnimationFrame(flipRaf);
    clearTimeout(wheelTimer);
  });

  // ---------------------------------------------------------------- input
  let root: HTMLDivElement;
  let drag: { id: number; x: number; y: number; lastX: number; moving: boolean; samples: [number, number][] } | null = null;
  let swallowClick = false;

  function pointerDown(e: PointerEvent) {
    if (e.button !== 0 || !albums.length) return;
    drag = { id: e.pointerId, x: e.clientX, y: e.clientY, lastX: e.clientX, moving: false, samples: [[e.timeStamp, e.clientX]] };
  }

  function pointerMove(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.id) return;
    if (!drag.moving) {
      const dx = Math.abs(e.clientX - drag.x);
      const dy = Math.abs(e.clientY - drag.y);
      if (dy > 8 && dy > dx) return void (drag = null); // a vertical gesture (the song list), not ours
      if (dx < 8) return;
      drag.moving = true;
      drag.lastX = e.clientX;
      dragging = true;
      opened = null;
      stop();
      root.setPointerCapture(e.pointerId);
    }
    pos = clamp(pos - (e.clientX - drag.lastX) / spacing, -0.4, last + 0.4);
    drag.lastX = e.clientX;
    drag.samples.push([e.timeStamp, e.clientX]);
    drag.samples = drag.samples.filter(([t]) => e.timeStamp - t < 100);
  }

  function pointerUp(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.id) return;
    const d = drag;
    drag = null;
    if (!d.moving) return;
    swallowClick = true;
    setTimeout(() => (swallowClick = false));
    const [t0, x0] = d.samples[0] ?? [0, 0];
    const [t1, x1] = d.samples[d.samples.length - 1] ?? [0, 0];
    const vx = t1 > t0 ? ((x1 - x0) / (t1 - t0)) * 1000 : 0;
    const v = -vx / spacing;
    touchedAt = performance.now();
    // fling, then settle on the nearest album
    const to = clamp(Math.round(pos + v * 0.35), 0, last);
    springTo(to, v, 0.85, () => (dragging = false));
  }

  function pointerCancel() {
    if (drag?.moving) springTo(clamp(Math.round(pos), 0, last), 0, 0.85, () => (dragging = false));
    drag = null;
  }

  // trackpad: horizontal scrolling browses, then snaps to the nearest album
  let wheelTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const onWheel = (e: WheelEvent) => {
      if (!albums.length || Math.abs(e.deltaX) <= Math.abs(e.deltaY)) return;
      e.preventDefault();
      stop();
      dragging = true;
      opened = null;
      touchedAt = performance.now();
      const px = e.deltaMode === 1 ? e.deltaX * 16 : e.deltaMode === 2 ? e.deltaX * W : e.deltaX;
      pos = clamp(pos + px / spacing, -0.4, last + 0.4);
      clearTimeout(wheelTimer);
      wheelTimer = setTimeout(() => springTo(clamp(Math.round(pos), 0, last), 0, 0.85, () => (dragging = false)), 140);
    };
    root.addEventListener("wheel", onWheel, { passive: false });
    return () => root.removeEventListener("wheel", onWheel);
  });

  /** ←/→: one album along (never the music). */
  function browse(dir: number) {
    const base = target ?? Math.round(pos);
    touchedAt = performance.now();
    opened = null;
    springTo(clamp(base + dir, 0, last));
  }

  // before the app's own keys (where ←/→ seek): here they only browse
  function onKey(e: KeyboardEvent) {
    if (!albums.length || app.queueMenu || app.confirmDialog || e.metaKey || e.ctrlKey || e.altKey || e.shiftKey) return;
    const el = e.target as HTMLElement;
    if (el.closest?.("input, textarea, select, [role=dialog]")) return;
    if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
      e.preventDefault();
      e.stopPropagation();
      browse(e.key === "ArrowLeft" ? -1 : 1);
    } else if (e.key === "Enter" && !el.closest?.("button, a")) {
      e.preventDefault();
      e.stopPropagation();
      touchedAt = performance.now();
      const album = albums[front];
      if (opened) opened = null;
      else if (album.tracks.length) opened = album.key;
    } else if (e.key === "Escape" && opened) {
      e.preventDefault();
      e.stopPropagation();
      opened = null;
    }
  }

  function clickAlbum(i: number) {
    if (swallowClick) return;
    touchedAt = performance.now();
    if (i === front) {
      if (albums[i].tracks.length) opened = albums[i].key;
    } else {
      opened = null;
      springTo(i);
    }
  }

  // ---------------------------------------------------------------- drawing
  /** Albums near enough to draw, the front one last (on top). */
  const shown = $derived.by(() => {
    const list: number[] = [];
    for (let i = Math.max(0, Math.ceil(pos - visible)); i <= Math.min(last, Math.floor(pos + visible)); i++) {
      if (Math.abs(i - pos) < visible && !(i === front && flip > 90)) list.push(i);
    }
    return list;
  });

  function place(i: number): string {
    const d = i - pos;
    const near = clamp(d, -1, 1);
    const x = Math.abs(d) <= 1 ? spacing * d : (spacing + step * (Math.abs(d) - 1)) * Math.sign(d);
    const scale = 1 - 0.18 * Math.abs(near);
    const dim = i === front ? 1 : 1 - 0.6 * (flip / 180);
    const alpha = clamp(visible - Math.abs(d), 0, 1) * dim;
    const turn = 62 * near - (i === front ? flip : 0);
    return [
      `left:${W / 2 - side / 2 + x}px`,
      `top:${top}px`,
      `width:${side}px`,
      `z-index:${10000 - Math.round(Math.abs(d) * 100)}`,
      `opacity:${alpha}`,
      `transform:perspective(${perspective}px) rotateY(${turn}deg) scale(${scale})`,
    ].join(";");
  }

  const frontAlbum = $derived(albums[front] as FlowAlbum | undefined);
  const backAlbum = $derived(flip > 90 && backKey && frontAlbum?.key === backKey ? frontAlbum : null);

  function queueAlbum(e: MouseEvent, album: FlowAlbum) {
    e.stopPropagation();
    app.openQueueMenu(e, album.title, (next) => app.queueFiles(album.tracks.map((t) => t.path), album.title, next));
  }

  function queueTrack(e: MouseEvent, track: Track) {
    e.stopPropagation();
    app.openQueueMenu(e, track.title, (next) => app.queueFiles([track.path], track.title, next));
  }
</script>

<svelte:window onkeydowncapture={onKey} />

<div
  class="flow"
  bind:this={root}
  bind:clientWidth={W}
  bind:clientHeight={H}
  onpointerdown={pointerDown}
  onpointermove={pointerMove}
  onpointerup={pointerUp}
  onpointercancel={pointerCancel}
  role="region"
  aria-label="Cover Flow"
>
  {#if W > 0 && albums.length}
    {#each shown as i (albums[i].key)}
      {@const album = albums[i]}
      <button class="album" style={place(i)} tabindex="-1" onclick={() => clickAlbum(i)} aria-label={album.title}>
        <Cover path={album.cover} size={side} radius={0} icon="disc" />
        <!-- reflection on the glossy floor -->
        <span class="reflection" style:height="{side * REFLECTION}px">
          <span class="mirror"><Cover path={album.cover} size={side} radius={0} icon="disc" /></span>
          <span class="fade"></span>
        </span>
      </button>
    {/each}

    {#if backAlbum}
      {@const album = backAlbum}
      <div
        class="back"
        style:width="{backW}px"
        style:height="{backH}px"
        style:left="{(W - backW) / 2}px"
        style:top="{(H - backH) / 2}px"
        style:transform="perspective({perspective}px) rotateY({180 - flip}deg)"
      >
        <div class="head">
          <button class="head-close" onclick={() => (opened = null)} title="Turn the album back" aria-label="Turn the album back">
            <b class="ellipsis">{album.title}</b>
            <span class="ellipsis">{album.artist}</span>
          </button>
          <button class="head-btn" onclick={(e) => queueAlbum(e, album)} title="Play next / Add to queue" aria-label="Play next / Add to queue">
            <Icon name="queue" size={18} />
          </button>
          <button class="head-btn" onclick={() => api.play(album.tracks.map((t) => t.path))} title="Play" aria-label="Play {album.title}">
            <Icon name="play" size={18} />
          </button>
        </div>
        <div class="songs">
          {#each album.tracks as track, i (track.path)}
            {@const playing = track.path === current?.path}
            <div class="song" class:playing class:odd={i % 2 === 1}>
              <button class="song-play" onclick={() => api.play(album.tracks.map((t) => t.path), i)}>
                <span class="num">{track.trackNumber > 0 ? track.trackNumber : i + 1}</span>
                <span class="name ellipsis">{track.title}</span>
                {#if track.durationMs > 0}<span class="len">{formatTime(track.durationMs)}</span>{/if}
              </button>
              <button class="song-more" onclick={(e) => queueTrack(e, track)} title="Play next / Add to queue" aria-label="Play next / Add to queue">
                <Icon name="more" size={16} />
              </button>
            </div>
          {/each}
        </div>
      </div>
    {:else if frontAlbum && flip <= 90}
      <!-- caption of the album in front -->
      <div class="caption" style:top="{top + side * (1 + REFLECTION)}px" style:width="{W - 32}px" style:left="16px" style:height="{CAPTION}px">
        <b class="ellipsis">{frontAlbum.title}</b>
        <span class="ellipsis">{frontAlbum.tracks.length ? `Click for the songs · ${frontAlbum.artist}` : frontAlbum.artist}</span>
      </div>
    {/if}
  {/if}
</div>

<style>
  .flow {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
    border-radius: 8px;
    background: linear-gradient(180deg, #2a2a2a, #000);
    user-select: none;
    touch-action: pan-y;
    /* the albums' z-order stays inside (menus above it keep theirs) */
    isolation: isolate;
  }
  .album {
    position: absolute;
    display: block;
    padding: 0;
    border-radius: 0;
    background: none;
    transform-origin: 50% 50%;
    backface-visibility: hidden;
    cursor: pointer;
  }
  .album :global(.cover) {
    display: block;
  }
  .reflection {
    position: relative;
    display: block;
    overflow: hidden;
  }
  /* a whole cover turned upside down: the top of the floor shows its bottom edge */
  .mirror {
    position: absolute;
    inset: 0 0 auto;
    display: block;
    transform: scaleY(-1);
    transform-origin: 50% 50%;
    opacity: 0.35;
  }
  .fade {
    position: absolute;
    inset: 0;
    background: linear-gradient(180deg, rgb(0 0 0 / 0.3), #000);
  }
  .caption {
    position: absolute;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    pointer-events: none;
    z-index: 20000;
  }
  .caption b {
    max-width: 100%;
    color: #fff;
    font-size: 14px;
  }
  .caption span {
    max-width: 100%;
    color: #b0b0b0;
    font-size: 12px;
  }
  .back {
    position: absolute;
    z-index: 20000;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    border-radius: 6px;
    background: linear-gradient(180deg, #f7f7f7, #dcdcdc);
    border: 1px solid #8c8c8c;
    color: #1e1e1e;
    box-shadow: 0 20px 60px rgb(0 0 0 / 0.6);
  }
  .head {
    display: flex;
    align-items: center;
    padding: 4px 4px 4px 0;
    background: linear-gradient(180deg, #5a5a5a, #2e2e2e);
    color: #fff;
  }
  .head-close {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    padding: 4px 12px;
    text-align: left;
  }
  .head-close b {
    max-width: 100%;
    font-size: 14px;
  }
  .head-close span {
    max-width: 100%;
    font-size: 12px;
    color: #cfcfcf;
  }
  .head-btn {
    display: grid;
    place-items: center;
    width: 38px;
    height: 38px;
    border-radius: 50%;
    color: #fff;
  }
  .head-btn:hover {
    background: rgb(255 255 255 / 0.12);
  }
  .songs {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
  .song {
    display: flex;
    align-items: center;
  }
  .song.odd {
    background: rgb(0 0 0 / 0.06);
  }
  .song.playing {
    background: #bfd4f2;
  }
  .song:hover:not(.playing) {
    background: rgb(0 0 0 / 0.1);
  }
  .song-play {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 8px 0 8px 12px;
    text-align: left;
    color: #1e1e1e;
  }
  .num {
    flex: none;
    width: 26px;
    font-size: 12px;
    color: #777;
  }
  .name {
    flex: 1;
    min-width: 0;
    font-size: 14px;
  }
  .song.playing .name {
    font-weight: 700;
  }
  .len {
    flex: none;
    font-size: 12px;
    color: #666;
  }
  .song-more {
    flex: none;
    display: grid;
    place-items: center;
    width: 36px;
    height: 34px;
    color: #555;
  }
  .song-more:hover {
    color: #000;
  }
</style>
