// Song lyrics for the now-playing room: what to ask the Rust side for the
// song a source plays, a per-session cache, the open / closed view and the
// line being sung. The view is src/components/LyricsView.svelte.
import { api, errorText, type LyricLine, type Lyrics } from "./api";
import { share } from "./share.svelte";
import type { PlayerSource } from "./source.svelte";
import { app } from "./state.svelte";

export type LyricsEntry =
  | { status: "loading" }
  | { status: "none" }
  | { status: "error"; message: string }
  | { status: "ready"; lyrics: Lyrics };

/** What identifies a song's lyrics: a local file (`path`) and / or a Tidal track. */
export interface LyricsRequest {
  key: string;
  trackId: string | null;
  path: string | null;
}

const OPEN_KEY = "homefi.lyrics.open";

function readOpen(): boolean {
  try {
    return sessionStorage.getItem(OPEN_KEY) === "1";
  } catch {
    return false;
  }
}

class LyricsState {
  /** The lyrics view replaces the room (remembered for the session). */
  open = $state(readOpen());
  /** By request key, for the whole session. */
  entries = $state<Record<string, LyricsEntry>>({});
  /** Karaoke is on screen with the mascot singing (the floating mascot sings too). */
  singing = $state(false);
  private pending = new Map<string, Promise<LyricsEntry>>();

  setOpen(open: boolean) {
    this.open = open;
    try {
      sessionStorage.setItem(OPEN_KEY, open ? "1" : "0");
    } catch {
      // private window: only this page remembers it
    }
  }

  toggle() {
    this.setOpen(!this.open);
  }

  /** Opens the lyrics from anywhere (the deck): the room comes to the front. */
  show() {
    if (app.view.name !== "room" && app.view.name !== "guest") app.view = share.inRoom ? { name: "guest" } : { name: "room" };
    if (app.view.name === "guest") share.tab = "player";
    this.setOpen(true);
  }

  /** What to ask for the song `src` plays; null when nothing plays. */
  request(src: PlayerSource): LyricsRequest | null {
    const track = src.current;
    if (!track) return null;
    if (src.remote) {
      // someone else's room: only its Tidal songs have lyrics we can fetch
      const source = share.guest.state?.now_playing?.source ?? "";
      const id = source.startsWith("tidal:") ? source.slice(6) : null;
      return { key: `room|${source}`, trackId: id, path: null };
    }
    // local files first (.lrc / tag), and the Tidal id when known (downloads, streams)
    const id = app.current?.path === track.path ? app.currentTidalId : null;
    return { key: `${track.path}|${id ?? ""}`, trackId: id, path: track.path };
  }

  entry(req: LyricsRequest | null): LyricsEntry | null {
    return req ? (this.entries[req.key] ?? null) : null;
  }

  /** Fetches once per session (again with `force`, the Retry button). */
  load(req: LyricsRequest, force = false): Promise<LyricsEntry> {
    const have = this.entries[req.key];
    if (have && !force) return Promise.resolve(have);
    const running = this.pending.get(req.key);
    if (running) return running;
    if (!req.trackId && !req.path) {
      this.entries[req.key] = { status: "none" };
      return Promise.resolve(this.entries[req.key]);
    }
    this.entries[req.key] = { status: "loading" };
    const job = api
      .lyrics(req.trackId, req.path)
      .then((l): LyricsEntry => (l && l.lines.length ? { status: "ready", lyrics: l } : { status: "none" }))
      .catch((e): LyricsEntry => ({ status: "error", message: errorText(e) }))
      .then((entry) => {
        this.entries[req.key] = entry;
        this.pending.delete(req.key);
        return entry;
      });
    this.pending.set(req.key, job);
    return job;
  }
}

export const lyrics = new LyricsState();

/** Index of the line sung at `ms` (the last one started), or -1 before the first. */
export function lineAt(lines: LyricLine[], ms: number): number {
  let lo = 0;
  let hi = lines.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if ((lines[mid].time_ms ?? 0) <= ms) lo = mid + 1;
    else hi = mid;
  }
  return lo - 1;
}

/** Longest a single line fills for, when no blank line marks its end (a solo follows). */
const MAX_LINE_MS = 7000;
/** A gap longer than this shows the "♪ ♪ ♪" interlude. */
export const GAP_MS = 4000;

export interface KaraokeFrame {
  /** Line being sung (an index into `lines`), or -1 during an interlude. */
  current: number;
  /** 0..1: how much of it has been sung. */
  fill: number;
  /** The next line with words, or -1. */
  next: number;
  /** The previous line with words, or -1. */
  previous: number;
  /** Interlude (intro or a long gap): ms until the next line starts. */
  waitMs: number | null;
}

/** Where karaoke is at `ms`: blank lines only end the line before them. */
export function karaokeAt(lines: LyricLine[], ms: number): KaraokeFrame {
  const sung = (i: number) => i >= 0 && i < lines.length && lines[i].text.trim() !== "";
  const nextWords = (from: number) => {
    for (let i = from; i < lines.length; i++) if (sung(i)) return i;
    return -1;
  };
  const prevWords = (from: number) => {
    for (let i = from; i >= 0; i--) if (sung(i)) return i;
    return -1;
  };
  const at = lineAt(lines, ms);
  const words = prevWords(at);
  if (words < 0) {
    const next = nextWords(0);
    return { current: -1, fill: 0, next, previous: -1, waitMs: next >= 0 ? (lines[next].time_ms ?? 0) - ms : null };
  }
  const start = lines[words].time_ms ?? 0;
  const following = lines[words + 1]?.time_ms;
  const end = Math.min(following ?? start + MAX_LINE_MS, start + MAX_LINE_MS);
  const next = nextWords(words + 1);
  const nextStart = next >= 0 ? (lines[next].time_ms ?? 0) : null;
  if (ms >= end && (nextStart === null || nextStart - end > GAP_MS)) {
    return { current: -1, fill: 0, next, previous: words, waitMs: nextStart === null ? null : nextStart - ms };
  }
  const fill = Math.max(0, Math.min(1, (ms - start) / Math.max(1, end - start)));
  return { current: words, fill, next, previous: prevWords(words - 1), waitMs: null };
}

/** "Lyrics by Musixmatch" for Tidal's provider codes. */
export function creditOf(provider: string | null): string | null {
  if (!provider) return null;
  if (provider.toUpperCase() === "MUSIXMATCH") return "Lyrics by Musixmatch";
  const name = provider.charAt(0).toUpperCase() + provider.slice(1).toLowerCase();
  return `Lyrics by ${name}`;
}
