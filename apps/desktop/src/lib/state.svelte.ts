// App-wide reactive state (Svelte 5 runes), fed by Rust events.
import { snapTime } from "./snap";
import { Paged } from "./paged.svelte";
import { api, errorText, on, type Account, type LibraryKind, type AppInfo, type DownloadEvent, type Hit, type Kind, type PlayerState, type Settings, type StreamItem, type Theme, type Track, type PlayerLayout, type Upcoming, type SourceContext, type TasteScope, type VersionHit, type AlbumVersionPrefs } from "./api";
import { fixedTheme } from "./layouts";
import { TERMS_VERSION } from "./terms";

export type View =
  | { name: "room" }
  | { name: "songs" }
  | { name: "albums" }
  | { name: "album"; key: string }
  | { name: "artists" }
  | { name: "artist"; name_: string }
  | { name: "playlists" }
  | { name: "playlist"; path: string }
  | { name: "search" }
  | { name: "tidal"; tab: "playlists" | "albums" | "artists" | "tracks" }
  | { name: "tidal-item"; kind: "album" | "playlist" | "artist" | "mix"; id: string; back?: View }
  | { name: "folder"; path: string }
  | { name: "downloads" }
  /** `page` opens a section (Settings → Sound, AI...). */
  | { name: "settings"; page?: string }
  /** The equalizer; `output` preselects whose profile to edit. */
  | { name: "eq"; output?: string; tab?: "device" | "taste" }
  /** "Set up my sound": the EQ assistant (AI or the guided wizard). */
  | { name: "assistant" }
  /** Someone else's room (shared control / party), as a guest. */
  | { name: "guest" };

export interface ActiveDownload {
  task: number;
  title: string;
  quality: string;
  downloaded: number;
  total: number | null;
}

/** Download state of one Tidal track, shown on its row wherever it appears. */
export interface TrackDownload {
  state: "waiting" | "downloading" | "done" | "failed";
  /** Where it was saved, once done. */
  path?: string;
  /** 0..1 while downloading. */
  progress: number;
  detail?: string;
}

export interface Note {
  id: number;
  kind: "info" | "success" | "error" | "loading";
  text: string;
  key?: string;
  /** A person's colour (shared control notices). */
  color?: string;
  /** A button on the note ("Undo"); clicking it dismisses the note. */
  action?: { label: string; run: () => void };
}

/** What a confirmation dialog asks (see `app.confirm`). */
export interface ConfirmOptions {
  title: string;
  text?: string;
  /** Label of the button that goes ahead (default "Confirm"). */
  confirmLabel?: string;
  cancelLabel?: string;
  /** Destructive: red button, and Cancel holds the focus so a stray Enter backs out. */
  danger?: boolean;
  /** An extra choice, unticked by default (e.g. "Also block this device"). */
  check?: string;
}

export interface LogEntry {
  id: number;
  kind: "done" | "exists" | "skipped" | "error";
  title: string;
  detail: string;
  path?: string;
}

class AppState {
  view = $state<View>({ name: "room" });
  info = $state<AppInfo | null>(null);
  account = $state<Account | null>(null);
  loginOpen = $state(false);

  // Big lists that are only ever replaced whole: raw (not deeply proxied), so
  // reading them costs one signal instead of one per track and field. Svelte
  // re-checks every dependency of a derived read outside an effect, and with
  // 10k proxied tracks that alone froze the app on every player tick.
  tracks = $state.raw<Track[]>([]);
  /** Tidal tracks still to come after the player's queue (fetched just before they play). */
  upcoming = $state.raw<Upcoming[]>([]);
  settings = $state<Settings | null>(null);
  /** ISRCs of local files, to mark Tidal tracks that are already downloaded. */
  localIsrc = $derived(new Map(this.tracks.filter((t) => t.isrc).map((t) => [t.isrc!, t])));
  scanning = $state(false);

  /** Replaced by every "player" event (4 a second while playing). */
  player = $state.raw<PlayerState | null>(null);
  /** The track being heard. */
  current = $derived<Track | null>(this.player?.track ?? null);
  /** Queue entries outside the library (Tidal stream cache, opened folders). */
  outside = $state.raw<Record<string, Track | null>>({});

