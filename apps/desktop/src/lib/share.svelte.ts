// Shared control and party mode (src-tauri/src/share): this computer as the hub,
// or as a guest of someone else's. Fed by the `share-*` events.
import {
  api,
  errorText,
  on,
  remoteCovers,
  type Collection,
  type GuestRequest,
  type GuestView,
  type HubView,
  type Member,
  type Nearby,
  type PlayerState,
  type QueueItem,
  type ShareResults,
  type SharePage,
  type ShareTrack,
  type Track,
} from "./api";
import { app } from "./state.svelte";

const IDLE_HUB: HubView = { running: false, starting: false, party: false, play_here: true, info: null, guests: [], pending: [], state: null, error: null };
const IDLE_GUEST: GuestView = {
  status: "idle",
  session_name: "",
  guest_id: "",
  host: "",
  port: 47800,
  state: null,
  position_ms: 0,
  error: null,
  play_along: true,
  playing_here: false,
  round_trip_ms: null,
  stream: null,
};

export type GuestTab = "player" | "room" | "search";

/** A queued song of someone's room as a player track (`room:<queue id>`). */
export function roomTrack(item: QueueItem): Track {
  return {
    path: `room:${item.queue_id}`,
    title: item.title,
    artist: item.artist,
    album: item.album,
    albumArtist: item.artist,
    trackNumber: 0,
    discNumber: 0,
    year: null,
    genre: "",
    durationMs: item.duration_ms ?? 0,
    format: "",
    sampleRate: null,
    bitDepth: null,
    lossless: false,
    quality: item.source.startsWith("tidal:") ? "Tidal" : "From the hub",
    isrc: null,
  };
}

class ShareStore {
  hub = $state<HubView>(IDLE_HUB);
  guest = $state<GuestView>(IDLE_GUEST);
  /** Hubs found on the network. */
  nearby = $state<Nearby[]>([]);
  /** Sessions the user said "Not now" to (host:port), until the app restarts. */
  dismissed = $state<Set<string>>(new Set());
  results = $state<ShareResults | null>(null);
  page = $state<SharePage | null>(null);
  searching = $state(false);
  /** The share panel (start sharing / the hub's guests / join). */
  panelOpen = $state(false);
  /** The join form: a found hub (only the code is missing) or a blank one. */
  joinForm = $state<{ host: string; port: number; sessionName: string; code: string } | null>(null);
  tab = $state<GuestTab>("player");
  /** Defaults from the backend: this computer's name and session name. */
  name = $state("");
  sessionName = $state("");
  /** Bumped 4 times a second in a room, so the hub's position moves between reports. */
  private tick = $state(0);
  private positionAt = 0;
  private timer: ReturnType<typeof setInterval> | null = null;
  private started = false;

  get inRoom(): boolean {
    return this.guest.status === "connected" && !!this.guest.state;
  }

  get sharing(): boolean {
    return this.hub.running;
  }

  /** Found hubs still worth offering. */
  get offers(): Nearby[] {
    if (this.inRoom || this.sharing) return [];
    return this.nearby.filter((n) => !this.dismissed.has(`${n.host}:${n.port}`));
  }

  async init() {
    if (this.started) return;
    this.started = true;
    try {
      const s = await api.shareStatus();
      this.hub = s.hub;
      this.setGuest(s.guest);
      this.nearby = s.nearby;
      this.name = s.name;
      this.sessionName = s.session_name;
    } catch (e) {
      console.warn("[homefi] share status", e);
    }
    on.shareHub((v) => (this.hub = v));
    on.shareGuest((v) => this.setGuest(v));
    on.shareNearby((list) => (this.nearby = list));
    on.shareResults((r) => {
      this.results = r;
      this.searching = false;
    });
    on.sharePage((p) => {
      this.page = p;
      this.searching = false;
    });
    on.shareDenied(() => (this.searching = false));
    on.shareNotice((n) => app.note(n.text, n.kind === "error" ? "error" : "info", undefined, { seconds: n.seconds, color: n.color ?? undefined }));
  }

