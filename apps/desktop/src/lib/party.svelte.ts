// AI-made playlists. Every chat is a `Conversation` (one session in the backend, by its id):
// "Make a party playlist" (the answers, the optional chat with the user's own AI) and "Make a
// playlist with AI" (the chat is the whole input, also "Playlist like this"). They live in the
// `conversations` list for the whole app session: leaving the screen does not stop one, the AI
// chats menu brings you back. The rules (clamping, the AI prompts, matching songs, the energy
// arc, the chat limit) are in homefi-core; this holds state and sends intents.
import { api, DEFAULT_PLAYLIST_SETTINGS, errorText, type ChatBrief, type PartyBrief, type PartyBuilt, type PartyProgress, type PartyTurn, type SongSeed } from "./api";
import { evictable, isWorking, tooManyChatsText, type ChatStatus } from "./conversations";
import { uiLanguage } from "./eq.svelte";
import { briefOf, chatBriefOf, playlists } from "./playlists.svelte";
import { app } from "./state.svelte";

export type { ChatStatus } from "./conversations";

/** What the core answers to a build the user cancelled. */
const CANCELLED = "cancelled";
/** User turns in the chat (the core stops asking after these). */
export const MAX_CHAT_TURNS = 6;
/** User turns in the free chat (the chat is the whole input, and how a built playlist is refined). */
export const MAX_FREE_CHAT_TURNS = 12;
/** Quiet time before an edited answer is sent to the open session. */
const BRIEF_SYNC_MS = 300;
/** Longest "anything else" text (chars). */
export const MAX_NOTES_CHARS = 500;
/** How long the "playlist is ready" notice stays (seconds). */
const NOTICE_SECONDS = 8;
/** The title until the core names the conversation. */
const PLACEHOLDER_TITLE: Record<"party" | "chat", string> = { party: "Party playlist", chat: "Playlist with AI" };

export interface ChatMsg {
  fromUser: boolean;
  text: string;
}

/** The questions' defaults from the user's settings, for when the backend can't answer. */
function briefFromSettings(): PartyBrief {
  const s = { ...DEFAULT_PLAYLIST_SETTINGS, ...app.settings?.playlists };
  return { guests: "friends", ages: [], vibe: "dance", minutes: s.party_minutes, curve: s.party_curve, allow_explicit: s.party_allow_explicit, notes: "" };
}

/** The chat's defaults from the user's settings, for when the backend can't answer. */
function chatBriefFromSettings(): ChatBrief {
  const s = { ...DEFAULT_PLAYLIST_SETTINGS, ...app.settings?.playlists };
  return { wish: "", title: "", minutes: s.chat_minutes, curve: s.chat_curve, allow_explicit: s.chat_allow_explicit };
}

/** Which playlist maker the conversation is for. */
export type MakerMode = "party" | "chat";

/** What a conversation starts from: a saved playlist to rebuild, or a song ("Playlist like this"). */
export interface ConversationStart {
  replaceId?: string | null;
  seed?: SongSeed;
}

export class Conversation {
  readonly id = crypto.randomUUID();
  readonly createdMs = Date.now();
  readonly mode: MakerMode;
  /** The name shown in the AI chats menu; the core's once it has one. */
  title = $state("");
  /** The last turn, build or draft (the menu's "5 min ago"). */
  lastActivityMs = $state(Date.now());
  /** The brief as the AI understands it (chat mode); null until a chat is open. */
  chatBrief = $state.raw<ChatBrief | null>(null);
  brief = $state.raw<PartyBrief>(briefFromSettings());
  messages = $state.raw<ChatMsg[]>([]);
  quickReplies = $state.raw<string[]>([]);
  /** A chat turn is in flight. */
  thinking = $state(false);
  /** The chat is open. */
  chatting = $state(false);
  building = $state(false);
  progress = $state.raw<PartyProgress | null>(null);
  /** Built, not saved yet (or, when rebuilding a saved playlist, already saved over). */
  draft = $state.raw<PartyBuilt | null>(null);
  error = $state<string | null>(null);
  /** The last build failed with this (the chat stays usable). */
  buildFailed = $state<string | null>(null);
  /** The words of a message the AI couldn't answer: they go back to the field (and out of the chat). */
  retryText = $state<string | null>(null);
  /** The saved playlist being rebuilt ("Change answers"). */
  readonly replaceId: string | null;
  /** The session is open in the backend. */
  opened = $state(false);
  /** Opening failed (no AI or no key): the screen only asks to set one up. */
  unavailable = $state(false);
  /** Still finding out whether an AI is set up. */
  checking = $state(true);

