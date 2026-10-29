<script lang="ts">
  // "Set up my sound": one chat for both the AI assistant (the user's own key)
  // and the guided wizard. A confirmed proposal is already saved and applied by
  // the backend; then we jump to the equalizer.
  import { onDestroy, tick, untrack } from "svelte";
  import { api, errorText, type AiProviderId, type AiProviderInfo, type AssistantTurn, type EqProfile, type EqProposal, type TasteRule } from "../../lib/api";
  import { eq, uiLanguage } from "../../lib/eq.svelte";
  import { fade, rise } from "../../lib/motion";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";
  import EqGraph from "./EqGraph.svelte";
  import SuggestionCard from "../sound/SuggestionCard.svelte";
  import TasteProposal from "../sound/TasteProposal.svelte";
  import { ruleWords, sound } from "../../lib/sound.svelte";

  type Msg = { id: number; from: "bot" | "me" | "error"; text: string; proposal?: EqProposal; before?: EqProfile | null; tasteProposal?: TasteRule; settled?: boolean; retry?: () => void };

  const lang = uiLanguage();
  let providers = $state<AiProviderInfo[]>([]);
  let keys = $state<Partial<Record<AiProviderId, boolean>>>({});
  let loaded = $state(false);
  $effect(() => {
    Promise.all([api.aiProviders(), api.aiKeyStatus()])
      .then(([p, k]) => ((providers = p), (keys = k)))
      .catch(() => {})
      .finally(() => (loaded = true));
  });
  const ai = $derived(app.settings?.ai);
  const provider = $derived(providers.find((p) => p.id === ai?.provider) ?? null);
  const aiReady = $derived(!!provider && (!provider.needs_key || !!keys[provider.id]));

  let phase = $state<"intro" | "chat">("intro");
  let guided = $state(true);
  let notes = $state("");
  let messages = $state<Msg[]>([]);
  let quick = $state<string[]>([]);
  let thinking = $state(false);
  let done = $state(false);
  let applied = $state<EqProposal | null>(null);
  /** Taste layers / loudness the conversation changed (already saved). */
  let tasteSaved = $state<string | null>(null);
  let text = $state("");
  let ids = 0;
  let list = $state<HTMLElement | null>(null);
  let input = $state<HTMLInputElement | null>(null);
  let jump: ReturnType<typeof setTimeout> | null = null;

  const push = (m: Omit<Msg, "id">) => (messages = [...messages, { ...m, id: ++ids }]);

  function handle(turn: AssistantTurn) {
    const p = turn.proposal;
    // the card repeats the explanation only when the text doesn't say it already
    const shown = p && !turn.confirmed ? p : undefined;
    // "before" is what the output plays with now, kept as it was when proposed
    push({ from: "bot", text: turn.text, proposal: shown, before: shown ? beforeOf(shown.output_id) : null, tasteProposal: turn.taste_proposal ?? undefined });
    quick = turn.quick_replies;
    done = turn.done;
    if (turn.confirmed && p) {
      applied = p;
      eq.target = p.output_id;
      // a taste offer may follow: stay for it
      if (turn.done) jump = setTimeout(() => openEq(), 2200);
    }
    if (turn.taste) {
      const key = (r: TasteRule) => `${r.id}:${JSON.stringify(r.tilt)}:${JSON.stringify(r.bands ?? [])}`;
      const before = new Set(rulesBefore.map(key));
      const changed = turn.taste.rules.filter((r) => !before.has(key(r)));
      tasteSaved = changed.length ? changed.map((r) => `${r.label} (${ruleWords(r)})`).join(" · ") : "Your taste layers were updated";
    }
    if (turn.loudness) tasteSaved = [tasteSaved, `Loudness: ${turn.loudness.mode}`].filter(Boolean).join(" · ");
  }

  // a suggestion card's answer goes on here; "Sound for this…" leaves its notes
  $effect(() => {
    const h = sound.handoff;
    if (!h) return;
    untrack(() => {
      sound.handoff = null;
      guided = !app.settings?.ai.provider;
      phase = "chat";
      messages = [];
      done = false;
      applied = null;
      tasteSaved = null;
      rulesBefore = JSON.parse(JSON.stringify(app.settings?.taste?.rules ?? []));
      handle(h.turn);
      if (h.reply) send(h.reply);
    });
  });
  $effect(() => {
    const n = sound.assistantNotes;
    if (n) untrack(() => ((notes = n), (sound.assistantNotes = null), (phase = "intro")));
  });
  // "Ask the AI to refine" a rule: the AI starts from its curve right away
  let focus = $state<string | null>(null);
  $effect(() => {
    const f = sound.assistantFocus;
    if (f) untrack(() => ((focus = f), (sound.assistantFocus = null), (phase = "intro")));
  });
  $effect(() => {
    if (focus && loaded && aiReady && phase === "intro") untrack(() => start(false));
  });

  /** The rules before the last request, to say which ones the turn changed. */
  let rulesBefore: TasteRule[] = [];
  async function run(action: () => Promise<AssistantTurn>) {
    rulesBefore = JSON.parse(JSON.stringify(app.settings?.taste?.rules ?? []));
    thinking = true;
    quick = [];
    try {
      handle(await action());
    } catch (e) {
      push({ from: "error", text: errorText(e), retry: () => run(action) });
    } finally {
      thinking = false;
      await tick();
      input?.focus();
    }
  }

  function start(asGuided: boolean) {
    guided = asGuided;
    phase = "chat";
    messages = [];
    done = false;
    applied = null;
    tasteSaved = null;
    const f = asGuided ? null : focus;
    focus = null;
    run(() => api.eqAssistantStart(asGuided, lang, notes.trim() || null, f));
  }

  function send(reply: string) {
    const t = reply.trim();
    if (!t || thinking) return;
    text = "";
    // answering settles the proposal on the table
    messages = messages.map((m) => (m.proposal || m.tasteProposal ? { ...m, settled: true } : m));
    push({ from: "me", text: t });
    run(() => api.eqAssistantReply(t));
  }

  function keepTalking(m: Msg) {
    messages = messages.map((x) => (x.id === m.id ? { ...x, settled: true } : x));
    input?.focus();
  }

  function openEq() {
    if (jump) clearTimeout(jump);
    app.view = { name: "eq", output: applied?.output_id };
  }

  function restart() {
    api.eqAssistantClose().catch(() => {});
    phase = "intro";
    messages = [];
    quick = [];
    done = false;
    applied = null;
  }

  // with a proposal card on the table its own Apply button stands for the "Apply" reply
  const pending = $derived(messages.some((m) => (m.proposal || m.tasteProposal) && !m.settled));
  const chips = $derived(pending ? quick.filter((q) => !/^(apply|aplicar)$/i.test(q.trim())) : quick);

  function beforeOf(output: string): EqProfile | null {
    const p = app.settings?.eq.perOutput[output] ?? app.settings?.eq.defaultProfile ?? null;
    return p && JSON.parse(JSON.stringify(p));
  }

  // the newest message stays in view
  $effect(() => {
    messages.length;
    thinking;
    // the page (not the log) scrolls: bring the last message above the composer
    tick().then(() => requestAnimationFrame(() => list?.lastElementChild?.scrollIntoView({ block: "end", behavior: "smooth" })));
  });

  onDestroy(() => {
    if (jump) clearTimeout(jump);
    api.eqAssistantClose().catch(() => {});
  });