  private setGuest(v: GuestView) {
    const was = this.guest.status;
    this.guest = v;
    this.positionAt = performance.now();
    for (const item of [v.state?.now_playing, ...(v.state?.queue ?? [])]) {
      if (item?.cover_url) remoteCovers.set(`room:${item.queue_id}`, item.cover_url);
    }
    if (v.status === "connected" && !this.timer) this.timer = setInterval(() => this.tick++, 250);
    if (v.status !== "connected" && this.timer) {
      clearInterval(this.timer);
      this.timer = null;
    }
    // the hub closed the connection (kicked, stopped) or it was lost
    if (was === "connected" && v.status === "disconnected") {
      app.note(v.error ?? "The connection to the hub was lost", "error");
      if (app.view.name === "guest") app.view = { name: "room" };
    }
  }

  // ------------------------------------------------------------ the room as a player

  /** Where the hub's song is now (interpolated between its reports). */
  get position(): number {
    this.tick;
    const st = this.guest.state;
    if (!st?.now_playing) return 0;
    const at = this.guest.position_ms + (st.playing ? performance.now() - this.positionAt : 0);
    const dur = st.now_playing.duration_ms;
    return Math.max(0, dur ? Math.min(at, dur) : at);
  }

  /** Everything in the room in play order: what plays now, then the queue. */
  get roomItems(): QueueItem[] {
    const st = this.guest.state;
    if (!st) return [];
    return [...(st.now_playing ? [st.now_playing] : []), ...st.queue];
  }

  /** The room as a `PlayerState`, so the room and its layouts draw it like the app's own player. */
  get roomPlayer(): PlayerState | null {
    const st = this.guest.state;
    if (!this.inRoom || !st) return null;
    const now = st.now_playing;
    return {
      status: now ? (st.playing ? "playing" : "paused") : "stopped",
      queue: this.roomItems.map((i) => `room:${i.queue_id}`),
      current: now ? 0 : null,
      position_ms: this.position,
      duration_ms: now?.duration_ms ?? null,
      volume: st.volume ?? app.player?.volume ?? 0.8,
      shuffle: false,
      repeat: "off",
      preferred_device: app.player?.preferred_device ?? null,
      stream: this.guest.playing_here ? this.guest.stream : null,
      error: null,
      track: now ? roomTrack(now) : null,
    };
  }

  roomTrackAt(path: string): Track | undefined {
    const item = this.roomItems.find((i) => `room:${i.queue_id}` === path);
    return item ? roomTrack(item) : undefined;
  }

  // ------------------------------------------------------------ what the hub allows

  get permissions() {
    return this.guest.state?.permissions;
  }
  get canToggle(): boolean {
    return !!this.permissions?.playback;
  }
  get canSkip(): boolean {
    return !!this.permissions && this.permissions.skip !== "nobody";
  }
  get votes(): boolean {
    return this.permissions?.skip === "vote";
  }
  get canPlayNow(): boolean {
    const p = this.permissions;
    return !!p && (p.playback || p.skip === "everyone");
  }
  canEdit(item: QueueItem): boolean {
    const edit = this.permissions?.edit;
    return edit === "all" || (edit === "own" && item.added_by.guest_id === this.guest.guest_id);
  }
  /** Why a song's menu is greyed out. */
  editHint(item: QueueItem): string | null {
    if (this.canEdit(item)) return null;
    return this.permissions?.edit === "own" ? "The hub only lets you change your own songs" : "The hub doesn’t let guests change the queue";
  }

  // ------------------------------------------------------------ guest actions

  private async send(message: GuestRequest) {
    try {
      await api.shareGuest(message);
    } catch (e) {
      app.note(errorText(e), "error");
    }
  }

