// AI-made playlists, one session store in two modes: "Make a party playlist" (the answers, the
// optional chat with the user's own AI) and "Make a playlist with AI" (the chat is the whole input).
// Lives only while the screen is open (leaving it closes the session). The rules (clamping, the
// AI prompts, matching songs, the energy arc, the chat limit) are in homefi-core; this holds
// state and sends intents.
import { api, DEFAULT_PLAYLIST_SETTINGS, errorText, type ChatBrief, type PartyBrief, type PartyBuilt, type PartyProgress, type PartyTurn, type SongSeed } from "./api";
import { uiLanguage } from "./eq.svelte";
import { briefOf, chatBriefOf, playlists } from "./playlists.svelte";
import { app } from "./state.svelte";

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

/** Which playlist maker the session is for. */
export type MakerMode = "party" | "chat";

class PartyStore {
  mode = $state<MakerMode>("party");
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
  /** The saved playlist being rebuilt ("Change answers"). */
  replaceId = $state<string | null>(null);
  /** The session is open in the backend. */
  opened = $state(false);
  /** Opening failed (no AI or no key): the screen only asks to set one up. */
  unavailable = $state(false);

  #epoch = 0;
  #timer: ReturnType<typeof setTimeout> | null = null;
  #cancelling = false;

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

  /**
   * Starts a session with the settings' answers, or the saved playlist's when `replaceId` is given.
   * In chat mode the chat opens at once (its first line is instant, no AI call); with a `seed`
   * ("Playlist like this") the wish made from that song is then sent as the first message.
   */
  async open(replaceId: string | null = null, mode: MakerMode = "party", seed?: SongSeed) {
    this.#reset();
    const epoch = this.#epoch;
    this.mode = mode;
    this.replaceId = replaceId;
    const saved = replaceId ? playlists.list.find((p) => p.id === replaceId) : undefined;
    if (mode === "chat") {
      await this.#openChat(epoch, saved ? chatBriefOf(saved) : null, seed);
      return;
    }
    let brief = saved ? briefOf(saved) : null;
    if (!brief) {
      brief = await api.partyDefaultBrief().catch(briefFromSettings);
      if (epoch !== this.#epoch) return;
    }
    this.brief = brief;
    try {
      await api.partyOpen(brief, uiLanguage());
      if (epoch !== this.#epoch) return;
      this.opened = true;
    } catch {
      if (epoch === this.#epoch) this.unavailable = true;
    }
  }

  async #openChat(epoch: number, savedBrief: ChatBrief | null, seed?: SongSeed) {
    const brief = savedBrief ?? (await api.chatDefaultBrief().catch(chatBriefFromSettings));
    if (epoch !== this.#epoch) return;
    this.chatBrief = brief;
    try {
      await api.chatOpen(brief, uiLanguage());
      if (epoch !== this.#epoch) return;
      this.opened = true;
    } catch {
      if (epoch === this.#epoch) this.unavailable = true;
      return;
    }
    await this.talk();
    if (!seed) return;
    const wish = await api.chatSongWish(seed, uiLanguage()).catch(() => "");
    if (epoch === this.#epoch && wish) await this.send(wish);
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
    if (this.opened && this.mode === "party") await api.partySetBrief(this.brief).catch(() => {});
  }

  /** "Talk it over with the AI first". */
  async talk() {
    if (!this.opened || this.thinking || this.building) return;
    this.chatting = true;
    this.messages = [];
    this.quickReplies = [];
    await this.#turn(() => api.partyStart());
  }

  async send(text: string) {
    const t = text.trim();
    if (!t || !this.opened || this.thinking || this.building || this.chatClosed) return;
    this.messages = [...this.messages, { fromUser: true, text: t }];
    this.quickReplies = [];
    await this.#turn(() => api.partyReply(t));
  }

  async #turn(ask: () => Promise<PartyTurn>) {
    const epoch = this.#epoch;
    this.thinking = true;
    this.error = null;
    try {
      await this.#sync();
      const turn = await ask();
      if (epoch !== this.#epoch) return;
      this.messages = [...this.messages, { fromUser: false, text: turn.text }];
      this.quickReplies = turn.quick_replies;
      if (this.mode === "chat") this.chatBrief = turn.chat_brief ?? this.chatBrief;
    } catch (e) {
      if (epoch === this.#epoch) this.error = errorText(e);
    } finally {
      if (epoch === this.#epoch) this.thinking = false;
    }
  }

  /** Builds the playlist (again: it replaces the draft). */
  async build() {
    if (!this.opened || this.building || this.thinking) return;
    const epoch = this.#epoch;
    this.building = true;
    this.#cancelling = false;
    this.error = null;
    this.progress = null;
    try {
      await this.#sync();
      const built = await api.partyBuild(this.replaceId, (p) => epoch === this.#epoch && (this.progress = p));
      if (epoch !== this.#epoch) return;
      // rebuilding a saved playlist writes over it, like "Search again"
      if (this.replaceId) await playlists.save(built.playlist);
      this.draft = built;
    } catch (e) {
      if (epoch === this.#epoch && !this.#cancelling && errorText(e) !== CANCELLED) this.error = errorText(e);
    } finally {
      if (epoch === this.#epoch) {
        this.building = false;
        this.progress = null;
      }
    }
  }

  async cancel() {
    if (!this.building) return;
    this.#cancelling = true;
    await api.partyCancel().catch(() => {});
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

  /** Leaving the screen: stops a running build, drops the session, clears everything. */
  close() {
    const wasOpen = this.opened;
    const wasBuilding = this.building;
    this.#reset();
    if (wasBuilding) api.partyCancel().catch(() => {});
    if (wasOpen) api.partyClose().catch(() => {});
  }

  #reset() {
    this.#epoch++;
    if (this.#timer) clearTimeout(this.#timer);
    this.#timer = null;
    this.mode = "party";
    this.chatBrief = null;
    this.brief = briefFromSettings();
    this.messages = [];
    this.quickReplies = [];
    this.thinking = false;
    this.chatting = false;
    this.building = false;
    this.progress = null;
    this.draft = null;
    this.error = null;
    this.replaceId = null;
    this.opened = false;
    this.unavailable = false;
  }
}

export const party = new PartyStore();