  busy = $state(false);
  jobLabel = $state("");
  queued = $state(0);
  finished = $state(0);
  active = $state<Record<number, ActiveDownload>>({});
  log = $state<LogEntry[]>([]);
  /** By Tidal track id. */
  trackDownloads = $state<Record<string, TrackDownload>>({});
  /** Jobs (by label) waiting for their turn or running: albums, playlists, artists. */
  jobs = $state<Record<string, "waiting" | "running">>({});
  private taskIds: Record<number, string> = {};

  /** Tidal favorites as `kind:id` (track, album, artist, playlist). */
  favorites = $state<Set<string>>(new Set());
  /** The user's own playlists, for "Add to playlist" (loaded on first use). */
  myPlaylists = $state<Hit[] | null>(null);
  /** Tidal lists already loaded in My Tidal; cleared when the collection changes. */
  tidalCache = new Map<string, Paged>();
  /** Tidal track id of files played through `playTidal` (local copies and stream cache). */
  tidalIdByPath = $state<Record<string, string>>({});
  /** Floating notifications (bottom-right). A `key` lets a note be updated in place. */
  notes = $state<Note[]>([]);
  private noteIds = 0;
  private noteTimers = new Map<number, ReturnType<typeof setTimeout>>();
  /** Open "Add to playlist" menu, anchored at a screen point. */
  playlistPicker = $state<{ trackIds: string[]; x: number; y: number } | null>(null);
  queueMenu = $state<{ label: string; x: number; y: number; run: (next: boolean) => void; sound?: { scope: TasteScope; label: string } } | null>(null);
  /** Open album version picker (a card's "3 versions" chip, the album page's "Other versions"). */
  versionPicker = $state<{ title: string; versions: VersionHit[]; current: string | null; x: number; y: number } | null>(null);
  /** Open "New playlist" dialog; the tracks go in once it's created. */
  newPlaylist = $state<{ trackIds: string[] } | null>(null);
  /** Open confirmation dialog; `resolve` answers the `ask()` that opened it. */
  confirmDialog = $state<(ConfirmOptions & { checked: boolean; resolve: (ok: boolean, checked: boolean) => void }) | null>(null);

  /** Asks before anything destructive (delete, remove, log out): true only if the user confirms. */
  confirm(options: ConfirmOptions): Promise<boolean> {
    return this.ask(options).then((r) => r.ok);
  }

  /** Like `confirm`, also telling whether the extra `check` was ticked. */
  ask(options: ConfirmOptions): Promise<{ ok: boolean; checked: boolean }> {
    // a second question cancels the first one
    this.confirmDialog?.resolve(false, false);
    return new Promise((resolve) => (this.confirmDialog = { ...options, checked: false, resolve: (ok, checked) => resolve({ ok, checked }) }));
  }

  /** Closes the confirmation dialog with the user's answer. */
  answerConfirm(ok: boolean) {
    const d = this.confirmDialog;
    this.confirmDialog = null;
    d?.resolve(ok, ok && d.checked);
  }

  /** Every local file of these Tidal tracks (matched by ISRC; a track may exist twice). */
  localFilesOf(hits: Hit[]): string[] {
    const isrcs = new Set(hits.map((h) => h.isrc).filter(Boolean));
    return this.tracks.filter((t) => t.isrc && isrcs.has(t.isrc)).map((t) => t.path);
  }

  /** Tidal track ids whose files are being moved to the Trash. */
  deletingIds = $state<Set<string>>(new Set());
  /** Rows crumbling to dust, with their place in the wave (for the stagger). */
  snapping = $state<Record<string, number>>({});
  /** A library reload that waits until the dust settles. */
  private holdLibrary = false;
  private libraryDirty = false;