</script>

<div class="assistant">
  <header class="head">
    <button class="icon-btn" title="Back to the equalizer" aria-label="Back to the equalizer" onclick={() => (app.view = { name: "eq" })}><Icon name="back" /></button>
    <div class="titles">
      <h1>Set up my sound</h1>
      {#if phase === "chat"}
        <p class="muted small">{guided ? "Guided setup · a few quick questions" : `Chatting with ${provider?.name ?? "your AI"}`}</p>
      {/if}
    </div>
    {#if phase === "chat"}<button class="btn tonal" onclick={restart}><Icon name="refresh" size={16} /> Start over</button>{/if}
  </header>

  {#if phase === "intro"}
    <section class="intro" in:rise>
      <div class="hero"><Icon name="ai" size={30} /></div>
      <h2>Your headphones, your taste</h2>
      <p class="muted">Tell HomeFi what you listen with and how you like it. It finds the measured correction for your headphones, adds your taste and shows you the curve before anything changes.</p>
      <label class="notes">
        <span>Anything to tell first? <small class="muted">optional</small></span>
        <textarea bind:value={notes} rows="2" maxlength="500" placeholder="e.g. Sennheiser HD 650 on a USB DAC, I listen to jazz and find them a bit dark"></textarea>
      </label>
      {#if !loaded}
        <p class="muted small">Checking your AI setup…</p>
      {:else if aiReady}
        <div class="choices">
          <button class="btn primary" onclick={() => start(false)}><Icon name="ai" size={16} /> Chat with {provider?.name}</button>
          <button class="btn tonal" onclick={() => start(true)}>Guided questions instead</button>
        </div>
        <p class="muted tiny"><Icon name="shield" size={12} /> Only what the conversation needs goes to {provider?.name} (your outputs, the genres and artists you play most): never your music files or your Tidal session.</p>
      {:else}
        <div class="choices">
          <button class="btn primary" onclick={() => start(true)}>Start the guided setup</button>
        </div>
        <div class="no-ai">
          <Icon name="key" size={16} />
          <span>
            {#if provider && provider.needs_key}
              {provider.name} has no key yet.
            {:else}
              Prefer to just talk about it?
            {/if}
            Bring your own AI key (Claude, ChatGPT, Gemini…) or a local Ollama to chat instead.
            <button class="link" onclick={() => (app.view = { name: "settings", page: "ai" })}>Set up AI</button>
          </span>
        </div>
      {/if}
      <div class="suggest"><SuggestionCard /></div>
    </section>
  {:else}
    <section class="chat">
      <div class="log" bind:this={list} aria-live="polite">
        {#each messages as m (m.id)}
          <div class="msg {m.from}" in:rise={{ y: 8, duration: 220 }}>
            {#if m.from === "bot"}<span class="avatar" aria-hidden="true"><Icon name="ai" size={14} /></span>{/if}
            <div class="bubble-col">
              {#if m.text}
                <div class="bubble">
                  {#if m.from === "error"}<Icon name="alert" size={14} />{/if}
                  <span class="text">{m.text}</span>
                  {#if m.retry && m.id === messages[messages.length - 1].id}<button class="link" onclick={() => ((messages = messages.filter((x) => x.id !== m.id)), m.retry?.())}>Try again</button>{/if}
                </div>
              {/if}
              {#if m.tasteProposal}
                <TasteProposal rule={m.tasteProposal} open={!m.settled && !done} busy={thinking} onapply={() => send(lang === "es" ? "Sí, aplícalo" : "Yes, apply it")} onkeep={() => keepTalking(m)} />
              {/if}
              {#if m.proposal}
                {@const p = m.proposal}
                <div class="proposal">
                  <div class="ptitle">
                    <b>{p.profile.name}</b>
                    <span class="muted">for {p.output_id}</span>
                  </div>
                  <EqGraph profile={p.profile} compare={m.before ?? null} height={170} />
                  <div class="legend"><span class="now-line"></span> New <span class="old-line"></span> Now</div>
                  {#if p.explanation && !m.text.includes(p.explanation)}<p class="explain">{p.explanation}</p>{/if}
                  {#if !m.settled && !done}
                    <div class="pbuttons">
                      <button class="btn primary" disabled={thinking} onclick={() => send(lang === "es" ? "Aplicar" : "Apply")}><Icon name="check" size={16} /> Apply</button>
                      <button class="btn tonal" disabled={thinking} onclick={() => keepTalking(m)}>Keep talking</button>
                    </div>
                  {/if}
                </div>
              {/if}
            </div>
          </div>
        {/each}
        {#if thinking}
          <div class="msg bot" in:fade>
            <span class="avatar" aria-hidden="true"><Icon name="ai" size={14} /></span>
            <div class="bubble typing" aria-label="Thinking"><i></i><i></i><i></i></div>
          </div>
        {/if}
        {#if tasteSaved}
          <div class="success" in:rise>
            <Icon name="layers" size={18} />
            <span><b>Saved.</b> {tasteSaved}</span>
            <button class="btn tonal" onclick={() => (app.view = { name: "eq", tab: "taste" })}>Taste rules</button>
          </div>
        {/if}
        {#if applied}
          <div class="success" in:rise>
            <Icon name="check" size={18} />
            <span><b>Applied.</b> “{applied.profile.name}” is on for {applied.output_id}.{done ? " Opening the equalizer…" : ""}</span>
            <button class="btn tonal" onclick={openEq}>Open now</button>
          </div>
        {:else if done}
          <div class="ended" in:rise>
            <span class="muted">The conversation is over.</span>
            <button class="btn tonal" onclick={restart}>Start again</button>
            <button class="btn tonal" onclick={() => (app.view = { name: "eq" })}>Open the equalizer</button>
          </div>
        {/if}
      </div>

      {#if !done}
        {#if chips.length && !thinking}
          <div class="chips" in:fade>
            {#each chips as q}<button onclick={() => send(q)}>{q}</button>{/each}
          </div>
        {/if}
        <form class="composer" onsubmit={(e) => (e.preventDefault(), send(text))}>
          <input bind:this={input} bind:value={text} placeholder={guided ? "Type an answer, or pick one above" : "Write a message"} maxlength="1000" disabled={thinking} aria-label="Your answer" />
          <button class="btn primary" disabled={!text.trim() || thinking} aria-label="Send"><Icon name="send" size={16} /></button>
        </form>
      {/if}
    </section>
  {/if}
</div>

<style>
  .suggest {
    width: 100%;
    margin-top: 8px;
    text-align: left;
  }
  .assistant {
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 820px;
    margin: 0 auto;
    min-height: calc(100vh - 190px);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .titles {
    flex: 1;
    min-width: 0;
  }
  .titles p {
    margin: 2px 0 0;
  }
  .small {
    font-size: 12.5px;
  }
  .tiny {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    margin: 0;
  }
  .intro {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
    padding: 24px;
    border-radius: min(18px, var(--r-max));
    background: var(--glass);
    border: 1px solid var(--glass-border);
    backdrop-filter: blur(18px);
  }
  .intro h2 {
    margin: 0;
  }
  .intro p {
    margin: 0;
    max-width: 620px;
  }
  .hero {
    display: grid;
    place-items: center;
    width: 56px;
    height: 56px;
    border-radius: min(16px, var(--r-max));
    background: var(--gradient);
    color: #fff;
  }
  .notes {
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: 100%;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--muted);
  }
  textarea {
    font: inherit;
    font-weight: 400;
    font-size: 14px;
    color: var(--text);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: min(10px, var(--r-max));
    padding: 8px 10px;
    resize: vertical;
    outline: none;
  }
  .choices {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }
  .no-ai {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 10px 12px;
    border-radius: min(12px, var(--r-max));
    background: var(--surface-2);
    font-size: 13px;
    color: var(--muted);
  }
  .link {
    padding: 0 2px;
    color: var(--primary);
    font-weight: 600;
  }
  .link:hover {
    text-decoration: underline;
  }
  .chat {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 0;
  }
  .log {
    flex: 1;
    scroll-padding-bottom: 90px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 4px 2px 8px;
  }
  .msg,
  .success,
  .ended {
    scroll-margin-bottom: 90px;
  }
  .msg {
    display: flex;
    gap: 8px;
    align-items: flex-end;
    max-width: 85%;
  }
  .msg.me {
    align-self: flex-end;
  }
  .bubble-col {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .avatar {
    flex: none;
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: var(--gradient);
    color: #fff;
  }
  .bubble {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    padding: 10px 14px;
    border-radius: min(18px, var(--r-max)) min(18px, var(--r-max)) min(18px, var(--r-max)) min(6px, var(--r-max));
    background: var(--surface-2);
    border: 1px solid var(--border);
    line-height: 1.5;
    user-select: text;
  }
  .text {
    white-space: pre-wrap;
  }
  .me .bubble {
    border-radius: min(18px, var(--r-max)) min(18px, var(--r-max)) min(6px, var(--r-max)) min(18px, var(--r-max));
    background: var(--primary-strong);
    border-color: transparent;
    color: var(--on-primary-strong);
  }
  .error .bubble {
    flex-wrap: wrap;
    color: var(--danger);
    background: color-mix(in srgb, var(--danger) 10%, var(--surface));
    border-color: color-mix(in srgb, var(--danger) 35%, transparent);
  }
  .typing {
    gap: 4px;
    padding: 14px 16px;
  }
  .typing i {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--muted);
    animation: bounce calc(1.1s / var(--motion-speed)) infinite ease-in-out;
  }
  .typing i:nth-child(2) {
    animation-delay: 0.15s;
  }
  .typing i:nth-child(3) {
    animation-delay: 0.3s;
  }
  @keyframes bounce {
    0%,
    60%,
    100% {
      transform: translateY(0);
      opacity: 0.45;
    }
    30% {
      transform: translateY(-5px);
      opacity: 1;
    }
  }
  :global(:root[data-motion="subtle"]) .typing i {
    animation-name: blink;
  }
  @keyframes blink {
    50% {
      opacity: 1;
    }
  }
  .proposal {
    width: min(560px, 70vw);
    padding: 12px 14px;
    border-radius: min(16px, var(--r-max));
    background: var(--glass);
    border: 1px solid color-mix(in srgb, var(--accent-text) 40%, var(--glass-border));
    box-shadow: 0 10px 30px -18px color-mix(in srgb, var(--accent-text) 80%, transparent);
  }
  .ptitle {
    display: flex;
    gap: 8px;
    align-items: baseline;
    margin-bottom: 6px;
    font-size: 13px;
  }
  .legend {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 4px;
    font-size: 11.5px;
    color: var(--muted);
  }
  .now-line,
  .old-line {
    width: 18px;
    height: 0;
    border-top: 2.5px solid var(--accent-text);
  }
  .old-line {
    margin-left: 10px;
    border-top: 1.6px dashed var(--muted);
  }
  .explain {
    margin: 8px 0 0;
    font-size: 13px;
    line-height: 1.5;
  }
  .pbuttons {
    display: flex;
    gap: 8px;
    margin-top: 10px;
  }
  .success,
  .ended {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px;
    padding: 12px 14px;
    border-radius: min(14px, var(--r-max));
    background: color-mix(in srgb, var(--ok) 12%, var(--surface));
    border: 1px solid color-mix(in srgb, var(--ok) 40%, transparent);
  }
  .success > :global(svg) {
    color: var(--ok);
  }
  .success span {
    flex: 1;
    min-width: 200px;
  }
  .ended {
    background: var(--surface-2);
    border-color: var(--border);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .chips button {
    height: 34px;
    padding: 0 14px;
    border-radius: min(999px, var(--r-max));
    border: 1px solid color-mix(in srgb, var(--primary) 55%, var(--border));
    color: var(--primary);
    background: color-mix(in srgb, var(--primary) 8%, var(--surface));
    font-size: 13px;
    font-weight: 600;
    transition: background 0.15s;
  }
  .chips button:hover {
    background: color-mix(in srgb, var(--primary) 18%, var(--surface));
  }
  .composer {
    position: sticky;
    bottom: 0;
    display: flex;
    gap: 8px;
    padding: 8px;
    border-radius: min(16px, var(--r-max));
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
  }
  .composer input {
    flex: 1;
    min-width: 0;
    border: 0;
    background: transparent;
  }
  .composer .btn {
    width: 40px;
    padding: 0;
    justify-content: center;
  }
</style>