  toggle() {
    if (this.canToggle) this.send({ type: "toggle" });
  }
  skip() {
    if (this.canSkip) this.send({ type: this.votes ? "vote_skip" : "skip" });
  }
  volume(value: number) {
    if (this.canToggle) this.send({ type: "volume", value: Math.max(0, Math.min(1, value)) });
  }
  playNow(item: QueueItem) {
    if (this.canPlayNow) this.send({ type: "play", queue_id: item.queue_id });
  }
  /** `to` among the songs waiting (0 = next). */
  move(item: QueueItem, to: number) {
    this.send({ type: "move", queue_id: item.queue_id, to: Math.max(0, to) });
  }
  async remove(item: QueueItem) {
    const ok = await app.confirm({ title: `Remove “${item.title}” from the queue?`, confirmLabel: "Remove", danger: true });
    if (ok) this.send({ type: "remove", queue_id: item.queue_id });
  }
  add(track: ShareTrack, next = false) {
    this.send({ type: "add", item: track, next });
    app.note(`“${track.title}” sent`, "success");
  }
  search(q: string) {
    if (!q.trim()) return;
    this.searching = true;
    this.page = null;
    this.send({ type: "search", q: q.trim() });
  }
  browse(c: Collection) {
    this.searching = true;
    this.send({ type: "browse", kind: c.kind, id: c.id });
  }
  closePage() {
    this.page = null;
  }
  playAlong(on: boolean) {
    api.sharePlayAlong(on);
  }

  async leave() {
    if (this.guest.status === "connected") {
      const ok = await app.confirm({ title: `Leave ${this.guest.session_name}?`, text: "You can join again later.", confirmLabel: "Leave", danger: true });
      if (!ok) return;
    }
    await api.shareLeave();
    this.results = null;
    this.page = null;
    if (app.view.name === "guest") app.view = { name: "room" };
  }

  /** Opens the room (the guest view). */
  open(tab?: GuestTab) {
    if (tab) this.tab = tab;
    app.view = { name: "guest" };
  }

  // ------------------------------------------------------------ joining

  /** The join form for a hub found nearby (only the code is missing), or blank. */
  askJoin(hub?: Nearby) {
    this.panelOpen = false;
    this.joinForm = hub ? { host: hub.host, port: hub.port, sessionName: hub.name, code: "" } : { host: "", port: 47800, sessionName: "", code: "" };
  }

  dismiss(hub: Nearby) {
    this.dismissed = new Set([...this.dismissed, `${hub.host}:${hub.port}`]);
  }

  async join(host: string, port: number, code: string, sessionName = "", name?: string, color?: string): Promise<boolean> {
    try {
      await api.shareJoin(host, port, code, name, color, sessionName);
      this.joinForm = null;
      this.panelOpen = false;
      this.open(app.settings?.share.guest_opens_player === false ? "room" : "player");
      return true;
    } catch (e) {
      if (errorText(e) !== "Cancelled") app.note(errorText(e), "error");
      return false;
    }
  }

  // ------------------------------------------------------------ hub actions

  async start(party: boolean) {
    try {
      this.hub = await api.shareStart(party);
    } catch (e) {
      app.note(errorText(e), "error");
    }
  }

  async stop() {
    const ok = await app.confirm({
      title: "End the session?",
      text: "Everyone is disconnected and the music stops on their devices.",
      confirmLabel: "Stop sharing",
      danger: true,
    });
    if (ok) await api.shareStop();
  }

  setParty(on: boolean) {
    api.shareSetParty(on);
  }

  setPlayHere(on: boolean) {
    api.shareSetPlayHere(on);
  }

  async kick(guest: Member) {
    const r = await app.ask({
      title: `Remove ${guest.name} from the session?`,
      text: "They can join again with the code.",
      check: "Also block this device until the session ends",
      confirmLabel: "Remove",
      danger: true,
    });
    if (r.ok) api.shareKick(guest.guest_id, r.checked);
  }

  approve(item: QueueItem) {
    api.shareApprove(item.queue_id);
  }

  async reject(item: QueueItem) {
    const ok = await app.confirm({ title: `Reject “${item.title}”?`, confirmLabel: "Reject", danger: true });
    if (ok) api.shareReject(item.queue_id);
  }
}

export const share = new ShareStore();
