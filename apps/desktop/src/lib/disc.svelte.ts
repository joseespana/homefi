// Discs: the drives, the disc in the screen, the info the user types before ripping and the
// progress of a rip. Lives across navigation (a rip takes minutes; leaving the screen must not
// lose it). The product rules (secure reading, offsets, encoders, file names) are in
// homefi-disc; this only holds state and sends intents. Big or fast-changing data is
// `$state.raw` and replaced, never mutated.
import { api, discSettingsOf, errorText, formatTime, on, type DiscMatch, type DiscSettings, type DiscView, type DriveView, type RipFormat, type RipProgress, type RipRequest, type RipSummary } from "./api";
import { app, type View } from "./state.svelte";

/** What a blank field becomes (shown as the placeholder, so the user sees what will happen). */
export const UNKNOWN_ARTIST = "Unknown artist";
const DISC_PREFIX = "Disc ";
const TRACK_PREFIX = "Track ";
const TRACK_DIGITS = 2;
const SECTORS_PER_SECOND = 75;
/** A year is four digits. */
const YEAR_DIGITS = 4;

export const trackName = (n: number) => `${TRACK_PREFIX}${String(n).padStart(TRACK_DIGITS, "0")}`;

/** The seconds of a track from its sectors (CD audio runs at 75 sectors a second). */
export const trackMs = (startLba: number, endLba: number) => Math.round(((endLba - startLba) * 1000) / SECTORS_PER_SECOND);

export type RowPhase = "waiting" | "reading" | "encoding" | "verifying" | "done" | "failed";
export interface RowProgress {
  phase: RowPhase;
  /** 0..1 of the current phase. */
  fraction: number;
  rereads: number;
  suspicious: number;
  unreadable: number;
  message: string | null;
  path: string | null;
}
const WAITING: RowProgress = { phase: "waiting", fraction: 0, rereads: 0, suspicious: 0, unreadable: 0, message: null, path: null };

/** What the user types about the disc. Empty strings mean "use the placeholder". */
export interface DiscForm {
  artist: string;
  album: string;
  year: string;
  genre: string;
  coverFile: string | null;
  /** "Name the songs?": yes shows a title field per track; no names them Track 01, Track 02... */
  nameSongs: boolean;
  /** By track number - 1. */
  titles: string[];
}
const emptyForm = (): DiscForm => ({ artist: "", album: "", year: "", genre: "", coverFile: null, nameSongs: false, titles: [] });