  /**
   * Moves the downloaded files of these Tidal tracks to the Trash, with the snap animation,
   * once the user confirms. `started` runs right after the confirmation.
   */
  async deleteDownloads(hits: Hit[], label: string, started?: () => void) {
    const paths = this.localFilesOf(hits);
    if (!paths.length) return;
    const files = paths.length === 1 ? "1 file" : `${paths.length} files`;
    const ok = await this.confirm({
      title: `Delete ${label}?`,
      text: `${files} will be moved to the Trash. You can put ${paths.length === 1 ? "it" : "them"} back from there.`,
      confirmLabel: "Move to Trash",
      danger: true,
    });
    if (!ok) return;
    started?.();
    this.deletingIds = new Set(hits.map((h) => h.id));
    this.holdLibrary = true;
    try {
      const removed = new Set(await api.deleteTracks(paths));
      const gone = new Set(this.tracks.filter((t) => t.isrc && removed.has(t.path)).map((t) => t.isrc));
      const snapped = hits.filter((h) => h.isrc && gone.has(h.isrc));
      this.deletingIds = new Set();
      this.snapping = Object.fromEntries(snapped.map((h, i) => [h.id, i]));
      // rows crumble one after another; their "downloaded" mark goes once they're dust
      await new Promise((r) => setTimeout(r, snapTime(snapped.length)));
      this.tracks = this.tracks.filter((t) => !removed.has(t.path));
      for (const [id, d] of Object.entries(this.trackDownloads)) {
        if (d.path && removed.has(d.path)) delete this.trackDownloads[id];
      }
      this.snapping = {};
      const n = removed.size;
      this.notify(n < paths.length ? `Moved ${n} of ${paths.length} files to the Trash` : `Moved ${label} to the Trash`, n < paths.length);
    } catch (e) {
      this.deletingIds = new Set();
      this.snapping = {};
      this.notify(errorText(e), true);
    } finally {
      this.holdLibrary = false;
      if (this.libraryDirty) {
        this.libraryDirty = false;
        this.tracks = await api.library();
      }
    }
  }

  /** Opens "Play next / Add to queue" next to the clicked button (or at the pointer, on right-click). */
  openQueueMenu(e: MouseEvent, label: string, run: (next: boolean) => void, sound?: TasteScope) {
    e.preventDefault();
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const x = e.type === "contextmenu" ? e.clientX + 200 : r.right;
    const y = e.type === "contextmenu" ? e.clientY : r.bottom + 6;
    this.queueMenu = { label, x, y, run, sound: sound && { scope: sound, label } };
  }

  /**
   * Opens the version picker next to the clicked chip (or at the pointer, on
   * right-click). `current` is the album on screen, when on its page.
   */
  openVersions(e: MouseEvent, title: string, versions: VersionHit[], current: string | null = null) {
    e.preventDefault();
    e.stopPropagation();
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const x = e.type === "contextmenu" ? e.clientX : r.left;
    const y = e.type === "contextmenu" ? e.clientY : r.bottom + 6;
    this.versionPicker = { title, versions, current, x, y };
  }

  /** Album versions as Settings → Tidal says (one card per album by default). */
  get albumVersions(): AlbumVersionPrefs {
    return { group: true, explicit: "explicit", spatial: "stereo", edition: "original", ...(this.settings?.album_versions ?? {}) };
  }

  /** Plays a whole Tidal album from its first track. */
  async playAlbum(id: string, title: string) {
    try {
      const d = await api.tidalDetail("album", id);
      if (!d.tracks.length) return this.notify(`“${title}” has no tracks to play`, true);
      await this.playTidal(d.tracks, 0, false, { kind: "album", id, name: title });
    } catch (e) {
      this.notify(errorText(e), true);
    }
  }

  /** Library / local files into the queue. */
  async queueFiles(paths: string[], label: string, next: boolean) {
    if (!paths.length) return;
    try {
      await api.queueAdd(paths, next);
      this.note(next ? `“${label}” plays next` : `Added “${label}” to the queue`, "success");
    } catch (e) {
      this.note(errorText(e), "error");
    }
  }