  readonly #seed: SongSeed | undefined;
  #closed = false;
  #timer: ReturnType<typeof setTimeout> | null = null;
  #cancelling = false;

  constructor(mode: MakerMode, start: ConversationStart = {}) {
    this.mode = mode;
    this.replaceId = start.replaceId ?? null;
    this.#seed = start.seed;
    this.title = PLACEHOLDER_TITLE[mode];
  }

  /** User turns the chat allows in this mode. */
  get maxTurns() {
    return this.mode === "chat" ? MAX_FREE_CHAT_TURNS : MAX_CHAT_TURNS;
  }
  /** User turns so far; the chat ends at `maxTurns`. */
  get userTurns() {
    return this.messages.filter((m) => m.fromUser).length;
  }
  get chatClosed() {
    return this.chatting && this.userTurns >= this.maxTurns;
  }

  /** Where the conversation stands, in one value (the menu and the Rail's dot read it). */
  get status(): ChatStatus {
    if (this.building) return { kind: "building", fraction: this.fraction };
    if (this.thinking) return { kind: "thinking" };
    if (this.draft) return { kind: "ready" };
    if (this.buildFailed) return { kind: "failed", message: this.buildFailed };
    return { kind: "waiting" };
  }

  /** What the running build is doing, in words. */
  get statusText(): string {
    const ai = playlists.sources?.ai?.provider_name ?? "your AI";
    const p = this.progress;
    if (p?.kind === "resolving") return `Finding the songs: ${p.done} of ${p.total}`;
    return p && p.round >= 1 ? `Asking ${ai} for a few more songs…` : `${ai} is picking songs…`;
  }
  get fraction(): number | null {
    const p = this.progress;
    return p?.kind === "resolving" && p.total > 0 ? p.done / p.total : null;
  }