class DiscStore {
  drives = $state.raw<DriveView[]>([]);
  /** The drive the screen shows (null: the first one with a disc). */
  focusId = $state<string | null>(null);
  /** The audio disc in the screen. */
  current = $state.raw<DiscView | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);
  /** By track number - 1; null until the ISRC read reaches the track. */
  isrcs = $state.raw<(string | null)[]>([]);
  form = $state<DiscForm>(emptyForm());
  /** Where the form's names came from, to tell the user ("From the disc (CD-Text)"). */
  filledFrom = $state<DiscMatch["source"] | null>(null);
  matches = $state.raw<DiscMatch[]>([]);
  looking = $state(false);
  lookupNote = $state<string | null>(null);
  selection = $state.raw<ReadonlySet<number>>(new Set());
  rows = $state.raw<Record<number, RowProgress>>({});
  ripping = $state(false);
  cancelling = $state(false);
  summary = $state.raw<RipSummary | null>(null);

  #started = false;
  /** Only the newest open may fill the screen. */
  #token = 0;
  #pending = new Map<number, RowProgress>();
  #frame = 0;

  /** Settings with defaults. */
  get settings(): DiscSettings {
    return discSettingsOf(app.settings);
  }

  /** The drive in the screen. */
  get drive(): DriveView | null {
    return this.drives.find((d) => d.id === this.focusId) ?? this.drives.find((d) => d.media !== "none") ?? this.drives[0] ?? null;
  }

  /** The Rail item: always, never, or while a disc is in. */
  get railVisible(): boolean {
    const rail = this.settings.railItem;
    return rail === "always" || (rail === "whenInserted" && (this.drives.some((d) => d.media !== "none") || this.ripping));
  }

  /** The album name when the field is left blank: "Disc 1", or the next number not used by this artist. */
  readonly placeholderAlbum = $derived.by(() => {
    const artist = (this.form.artist.trim() || UNKNOWN_ARTIST).toLowerCase();
    const used = new Set<number>();
    for (const t of app.tracks) {
      if (t.albumArtist.toLowerCase() !== artist) continue;
      const m = /^disc (\d+)$/i.exec(t.album.trim());
      if (m) used.add(Number(m[1]));
    }
    let n = 1;
    while (used.has(n)) n++;
    return `${DISC_PREFIX}${n}`;
  });

  /** Starts watching: the list of drives now and the insert / remove events. Once. */
  async init() {
    if (this.#started) return;
    this.#started = true;
    await this.refresh();
    on.disc((e) => this.#onEvent(e)).catch(() => {});
  }

  async refresh() {
    try {
      this.drives = await api.discDrives();
    } catch {
      this.drives = [];
    }
  }

  /** Opens the Disc screen on a drive and reads the disc in it. */
  async show(driveId?: string) {
    if (driveId) this.focusId = driveId;
    const here = app.view.name === "disc";
    app.view = { name: "disc" } satisfies View;
    // the screen reads the disc when it opens; already open, it reads the other drive now
    if (here) await this.enter();
  }

  /** The screen opened: look at the drives again and read the audio disc, if any. */
  async enter() {
    await this.refresh();
    const drive = this.drive;
    if (!drive || drive.media !== "audio") {
      if (!this.ripping) this.current = null;
      return;
    }
    if (this.current?.drive.id === drive.id && this.current.ids.musicbrainz) return;
    await this.open(drive.id);
  }

  async open(driveId: string) {
    const mine = ++this.#token;
    this.loading = true;
    this.error = null;
    try {
      const view = await api.discOpen(driveId);
      if (mine !== this.#token) return;
      this.focusId = view.drive.id;
      this.current = view;
      this.isrcs = [];
      this.matches = [];
      this.lookupNote = null;
      this.selection = new Set(view.toc.tracks.map((t) => t.number));
      this.rows = {};
      this.summary = null;
      this.form = emptyForm();
      this.filledFrom = null;
      if (view.cdText) this.apply(view.cdText);
    } catch (e) {
      if (mine === this.#token) this.error = errorText(e);
    } finally {
      if (mine === this.#token) this.loading = false;
    }
  }

  /** Fills the form from names found (the disc's CD-Text, or a lookup the user asked for). */
  apply(match: DiscMatch) {
    const titles: string[] = [];
    for (const t of match.tracks) titles[t.number - 1] = t.title;
    this.form = {
      ...this.form,
      artist: match.album.artist,
      album: match.album.title,
      year: match.album.year ? String(match.album.year) : "",
      genre: match.album.genre ?? "",
      nameSongs: match.tracks.some((t) => t.title.trim().length > 0),
      titles,
    };
    this.filledFrom = match.source;
  }

  /** "Look it up on MusicBrainz" - only offered when the setting is on. */
  async lookup() {
    const drive = this.current?.drive.id;
    if (!drive || this.looking || !this.settings.sources.musicbrainz) return;
    this.looking = true;
    this.lookupNote = null;
    try {
      const found = await api.discIdentify(drive, { cdText: false, tidal: false, musicbrainz: true, ai: false });
      this.matches = found;
      if (found.length) this.apply(found[0]);
      else this.lookupNote = "MusicBrainz doesn't know this disc. Type the names yourself.";
    } catch (e) {
      this.lookupNote = errorText(e);
    } finally {
      this.looking = false;
    }
  }

  toggleTrack(n: number) {
    const next = new Set(this.selection);
    if (!next.delete(n)) next.add(n);
    this.selection = next;
  }

  selectAll(all: boolean) {
    this.selection = new Set(all ? (this.current?.toc.tracks.map((t) => t.number) ?? []) : []);
  }

  /** The year as a number; null when blank or not a year. */
  get yearValue(): number | null {
    const y = this.form.year.trim();
    return y.length === YEAR_DIGITS && /^\d+$/.test(y) ? Number(y) : null;
  }

  /** What the files will be called: the typed value, or the placeholder. */
  resolved(): { artist: string; album: string; titleOf: (n: number) => string } {
    const f = this.form;
    return {
      artist: f.artist.trim() || UNKNOWN_ARTIST,
      album: f.album.trim() || this.placeholderAlbum,
      titleOf: (n) => (f.nameSongs && f.titles[n - 1]?.trim()) || trackName(n),
    };
  }

  /** The disc as the user filled it in, blanks resolved, for the rip. */
  #meta(view: DiscView): DiscMatch {
    const r = this.resolved();
    const f = this.form;
    return {
      source: this.filledFrom ?? "manual",
      confidence: 1,
      album: { title: r.album, artist: r.artist, year: this.yearValue, genre: f.genre.trim() || null, upc: this.current?.mcn ?? null, discNumber: 1, discTotal: 1, tidalAlbumId: null, mbid: null },
      tracks: view.toc.tracks.map((t) => ({ number: t.number, title: r.titleOf(t.number), artists: [r.artist], isrc: this.isrcs[t.number - 1] ?? null, tidalTrackId: null })),
      cover: null,
      note: null,
    };
  }

  /** Writes a setting. The quality panel and Settings → Discs are the same values. */
  async save(patch: Partial<DiscSettings>) {
    const settings = app.settings;
    if (!settings) return;
    try {
      app.settings = await api.setSettings({ ...settings, disc: { ...discSettingsOf(settings), ...patch } });
    } catch (e) {
      app.note(errorText(e), "error");
    }
  }

  async rip() {
    const view = this.current;
    if (!view || this.ripping || !this.selection.size) return;
    const tracks = view.toc.tracks.map((t) => t.number).filter((n) => this.selection.has(n));
    const request: RipRequest = { drive: view.drive.id, tracks, meta: this.#meta(view), format: this.settings.format, coverFile: this.form.coverFile };
    this.ripping = true;
    this.cancelling = false;
    this.error = null;
    this.summary = null;
    this.rows = Object.fromEntries(tracks.map((n) => [n, WAITING]));
    try {
      this.summary = await api.discRip(request, (p) => this.#onProgress(p));
    } catch (e) {
      if (!this.cancelling) this.error = errorText(e);
    } finally {
      this.#flushNow();
      this.ripping = false;
      this.cancelling = false;
      this.refresh();
    }
  }

  /** Back to the form after a rip ("Rip again"). */
  startOver() {
    if (this.ripping) return;
    this.rows = {};
    this.summary = null;
  }

  async cancel() {
    if (!this.ripping || this.cancelling) return;
    const ok = await app.confirm({
      title: "Stop ripping?",
      text: "The songs already saved stay in your library. The one in progress is thrown away.",
      confirmLabel: "Stop ripping",
      cancelLabel: "Keep ripping",
      danger: true,
    });
    if (!ok || !this.ripping) return;
    this.cancelling = true;
    await api.discCancel().catch(() => {});
  }

  async eject() {
    const id = this.current?.drive.id ?? this.drive?.id;
    if (!id || this.ripping) return;
    try {
      await api.discEject(id);
      this.current = null;
      this.rows = {};
      this.summary = null;
    } catch (e) {
      this.error = errorText(e);
    }
    await this.refresh();
  }

  /** The ripped album in the library, once the rescan has brought it in. */
  showInLibrary() {
    const album = this.#meta(this.current!).album;
    const hit = app.tracks.find((t) => t.album === album.title && t.albumArtist.toLowerCase() === album.artist.toLowerCase());
    app.view = hit ? { name: "album", key: `${hit.albumArtist.toLowerCase()}\u0000${hit.album.toLowerCase()}` } : { name: "albums" };
  }

  #onEvent(e: Parameters<Parameters<typeof on.disc>[0]>[0]) {
    if (e.type === "isrcs") {
      if (this.current?.drive.id === e.drive.id) this.isrcs = e.isrcs;
      return;
    }
    this.refresh().then(() => {
      if (e.type === "removed") {
        if (this.current?.drive.id === e.drive.id && !this.ripping) this.current = null;
        return;
      }
      const mode = this.settings.openOnInsert;
      if (mode === "off" || this.ripping) return;
      if (mode === "open") return void this.show(e.drive.id);
      app.note(insertedText(e.drive), "info", "disc", { action: { label: "Open", run: () => void this.show(e.drive.id) }, seconds: 10 });
    });
  }

  /** Progress comes many times a second: keep the newest per track and draw once per frame. */
  #onProgress(p: RipProgress) {
    if (p.type === "finished") return;
    const before = this.#pending.get(p.track) ?? this.rows[p.track] ?? WAITING;
    let next: RowProgress = before;
    switch (p.type) {
      case "started":
        next = { ...before, phase: "reading", fraction: 0 };
        break;
      case "reading":
        next = { ...before, phase: "reading", fraction: p.totalSectors ? p.doneSectors / p.totalSectors : 0, rereads: p.rereads, suspicious: p.suspiciousSamples };
        break;
      case "encoding":
        next = { ...before, phase: "encoding", fraction: p.percent / 100 };
        break;
      case "verifying":
        next = { ...before, phase: "verifying", fraction: 1 };
        break;
      case "trackDone":
        next = { ...before, phase: "done", fraction: 1, rereads: p.read.rereads, suspicious: p.read.suspiciousSamples, unreadable: p.read.unreadableSamples, path: p.path };
        break;
      case "trackFailed":
        next = { ...before, phase: "failed", message: p.message };
        break;
    }
    this.#pending.set(p.track, next);
    // a finished or failed track is drawn at once; the rest wait for the frame
    if (next.phase === "done" || next.phase === "failed") this.#flushNow();
    else if (!this.#frame) this.#frame = requestAnimationFrame(() => this.#flushNow());
  }

  #flushNow() {
    cancelAnimationFrame(this.#frame);
    this.#frame = 0;
    if (!this.#pending.size) return;
    this.rows = { ...this.rows, ...Object.fromEntries(this.#pending) };
    this.#pending.clear();
  }
}

/** "Audio CD inserted · 12 tracks · 54:12" or what is wrong with the disc. */
export function insertedText(drive: DriveView): string {
  switch (drive.media) {
    case "audio":
      return `Audio CD inserted · ${drive.tracks ?? 0} tracks${drive.lengthMs ? ` · ${formatTime(drive.lengthMs)}` : ""}`;
    case "protected":
      return "Disc inserted · it is copy-protected";
    case "data":
      return "Disc inserted · it has files on it";
    default:
      return "Disc inserted";
  }
}

/** The formats as the quality panel names them. */
export function formatLabel(f: RipFormat): string {
  switch (f.kind) {
    case "flac":
      return `FLAC level ${f.level}`;
    case "wav":
      return "WAV";
    case "alac":
      return "ALAC";
    case "mp3":
      return f.mp3 === "v0" ? "MP3 V0" : "MP3 320";
    case "aac":
      return `AAC ${f.kbps}`;
    case "opus":
      return `Opus ${f.kbps}`;
  }
}

export const disc = new DiscStore();