  /** Tidal tracks into the queue: local copies when we have them, the rest fetched first. */
  async queueHits(hits: Hit[], label: string, next: boolean) {
    const items = hits
      .filter((h) => h.kind === "track")
      .map((h) => streamItem(h, (h.isrc && this.localIsrc.get(h.isrc)?.path) || null));
    if (!items.length) return;
    for (const i of items) if (i.local) this.tidalIdByPath[i.local] = i.id;
    const key = `queue-${++this.ids}`;
    this.note(`Adding “${label}” to the queue…`, "loading", key);
    try {
      const n = await api.queueTidal(items, next);
      this.dismissKey(key);
      const what = items.length === 1 ? `“${label}”` : `${n} songs`;
      this.note(next ? `${what} play${n === 1 ? "s" : ""} next` : `Added ${what} to the queue`, "success");
    } catch (e) {
      this.dismissKey(key);
      this.note(errorText(e), "error");
    }
  }

  /**
   * Opens the "Add to playlist" menu next to the clicked button. `trackIds` may
   * be a promise (a list still loading its other pages): the menu opens once it's there.
   */
  async pickPlaylist(e: MouseEvent, trackIds: string[] | Promise<string[]>) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    try {
      this.playlistPicker = { trackIds: await trackIds, x: r.right, y: r.bottom + 6 };
    } catch (err) {
      this.notify(errorText(err), true);
    }
  }

  /**
   * A list of the user's Tidal collection. One seen before comes back as it was
   * left (with the pages already loaded); `force` (Refresh) or a change to the
   * collection starts it again: the first page, or all of it with lazy lists off.
   */
  tidalList(kind: LibraryKind, force = false): Paged {
    const cached = this.tidalCache.get(kind);
    if (!force && cached && !(cached.error && !cached.items.length)) {
      // left half-loaded (the user went elsewhere): the rest goes on streaming
      if (!this.lazyLists) cached.rest();
      return cached;
    }
    this.tidalCache.get(kind)?.cancel();
    const fresh = new Paged((offset, rest) => api.tidalLibraryPage(kind, offset, rest), true, {
      pages: (offset, onPage, load) => api.tidalLibraryStream(kind, offset, onPage, load),
      cancel: api.tidalCancel,
    });
    this.tidalCache.set(kind, fresh);
    if (this.lazyLists) fresh.more();
    else fresh.whole();
    return fresh;
  }

  /** Tidal lists show their first page and load more while scrolling (the "Load lists as you scroll" setting). */
  get lazyLists(): boolean {
    return this.settings?.lazy_lists ?? true;
  }
  private ids = 0;

  async init() {
    this.info = await api.appInfo();
    this.account = await api.account();
    this.loadFavorites();
    this.watchFiles();
    this.tracks = await api.library();
    this.settings = await api.getSettings();
    this.watchMute();
    // saved from another window (the mini player follows the skin and theme)
    on.settings((s) => (this.settings = s));
    this.player = await api.playerState();
    on.player((s) => {
      this.player = s;
      this.learnQueue(s.queue);
    });
    this.upcoming = await api.streamUpcoming();
    on.upcoming((list) => (this.upcoming = list));
    on.stream((e) => {
      if (e.status === "loading") this.note(`Loading “${e.title}” from Tidal…`, "loading", "stream");
      else if (e.status === "ready") this.dismissKey("stream");
      else this.note(`Couldn't play “${e.title}”: ${e.message}`, "error", "stream");
    });
    on.library(async () => {
      if (this.holdLibrary) this.libraryDirty = true;
      else this.tracks = await api.library();
    });
    on.download((e) => this.onDownload(e));
  }

  /** The sound is silenced (the saved volume is kept); every window follows it. */
  muted = $state(false);

  private watchMute() {
    api.isMuted().then((m) => (this.muted = m)).catch(() => {});
    on.muted((m) => (this.muted = m));
  }

  toggleMute() {
    const next = !this.muted;
    this.muted = next;
    api.mute(next).catch(() => (this.muted = !next));
  }

  /**
   * The mini player's window: only what it shows (theme, the player and the songs in
   * its queue), never the whole library, Tidal or downloads, so it opens at once even
   * where the web view is slow (WebView2 on Windows).
   */
  async initMini() {
    this.settings = await api.getSettings();
    this.watchMute();
    on.settings((s) => (this.settings = s));
    this.player = await api.playerState();
    this.learnQueue(this.player.queue);
    on.player((s) => {
      this.player = s;
      this.learnQueue(s.queue);
    });
  }

  /**
   * Files deleted by hand must stop showing as downloaded: check the library
   * whenever the window comes back to the front, and every minute while visible.
   */
  private watchFiles() {
    const check = () => {
      if (document.visibilityState === "visible") this.pruneMissing();
    };
    window.addEventListener("focus", check);
    document.addEventListener("visibilitychange", check);
    setInterval(check, 60_000);
  }

  private pruning = false;
  async pruneMissing() {
    if (this.pruning) return;
    this.pruning = true;
    try {
      const gone = new Set(await api.pruneLibrary());
      if (!gone.size) return;
      // the "library" event reloads app.tracks; forget finished downloads of those files too
      for (const [id, d] of Object.entries(this.trackDownloads)) {
        if (d.state === "done" && d.path && gone.has(d.path)) delete this.trackDownloads[id];
      }
      this.outside = Object.fromEntries(Object.entries(this.outside).filter(([p]) => !gone.has(p)));
    } catch (e) {
      console.warn("[homefi] prune", e);
    } finally {
      this.pruning = false;
    }
  }

  /**
   * Shows (or updates, with the same `key`) a floating note. Loading notes stay
   * until replaced or dismissed; the others go away on their own.
   */
  note(text: string, kind: Note["kind"] = "info", key?: string, opts: { seconds?: number; color?: string; action?: Note["action"] } = {}): number {
    const existing = key ? this.notes.find((n) => n.key === key) : undefined;
    const id = existing?.id ?? ++this.noteIds;
    if (existing) {
      existing.text = text;
      existing.kind = kind;
      existing.action = opts.action;
    } else {
      this.notes = [...this.notes, { id, kind, text, key, color: opts.color, action: opts.action }].slice(-4);
    }
    clearTimeout(this.noteTimers.get(id));
    const ms = opts.seconds ? opts.seconds * 1000 : kind === "error" ? 8000 : 3500;
    if (kind !== "loading") this.noteTimers.set(id, setTimeout(() => this.dismiss(id), ms));
    return id;
  }

  dismiss(id: number) {
    clearTimeout(this.noteTimers.get(id));
    this.noteTimers.delete(id);
    this.notes = this.notes.filter((n) => n.id !== id);
  }

  dismissKey(key: string) {
    const n = this.notes.find((n) => n.key === key);
    if (n) this.dismiss(n.id);
  }

  notify(text: string, error = false) {
    this.note(text, error ? "error" : "success");
  }

  async loadFavorites() {
    if (!this.account) return;
    try {
      const ids = await api.favoriteIds();
      this.favorites = new Set(Object.entries(ids).flatMap(([kind, list]) => list.map((id) => `${kind}:${id}`)));
    } catch (e) {
      console.warn("[homefi] favorites", e);
    }
  }

  isFavorite(kind: Kind, id: string): boolean {
    return this.favorites.has(`${kind}:${id}`);
  }

  /** Adds / removes from Tidal favorites right away, rolls back if Tidal refuses. */
  async toggleFavorite(kind: Kind, id: string, title = "") {
    const key = `${kind}:${id}`;
    const on = !this.favorites.has(key);
    const next = new Set(this.favorites);
    if (on) next.add(key);
    else next.delete(key);
    this.favorites = next;
    try {
      await api.favorite(kind, id, on);
      this.tidalCache.delete(`${kind}s`);
      this.notify(on ? `Added “${title}” to your favorites` : `Removed “${title}” from your favorites`);
    } catch (e) {
      const back = new Set(this.favorites);
      if (on) back.delete(key);
      else back.add(key);
      this.favorites = back;
      this.notify(errorText(e), true);
    }
  }

  async loadMyPlaylists(force = false) {
    if (this.myPlaylists && !force) return this.myPlaylists;
    this.myPlaylists = await api.myPlaylists();
    return this.myPlaylists;
  }

  async addToPlaylist(playlist: Hit, trackIds: string[]) {
    try {
      await api.playlistAdd(playlist.id, trackIds);
      this.tidalCache.delete("playlists");
      this.notify(`Added ${trackIds.length === 1 ? "1 track" : `${trackIds.length} tracks`} to “${playlist.title}”`);
    } catch (e) {
      this.notify(errorText(e), true);
    }
  }

  async createPlaylist(title: string, description: string, trackIds: string[] = []) {
    const hit = await api.createPlaylist(title, description, trackIds);
    this.myPlaylists = [hit, ...(this.myPlaylists ?? [])];
    this.tidalCache.delete("playlists");
    this.notify(`Created “${hit.title}”`);
    return hit;
  }

  /** Tidal ids per ISRC (a recording is on several releases), looked up for local files. */
  isrcIds = $state<Record<string, string[]>>({});
  private isrcPending = new Set<string>();

  async lookupIsrc(isrc: string) {
    if (!this.account || isrc in this.isrcIds || this.isrcPending.has(isrc)) return;
    this.isrcPending.add(isrc);
    try {
      this.isrcIds[isrc] = await api.tracksByIsrc(isrc);
    } catch (e) {
      console.warn("[homefi] isrc lookup", e);
    } finally {
      this.isrcPending.delete(isrc);
    }
  }

  /** `true` while the current track plays from the Tidal stream cache (not downloaded). */
  get currentIsStream(): boolean {
    return /[\\/]stream-cache[\\/][^\\/]+$/.test(this.current?.path ?? "");
  }

  /** The current track is in the library (or was downloaded while it streamed). */
  get currentIsLocal(): boolean {
    const t = this.current;
    if (!t) return false;
    return !this.currentIsStream || (!!t.isrc && this.localIsrc.has(t.isrc));
  }

  /** Tidal track id of the file being heard, when known (to like / download / add it). */
  currentTidalId = $derived.by((): string | null => {
    const path = this.current?.path;
    if (!path) return null;
    if (this.tidalIdByPath[path]) return this.tidalIdByPath[path];
    // stream cache files are named after the track id
    const m = /[\\/]stream-cache[\\/](\d+)\.[^.\\/]+$/.exec(path);
    if (m) return m[1];
    // local file: one of the releases with its ISRC, preferably the one already in favorites
    const ids = this.current?.isrc ? this.isrcIds[this.current.isrc] : undefined;
    if (!ids?.length) return null;
    return ids.find((id) => this.favorites.has(`track:${id}`)) ?? ids[0];
  });
  /** ISRC of the track being heard: rows compare against this plain value, so a tick doesn't wake them. */
  currentIsrc = $derived<string | null>(this.current?.isrc ?? null);

  /** Track info for a queue path, from the library or read from the file. */
  trackAt(path: string): Track | undefined {
    return this.byPath.get(path) ?? this.outside[path] ?? undefined;
  }
  byPath = $derived(new Map(this.tracks.map((t) => [t.path, t])));

  /** The queue `learnQueue` last looked at: the same one again (every tick) costs nothing. */
  private seenQueue: string[] | null = null;
  private seenLibrary: Track[] | null = null;
  private asking = new Set<string>();

  private async learnQueue(queue: string[]) {
    // ticks resend the same queue: compare it once instead of looking up every path
    if (this.seenLibrary === this.tracks && this.seenQueue && sameList(this.seenQueue, queue)) return;
    this.seenQueue = queue;
    this.seenLibrary = this.tracks;
    const byPath = this.byPath;
    const outside = this.outside;
    const unknown = queue.filter((p) => !byPath.has(p) && !(p in outside) && !this.asking.has(p));
    if (!unknown.length) return;
    for (const p of unknown) this.asking.add(p);
    try {
      const found = await api.tracksFor(unknown);
      this.outside = { ...this.outside, ...Object.fromEntries(unknown.map((p, i) => [p, found[i]])) };
    } finally {
      for (const p of unknown) this.asking.delete(p);
    }
  }

  /** Plays Tidal tracks from `index`: local copies when we have them, the rest streamed. */
  playTidal(hits: Hit[], index = 0, shuffle = false, source: SourceContext | null = null) {
    const items = hits
      .filter((h) => h.kind === "track")
      .map((h) => streamItem(h, (h.isrc && this.localIsrc.get(h.isrc)?.path) || null));
    const start = Math.max(0, items.findIndex((i) => i.id === hits[index]?.id));
    if (!items.length) return;
    for (const i of items) if (i.local) this.tidalIdByPath[i.local] = i.id;
    this.note(`Loading “${items[start].title}”…`, "loading", "stream");
    return api.playTidal(items, start, shuffle, source);
  }

  /**
   * Queues a download. `trackIds` are the Tidal tracks it covers, when known,
   * so their rows show "waiting" right away (the job may wait behind others).
   */
  async download(label: string, links: string[], trackIds: string[] = []) {
    for (const id of trackIds) {
      const s = this.trackDownloads[id]?.state;
      if (s !== "done" && s !== "downloading") this.trackDownloads[id] = { state: "waiting", progress: 0 };
    }
    this.jobs[label] = "waiting";
    try {
      await api.download(label, links);
    } catch (e) {
      delete this.jobs[label];
      for (const id of trackIds) delete this.trackDownloads[id];
      throw e;
    }
  }

  private setTrack(id: string | null | undefined, d: TrackDownload) {
    if (id) this.trackDownloads[id] = d;
  }

  /** Plays an upcoming Tidal track now, the rest of the stream after it. */
  playUpcoming(i: number) {
    const rest = this.upcoming.slice(i).map((u) => ({ id: u.id, title: u.title, artist: u.artist, local: null, album: "", cover_url: null, duration_ms: u.duration_ms, explicit: false }));
    if (rest.length) api.playTidal(rest, 0);
  }

  /** The current personal-use notice was accepted (unknown until the settings load). */
  get termsAccepted(): boolean {
    return (this.settings?.tidal_terms_accepted ?? 0) >= TERMS_VERSION;
  }

  async acceptTerms() {
    await this.saveSettings({ tidal_terms_accepted: TERMS_VERSION, tidal_terms_accepted_at: Date.now() });
  }

  async saveSettings(patch: Partial<Settings>) {
    if (!this.settings) return;
    this.settings = await api.setSettings({ ...this.settings, ...patch });
  }

  async rescan() {
    this.scanning = true;
    try {
      this.tracks = await api.rescanLibrary();
    } finally {
      this.scanning = false;
    }
  }

  private push(entry: Omit<LogEntry, "id">) {
    this.log = [{ id: ++this.ids, ...entry }, ...this.log].slice(0, 300);
  }

  private onDownload(e: DownloadEvent) {
    switch (e.type) {
      case "jobStarted":
        this.busy = true;
        this.jobLabel = e.label;
        this.jobs[e.label] = "running";
        break;
      case "jobFinished":
        this.busy = false;
        this.active = {};
        delete this.jobs[e.label];
        // anything of this job still waiting never came (filtered, cancelled)
        for (const [id, d] of Object.entries(this.trackDownloads)) {
          if (d.state === "waiting" && !Object.keys(this.jobs).length) delete this.trackDownloads[id];
        }
        break;
      case "queued":
        this.queued++;
        if (this.trackDownloads[e.id]?.state !== "done") this.setTrack(e.id, { state: "waiting", progress: 0 });
        break;
      case "started":
        this.active[e.task] = { task: e.task, title: e.title, quality: e.quality, downloaded: 0, total: null };
        this.taskIds[e.task] = e.id;
        this.setTrack(e.id, { state: "downloading", progress: 0, detail: e.quality });
        break;
      case "progress":
        if (this.active[e.task]) {
          this.active[e.task].downloaded = e.downloaded;
          this.active[e.task].total = e.total;
        }
        {
          const id = this.taskIds[e.task];
          const d = id && this.trackDownloads[id];
          if (d && e.total) d.progress = Math.min(1, e.downloaded / e.total);
        }
        break;
      case "finished":
        delete this.active[e.task];
        delete this.taskIds[e.task];
        this.setTrack(e.id, { state: "done", progress: 1, path: e.path });
        this.finished++;
        this.push({ kind: "done", title: e.title, detail: e.quality, path: e.path });
        break;
      case "exists":
        this.finished++;
        this.setTrack(e.id, { state: "done", progress: 1, path: e.path });
        this.push({ kind: "exists", title: e.title, detail: "already downloaded", path: e.path });
        break;
      case "skipped":
        this.finished++;
        this.setTrack(e.id, { state: "failed", progress: 0, detail: e.reason });
        this.push({ kind: "skipped", title: e.title, detail: e.reason });
        break;
      case "failed":
        this.setTrack(e.id, { state: "failed", progress: 0, detail: e.message });
        this.push({ kind: "error", title: e.context, detail: e.message });
        break;
    }
  }

}