  #touch() {
    this.lastActivityMs = Date.now();
  }

  /**
   * Starts the session with the settings' answers, or the saved playlist's when `replaceId` is
   * given. In chat mode the chat opens at once (its first line is instant, no AI call); with a
   * seed ("Playlist like this") the wish made from that song is then sent as the first message.
   */
  async open() {
    const saved = this.replaceId ? await this.#savedPlaylist() : undefined;
    await playlists.loadSources();
    if (this.#closed) return;
    this.checking = false;
    if (!playlists.sources?.ai) {
      this.unavailable = true;
      return;
    }
    if (this.mode === "chat") {
      await this.#openChat(saved ? chatBriefOf(saved) : null);
      return;
    }
    let brief = saved ? briefOf(saved) : null;
    if (!brief) {
      brief = await api.partyDefaultBrief().catch(briefFromSettings);
      if (this.#closed) return;
    }
    this.brief = brief;
    try {
      this.#named(await api.partyOpen(this.id, brief, uiLanguage()));
      if (this.#closed) return;
      this.opened = true;
    } catch {
      if (!this.#closed) this.unavailable = true;
    }
  }

  async #savedPlaylist() {
    if (!playlists.loaded) await playlists.load();
    return playlists.list.find((p) => p.id === this.replaceId);
  }

  async #openChat(savedBrief: ChatBrief | null) {
    const brief = savedBrief ?? (await api.chatDefaultBrief().catch(chatBriefFromSettings));
    if (this.#closed) return;
    this.chatBrief = brief;
    try {
      this.#named(await api.chatOpen(this.id, brief, uiLanguage()));
      if (this.#closed) return;
      this.opened = true;
    } catch {
      if (!this.#closed) this.unavailable = true;
      return;
    }
    await this.talk();
    if (!this.#seed) return;
    const wish = await api.chatSongWish(this.#seed, uiLanguage()).catch(() => "");
    if (!this.#closed && wish) await this.send(wish);
  }

  /** The core's title for the conversation, when it has one. */
  #named(title: string | undefined) {
    if (title) this.title = title;
  }

  /** An answer changed: shown at once, sent to the session after a short pause. */
  setBrief(patch: Partial<PartyBrief>) {
    this.brief = { ...this.brief, ...patch };
    if (!this.opened) return;
    if (this.#timer) clearTimeout(this.#timer);
    this.#timer = setTimeout(() => void this.#sync(), BRIEF_SYNC_MS);
  }

  async #sync() {
    if (this.#timer) clearTimeout(this.#timer);
    this.#timer = null;
    // the chat's brief is built by the AI inside the session; there is nothing to send
    if (this.opened && this.mode === "party" && !this.#closed) this.#named(await api.partySetBrief(this.id, this.brief).catch(() => undefined));
  }

  /** "Talk it over with the AI first". */
  async talk() {
    if (!this.opened || this.thinking || this.building) return;
    this.chatting = true;
    this.messages = [];
    this.quickReplies = [];
    await this.#turn(() => api.partyStart(this.id));
  }

  async send(text: string) {
    const t = text.trim();
    if (!t || !this.opened || this.thinking || this.building || this.chatClosed) return;
    this.messages = [...this.messages, { fromUser: true, text: t }];
    this.quickReplies = [];
    const answered = await this.#turn(() => api.partyReply(this.id, t));
    // the core dropped it too: take it out of the chat and hand it back, so nothing is built from it
    if (!answered && !this.#closed && this.messages.at(-1)?.text === t) {
      this.messages = this.messages.slice(0, -1);
      this.retryText = t;
    }
  }

  /** One chat turn; false when the AI couldn't answer (the error says why). */
  async #turn(ask: () => Promise<PartyTurn>): Promise<boolean> {
    this.thinking = true;
    this.error = null;
    this.#touch();
    try {
      await this.#sync();
      const turn = await ask();
      if (this.#closed) return false;
      this.messages = [...this.messages, { fromUser: false, text: turn.text }];
      this.quickReplies = turn.quick_replies;
      if (this.mode === "chat") this.chatBrief = turn.chat_brief ?? this.chatBrief;
      this.#named(turn.title);
      return true;
    } catch (e) {
      if (!this.#closed) this.error = errorText(e);
      return false;
    } finally {
      if (!this.#closed) {
        this.thinking = false;
        this.#touch();
      }
    }
  }

  /** Builds the playlist (again: it replaces the draft). The build goes on if the screen is left. */
  async build() {
    if (!this.opened || this.building || this.thinking) return;
    this.building = true;
    this.#cancelling = false;
    this.error = null;
    this.buildFailed = null;
    this.progress = null;
    this.#touch();
    let failure: string | null = null;
    try {
      await this.#sync();
      const built = await api.partyBuild(this.id, this.replaceId, (p) => !this.#closed && (this.progress = p));
      if (this.#closed) return;
      // rebuilding a saved playlist writes over it, like "Search again"
      if (this.replaceId) await playlists.save(built.playlist);
      this.draft = built;
    } catch (e) {
      if (!this.#closed && !this.#cancelling && errorText(e) !== CANCELLED) failure = errorText(e);
    } finally {
      if (!this.#closed) {
        this.building = false;
        this.progress = null;
        this.buildFailed = failure;
        this.error = failure;
        this.#touch();
        this.#notifyDone(failure);
      }
    }
  }

  /** A finished or failed build while another screen is shown: a note with "Open" (the setting can turn it off). */
  #notifyDone(failure: string | null) {
    if (!this.draft && !failure) return;
    const on = { ...DEFAULT_PLAYLIST_SETTINGS, ...app.settings?.playlists }.chat_done_notice;
    if (!on || conversations.isShown(this.id)) return;
    const action = { label: "Open", run: () => conversations.show(this.id) };
    const key = `chat:${this.id}`;
    if (failure) app.note(`“${this.title}” didn't build: ${failure}`, "error", key, { seconds: NOTICE_SECONDS, action });
    else app.note(`“${this.title}” is ready`, "success", key, { seconds: NOTICE_SECONDS, action });
  }

  async cancel() {
    if (!this.building) return;
    this.#cancelling = true;
    await api.partyCancel(this.id).catch(() => {});
  }

  /** Keeps the draft in "Made by HomeFi"; returns its id. */
  async save(): Promise<string | null> {
    const draft = this.draft;
    if (!draft) return null;
    try {
      await playlists.save(draft.playlist);
    } catch (e) {
      this.error = errorText(e);
      return null;
    }
    this.draft = null;
    return draft.playlist.id;
  }

  discard() {
    this.draft = null;
    this.error = null;
  }

  /** Stops a running build, drops the session; whatever is still in flight is ignored. */
  close() {
    if (this.#closed) return;
    this.#closed = true;
    if (this.#timer) clearTimeout(this.#timer);
    this.#timer = null;
    app.dismissKey(`chat:${this.id}`);
    if (this.building) api.partyCancel(this.id).catch(() => {});
    api.partyClose(this.id).catch(() => {});
  }
}

/** Every open AI chat, for the whole app session (they are lost when the app closes). */
class Conversations {
  list = $state.raw<Conversation[]>([]);
  /** The AI chats menu (the Rail's entry) is open. */
  menuOpen = $state(false);

  /** The chats that count: ones without an AI to talk to are closed as their screen is left. */
  get live(): Conversation[] {
    return this.list.filter((c) => !c.unavailable);
  }
  /** Any chat is thinking or building (the Rail's dot). */
  get busy(): boolean {
    return this.list.some((c) => isWorking(c.status));
  }
  get max(): number {
    return { ...DEFAULT_PLAYLIST_SETTINGS, ...app.settings?.playlists }.chat_max_open;
  }

  get(id: string): Conversation | undefined {
    return this.list.find((c) => c.id === id);
  }

  /**
   * Registers a conversation and starts it (without waiting for the network). When the list is
   * full the oldest failed chat gives way; with none, null: a note says why and offers the menu.
   */
  start(mode: MakerMode, opts: ConversationStart = {}): Conversation | null {
    // the same saved playlist already has its chat open: that one, not a second
    const again = opts.replaceId ? this.list.find((c) => c.mode === mode && c.replaceId === opts.replaceId) : undefined;
    if (again) return again;
    if (this.live.length >= this.max) {
      const gone = evictable(this.live);
      if (gone) this.close(gone);
      else {
        app.note(tooManyChatsText(this.max), "error", "chats-cap", { action: { label: "Open chats", run: () => (this.menuOpen = true) } });
        return null;
      }
    }
    const c = new Conversation(mode, opts);
    this.list = [...this.list, c];
    void c.open();
    return c;
  }

  /** Starts a conversation and goes to its screen (nothing happens when the list is full). */
  begin(mode: MakerMode, opts: ConversationStart = {}) {
    const c = this.start(mode, opts);
    if (c) this.show(c.id);
  }

  /** Brings the chat's screen forward with its progress. */
  show(id: string) {
    const c = this.get(id);
    if (!c) return;
    this.menuOpen = false;
    app.view = { name: c.mode === "chat" ? "chat-playlist" : "party", conversation: id };
  }

  /** Whether the chat's own screen is what the user is looking at. */
  isShown(id: string): boolean {
    const v = app.view;
    return (v.name === "party" || v.name === "chat-playlist") && v.conversation === id;
  }

  /** From the menu, after saving, or when a chat without an AI is left. */
  close(id: string) {
    const c = this.get(id);
    if (!c) return;
    c.close();
    this.list = this.list.filter((x) => x.id !== id);
    if (!this.list.length) this.menuOpen = false;
  }
}

export const conversations = new Conversations();
