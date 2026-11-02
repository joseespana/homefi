// The AI assistant's conversation (or the guided wizard), shared by the full
// "Set up my sound" page and the inline "Ask the AI" panel on the Sound screen:
// one session, one list of messages, whichever screen draws it.
import { tick } from "svelte";
import { api, errorText, type AiProviderId, type AiProviderInfo, type AssistantTurn, type EqProfile, type EqProposal, type TasteChange, type TasteRule } from "./api";
import { eq, uiLanguage } from "./eq.svelte";
import { ruleWords } from "./sound.svelte";
import { app } from "./state.svelte";

export type Msg = {
  id: number;
  from: "bot" | "me" | "error";
  text: string;
  proposal?: EqProposal;
  before?: EqProfile | null;
  tasteProposal?: TasteRule;
  settled?: boolean;
  retry?: () => void;
};

/** What the answer to a proposal's "Apply" button says, in the language of the chat. */
const APPLY_REPLY = { en: "Yes, apply it", es: "Sí, aplícalo" } as const;
const APPLY_EQ_REPLY = { en: "Apply", es: "Aplicar" } as const;
/** Recent turns the inline panel keeps on screen. */
export const INLINE_TURNS = 4;

class AssistantStore {
  readonly lang = uiLanguage();
  providers = $state.raw<AiProviderInfo[]>([]);
  keys = $state.raw<Partial<Record<AiProviderId, boolean>>>({});
  loaded = $state(false);

  phase = $state<"intro" | "chat">("intro");
  guided = $state(true);
  notes = $state("");
  messages = $state.raw<Msg[]>([]);
  quick = $state.raw<string[]>([]);
  thinking = $state(false);
  done = $state(false);
  applied = $state.raw<EqProposal | null>(null);
  /** Sounds / loudness the conversation changed (already saved). */
  tasteSaved = $state<string | null>(null);
  /** The sound "Ask the AI to refine" opened the assistant for. */
  focus = $state<string | null>(null);
  private ids = 0;
  private asked = false;

  get ai() {
    return app.settings?.ai;
  }
  get provider(): AiProviderInfo | null {
    return this.providers.find((p) => p.id === this.ai?.provider) ?? null;
  }
  /** A provider is chosen and has its key (or needs none). */
  get ready(): boolean {
    const p = this.provider;
    return !!p && (!p.needs_key || !!this.keys[p.id]);
  }
  /** The newest message that offers a sound or a profile not settled yet. */
  get pending(): boolean {
    return this.messages.some((m) => (m.proposal || m.tasteProposal) && !m.settled);
  }
  /** With a proposal card on the table its own Apply button stands for the "Apply" reply. */
  get chips(): string[] {
    return this.pending ? this.quick.filter((q) => !/^(apply|aplicar)$/i.test(q.trim())) : this.quick;
  }

  /** Providers and keys, once (the screens that need to know if the AI is set up). */
  async loadStatus(force = false) {
    if (this.asked && !force) return;
    this.asked = true;
    try {
      const [p, k] = await Promise.all([api.aiProviders(), api.aiKeyStatus()]);
      this.providers = p;
      this.keys = k;
    } catch {
      // without the list the AI counts as not set up
    } finally {
      this.loaded = true;
    }
  }

  private push(m: Omit<Msg, "id">) {
    this.messages = [...this.messages, { ...m, id: ++this.ids }];
  }

  /** One line for what the turn saved and removed (removed rules are known by id only). */
  private describe(c: TasteChange): string | null {
    const saved = c.upsert.map((r) => `${r.label} (${ruleWords(r)})`);
    const removed = c.remove.map((id) => app.settings?.taste?.rules.find((r) => r.id === id)?.label ?? "a sound");
    return [saved.length ? saved.join(" · ") : null, removed.length ? `Removed: ${removed.join(" · ")}` : null].filter(Boolean).join(" · ") || null;
  }

  private beforeOf(output: string): EqProfile | null {
    const p = app.settings?.eq.perOutput[output] ?? app.settings?.eq.defaultProfile ?? null;
    return p && JSON.parse(JSON.stringify(p));
  }

