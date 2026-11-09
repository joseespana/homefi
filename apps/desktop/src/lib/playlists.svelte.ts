// The playlists HomeFi makes ("Get ready for a concert"). Lives across navigation: a build
// can take a couple of minutes, and leaving the screen must not lose it. Lists are
// `$state.raw` (replaced, never mutated). The product rules (tour set, thresholds, fallback
// order) are in homefi-core; this only holds state and sends intents.
import { api, errorText, on, type ChatBrief, type ConcertBuilt, type ConcertMode, type ConcertProgress, type ConcertRequest, type ConcertSources, type HomePlaylist, type PartyBrief, type PlaylistOrigin, type PlaylistSettings, type SourceContext, type StreamItem, type TidalSyncEvent } from "./api";
import { uiLanguage } from "./eq.svelte";

/** What the core answers to a build the user cancelled. */
const CANCELLED = "cancelled";

/** The queue context that makes the user's Sounds apply to a HomeFi playlist. */
export const sourceOf = (p: HomePlaylist): SourceContext => ({ kind: "playlist", id: p.id, name: p.name });

/** One entry as a player item; null when it has neither a Tidal id nor a library file. */
function toItem(e: HomePlaylist["entries"][number], source: SourceContext): StreamItem | null {
  if (!e.tidal_id && !e.local_path) return null;
  return { id: e.tidal_id ?? "", title: e.title, local: e.local_path, source, artist: e.artist, album: e.album, cover_url: e.cover_url, duration_ms: e.duration_ms, explicit: e.explicit };
}

/** The playable entries (index of each in `entries`, for "play from this row"). */
export function playable(p: HomePlaylist): { items: StreamItem[]; indexOf: Map<number, number> } {
  const source = sourceOf(p);
  const items: StreamItem[] = [];
  const indexOf = new Map<number, number>();
  p.entries.forEach((e, i) => {
    const item = toItem(e, source);
    if (item) {
      indexOf.set(i, items.length);
      items.push(item);
    }
  });
  return { items, indexOf };
}

/** A playlist made by "Get ready for a concert" / "Make a party playlist" / "Make a playlist with AI" (the origin tells which). */
export type ConcertPlaylist = HomePlaylist & { origin: Extract<PlaylistOrigin, { kind: "concert" }> };
export type PartyPlaylist = HomePlaylist & { origin: Extract<PlaylistOrigin, { kind: "party" }> };
export type ChatPlaylist = HomePlaylist & { origin: Extract<PlaylistOrigin, { kind: "chat" }> };
export const isConcert = (p: HomePlaylist): p is ConcertPlaylist => p.origin.kind === "concert";
export const isParty = (p: HomePlaylist): p is PartyPlaylist => p.origin.kind === "party";
export const isChat = (p: HomePlaylist): p is ChatPlaylist => p.origin.kind === "chat";
/** What a chat playlist was asked for ("Keep chatting"). */
export const chatBriefOf = (p: HomePlaylist): ChatBrief | null => (isChat(p) ? p.origin.brief : null);
/** The answers a party playlist was made with ("Change answers"). */
export const briefOf = (p: HomePlaylist): PartyBrief | null => (isParty(p) ? p.origin.brief : null);

/** The request that repeats a saved playlist's search (no new AI call is made by rearrange). */
export function requestOf(p: ConcertPlaylist, mode: ConcertMode = p.origin.mode): ConcertRequest {
  const o = p.origin;
  return { artist: o.artist, shows: o.shows_wanted, thresholds: o.thresholds, mode, popular_only: o.source.kind === "tidal_popular" };
}

/** Saving one playlist to TIDAL: running, done (with the words to show) or failed. */
export type TidalStatus =
  | { id: string; state: "saving" }
  | { id: string; state: "saved"; summary: string; skipped: string[] }
  | { id: string; state: "failed"; message: string };

class PlaylistStore {
  list = $state.raw<HomePlaylist[]>([]);
  loaded = $state(false);
  /** Built, not saved yet. */
  draft = $state.raw<ConcertBuilt | null>(null);
  building = $state(false);
  progress = $state.raw<ConcertProgress | null>(null);
  error = $state<string | null>(null);
  sources = $state.raw<ConcertSources | null>(null);
  /** The last save to TIDAL (button or automatic), for the playlist it names. */
  tidal = $state.raw<TidalStatus | null>(null);

  #cancelling = false;
  #listening = false;