function sameList(a: string[], b: string[]): boolean {
  if (a === b) return true;
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}

/** A Tidal track to play, with what guests see of it while sharing. */
function streamItem(h: Hit, local: string | null): StreamItem {
  return { id: h.id, title: h.title, local, artist: h.artist ?? "", album: h.album ?? "", cover_url: h.cover_url, duration_ms: h.duration_ms, explicit: h.explicit };
}

/** Light / dark / follow the system; also the native title bar. */
const darkQuery = typeof matchMedia === "function" ? matchMedia("(prefers-color-scheme: dark)") : null;
let lastTheme: Theme = "system";
darkQuery?.addEventListener("change", () => lastTheme === "system" && setMode("system"));

function setMode(t: Theme) {
  // the resolved light / dark, for the skins (they have no "system" of their own)
  document.documentElement.dataset.mode = t === "system" ? (darkQuery?.matches === false ? "light" : "dark") : t;
}

/** Light / dark from the setting, unless the skin fixes it; `skin` dresses the whole app. */
export function applyTheme(theme: Theme | undefined, skin: PlayerLayout = "classic") {
  const t: Theme = fixedTheme(skin) ?? theme ?? "system";
  const root = document.documentElement;
  if (t === "system") delete root.dataset.theme;
  else root.dataset.theme = t;
  root.dataset.skin = skin;
  lastTheme = t;
  setMode(t);
  if ("__TAURI_INTERNALS__" in window) {
    import("@tauri-apps/api/window")
      .then(({ getCurrentWindow }) => getCurrentWindow().setTheme(t === "system" ? null : t))
      .catch(() => {});
  }
}