  /** Shows a turn: its text, the proposals on the table and what it saved. */
  handle(turn: AssistantTurn) {
    const p = turn.proposal;
    const shown = p && !turn.confirmed ? p : undefined;
    // "before" is what the output plays with now, kept as it was when proposed
    this.push({ from: "bot", text: turn.text, proposal: shown, before: shown ? this.beforeOf(shown.output_id) : null, tasteProposal: turn.taste_proposal ?? undefined });
    this.quick = turn.quick_replies;
    this.done = turn.done;
    if (turn.confirmed && p) {
      this.applied = p;
      eq.target = p.output_id;
    }
    if (turn.taste_change) this.tasteSaved = this.describe(turn.taste_change);
    if (turn.loudness) this.tasteSaved = [this.tasteSaved, `Loudness: ${turn.loudness.mode}`].filter(Boolean).join(" · ");
  }

  private async run(action: () => Promise<AssistantTurn>) {
    this.thinking = true;
    this.quick = [];
    try {
      this.handle(await action());
    } catch (e) {
      this.push({ from: "error", text: errorText(e), retry: () => this.run(action) });
    } finally {
      this.thinking = false;
      await tick();
    }
  }

  private reset(asGuided: boolean) {
    this.guided = asGuided;
    this.phase = "chat";
    this.messages = [];
    this.done = false;
    this.applied = null;
    this.tasteSaved = null;
  }

  /** Starts a conversation; `notes` is what the user already said, `focus` a sound to refine. */
  start(asGuided: boolean, notes: string = this.notes) {
    this.reset(asGuided);
    const f = asGuided ? null : this.focus;
    this.focus = null;
    return this.run(() => api.eqAssistantStart(asGuided, this.lang, notes.trim() || null, f));
  }

  /** The inline panel: the first words start the session (with them as its notes), the next ones reply. */
  ask(text: string) {
    const t = text.trim();
    if (!t || this.thinking) return;
    if (this.phase !== "chat" || this.done) {
      this.notes = t;
      this.reset(false);
      this.push({ from: "me", text: t });
      return this.run(() => api.eqAssistantStart(false, this.lang, t, null));
    }
    return this.send(t);
  }

  send(reply: string) {
    const t = reply.trim();
    if (!t || this.thinking) return;
    // answering settles the proposal on the table
    this.messages = this.messages.map((m) => (m.proposal || m.tasteProposal ? { ...m, settled: true } : m));
    this.push({ from: "me", text: t });
    return this.run(() => api.eqAssistantReply(t));
  }

  /** The "Apply" button of a sound proposal (answers yes to the assistant). */
  applyTaste() {
    return this.send(APPLY_REPLY[this.lang]);
  }
  applyProfile() {
    return this.send(APPLY_EQ_REPLY[this.lang]);
  }

  keepTalking(m: Msg) {
    this.messages = this.messages.map((x) => (x.id === m.id ? { ...x, settled: true } : x));
  }

  retry(m: Msg) {
    this.messages = this.messages.filter((x) => x.id !== m.id);
    m.retry?.();
  }

  /** A suggestion card's answer goes on here. */
  resume(turn: AssistantTurn, reply: string | null) {
    this.reset(!app.settings?.ai.provider);
    this.handle(turn);
    if (reply) this.send(reply);
  }

  /** The page opens with `notes` typed in, or starts refining `soundId` right away. */
  openPage(opts: { notes?: string; focus?: string } = {}) {
    this.phase = "intro";
    if (opts.notes !== undefined) this.notes = opts.notes;
    if (opts.focus) this.focus = opts.focus;
    app.view = { name: "assistant" };
  }

  /** "Ask the AI to refine" a sound: the AI starts from its curve. */
  refine(rule: TasteRule) {
    this.openPage({ notes: `Refine “${rule.label}” (${ruleWords(rule)}).`, focus: rule.id });
  }

  restart() {
    api.eqAssistantClose().catch(() => {});
    this.phase = "intro";
    this.messages = [];
    this.quick = [];
    this.done = false;
    this.applied = null;
  }

  /** The conversation is over for good (the page left, the app closing). */
  close() {
    api.eqAssistantClose().catch(() => {});
  }
}

export const assistant = new AssistantStore();