  /** Automatic saves to TIDAL run in the background; their outcome arrives as an event. */
  #listen() {
    if (this.#listening) return;
    this.#listening = true;
    on.homePlaylistTidal((e) => this.#tidalDone(e)).catch(() => (this.#listening = false));
  }

  #tidalDone(e: TidalSyncEvent) {
    if (e.kind === "failed") {
      this.tidal = { id: e.id, state: "failed", message: e.message };
      return;
    }
    this.#replace(e.playlist);
    this.tidal = { id: e.playlist.id, state: "saved", summary: e.report.summary, skipped: e.report.skipped };
  }

  /** Puts the stored copy of a playlist in the list, where it was (new ones go first). */
  #replace(p: HomePlaylist) {
    const at = this.list.findIndex((x) => x.id === p.id);
    this.list = at < 0 ? [p, ...this.list] : this.list.map((x) => (x.id === p.id ? p : x));
  }

  async load() {
    this.#listen();
    try {
      this.list = await api.homePlaylists();
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.loaded = true;
    }
  }

  async loadSources() {
    try {
      this.sources = await api.concertSources();
    } catch {
      this.sources = null;
    }
  }

  async save(p: HomePlaylist) {
    this.#listen();
    const stored = await api.homePlaylistSave(p);
    this.list = [stored, ...this.list.filter((x) => x.id !== p.id)];
  }

  /** "Save to TIDAL" / "Update on TIDAL": the TIDAL copy ends up with the same songs, in order. */
  async saveToTidal(p: HomePlaylist) {
    if (this.tidal?.state === "saving") return;
    this.tidal = { id: p.id, state: "saving" };
    try {
      const done = await api.homePlaylistSaveToTidal(p.id, uiLanguage());
      this.#replace(done.playlist);
      this.tidal = { id: p.id, state: "saved", summary: done.report.summary, skipped: done.report.skipped };
    } catch (e) {
      this.tidal = { id: p.id, state: "failed", message: errorText(e) };
    }
  }

  async remove(id: string) {
    if (await api.homePlaylistDelete(id)) this.list = this.list.filter((x) => x.id !== id);
  }

  /** The request for the screen's inline values on top of the user's settings. */
  request(settings: PlaylistSettings, overrides: Partial<ConcertRequest> = {}): ConcertRequest {
    return { artist: "", shows: settings.concert_shows, thresholds: settings.concert_thresholds, mode: { kind: "tour" }, popular_only: false, ...overrides };
  }

  /** Builds a playlist; `replaceId` keeps a saved playlist's identity ("Search again"). */
  async build(request: ConcertRequest, replaceId: string | null = null): Promise<void> {
    if (this.building) return;
    this.building = true;
    this.#cancelling = false;
    this.error = null;
    this.progress = null;
    this.draft = null;
    try {
      const built = await api.concertBuild(request, replaceId, uiLanguage(), (p) => (this.progress = p));
      if (replaceId) {
        // searching again on a saved playlist: the result replaces it
        await this.save(built.playlist);
        this.draft = built.fallback_note ? built : null;
      } else {
        this.draft = built;
      }
    } catch (e) {
      if (!this.#cancelling && errorText(e) !== CANCELLED) this.error = errorText(e);
    } finally {
      this.building = false;
      this.progress = null;
    }
  }

  async cancel() {
    if (!this.building) return;
    this.#cancelling = true;
    await api.concertCancel().catch(() => {});
  }

  /** Another mode or settings over the shows already found (no AI call). */
  async rearrange(p: HomePlaylist, request: ConcertRequest): Promise<void> {
    if (this.building) return;
    this.building = true;
    this.#cancelling = false;
    this.error = null;
    this.progress = null;
    try {
      const next = await api.concertRearrange(p, request, uiLanguage(), (pr) => (this.progress = pr));
      if (this.list.some((x) => x.id === p.id)) await this.save(next);
      else if (this.draft?.playlist.id === p.id) this.draft = { ...this.draft, playlist: next };
    } catch (e) {
      if (!this.#cancelling && errorText(e) !== CANCELLED) this.error = errorText(e);
    } finally {
      this.building = false;
      this.progress = null;
    }
  }

  discard() {
    this.draft = null;
    this.error = null;
  }

  play(p: HomePlaylist, index = 0, shuffle = false) {
    const { items, indexOf } = playable(p);
    if (!items.length) return;
    api.playTidal(items, indexOf.get(index) ?? 0, shuffle, sourceOf(p)).catch((e) => (this.error = errorText(e)));
  }

  queue(p: HomePlaylist, next: boolean) {
    const { items } = playable(p);
    if (items.length) api.queueTidal(items, next).catch((e) => (this.error = errorText(e)));
  }
}

export const playlists = new PlaylistStore();