export const app = new AppState();

/** Groups the library into albums (album artist + album, or folder). */
export interface Album {
  key: string;
  title: string;
  artist: string;
  year: number | null;
  tracks: Track[];
}

function folder(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 2] ?? "";
}

export function albumKey(t: Track): string {
  return `${t.albumArtist.toLowerCase()}\u0000${(t.album || folder(t.path)).toLowerCase()}`;
}

export function groupAlbums(tracks: Track[]): Album[] {
  const map = new Map<string, Album>();
  for (const t of tracks) {
    const key = albumKey(t);
    let a = map.get(key);
    if (!a) {
      a = { key, title: t.album || folder(t.path), artist: t.albumArtist || t.artist, year: t.year, tracks: [] };
      map.set(key, a);
    }
    a.tracks.push(t);
    a.year ??= t.year;
  }
  for (const a of map.values()) {
    a.tracks.sort((x, y) => x.discNumber - y.discNumber || x.trackNumber - y.trackNumber || x.title.localeCompare(y.title));
  }
  return [...map.values()].sort((a, b) => a.artist.localeCompare(b.artist) || a.title.localeCompare(b.title));
}

export interface Artist {
  name: string;
  albums: number;
  tracks: Track[];
}

export function groupArtists(tracks: Track[]): Artist[] {
  const map = new Map<string, Track[]>();
  for (const t of tracks) {
    const name = (t.albumArtist || t.artist).trim();
    map.set(name, [...(map.get(name) ?? []), t]);
  }
  return [...map.entries()]
    .map(([name, list]) => ({ name, albums: new Set(list.map(albumKey)).size, tracks: list }))
    .sort((a, b) => a.name.localeCompare(b.name));
}

export function filterTracks(tracks: Track[], query: string): Track[] {
  const q = query.trim().toLowerCase();
  if (!q) return tracks;
  return tracks.filter((t) =>
    [t.title, t.artist, t.album, t.albumArtist].some((field) => field.toLowerCase().includes(q)),
  );
}
