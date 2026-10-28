<script lang="ts">
  // Settings → AI: bring your own key. The key goes to the OS keychain through
  // the backend and never comes back to the screen (only "saved / not saved").
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { untrack } from "svelte";
  import { api, errorText, type AiModel, type AiProviderBadge, type AiProviderId, type AiProviderInfo, type AiSettings, type Settings } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import Icon from "./Icon.svelte";

  let { settings, save }: { settings: Settings; save: (patch: Partial<Settings>) => Promise<void> | void } = $props();

  let providers = $state<AiProviderInfo[]>([]);
  let keys = $state<Partial<Record<AiProviderId, boolean>>>({});
  $effect(() => {
    api.aiProviders().then((p) => (providers = [...p].sort((a, b) => (a.rank || 99) - (b.rank || 99)))).catch(() => {});
    refreshKeys();
  });
  const refreshKeys = () => api.aiKeyStatus().then((k) => (keys = k)).catch(() => {});

  const ai = $derived<AiSettings>(settings.ai ?? { provider: "", model: "", base_url: "" });
  const provider = $derived(providers.find((p) => p.id === ai.provider) ?? null);
  const hasKey = $derived(!!provider && !!keys[provider.id]);
  const ready = $derived(!!provider && (!provider.needs_key || hasKey));
  const baseUrl = $derived(ai.base_url.trim() || null);
  const keyField = $derived(!!provider && (provider.needs_key || provider.id === "openai_compatible"));

  const badgeLabel: Record<AiProviderBadge, string> = {
    recommended: "Recommended",
    cheapest: "Cheapest",
    one_key_for_all: "One key for all",
    private_local: "Private, local",
  };

  function saveAi(patch: Partial<AiSettings>) {
    save({ ai: { ...ai, ...patch } });
  }
  function pick(id: AiProviderId | "") {
    if (id === ai.provider) return;
    key = "";
    result = null;
    saveAi({ provider: id, model: "", base_url: "" });
  }

  // ---------------------------------------------------------------- key
  let key = $state("");
  let busy = $state<"test" | "save" | "remove" | null>(null);
  let result = $state<{ ok: boolean; text: string } | null>(null);
  let testedKey: string | null = null;

  async function test(): Promise<boolean> {
    if (!provider) return false;
    const typed = key.trim() || null;
    busy = "test";
    try {
      await api.aiTestKey(provider.id, baseUrl, typed);
      testedKey = typed;
      result = { ok: true, text: typed ? "The key works. Save it to use it." : provider.needs_key ? "The saved key works." : "The server answers." };
      return true;
    } catch (e) {
      result = { ok: false, text: errorText(e) };
      return false;
    } finally {
      busy = null;
    }
  }

  async function saveKey() {
    if (!provider) return;
    const typed = key.trim();
    if (!typed) return;
    if (testedKey !== typed && !(await test())) return;
    busy = "save";
    try {
      await api.aiKeySet(provider.id, typed);
      key = "";
      testedKey = null;
      result = { ok: true, text: "Saved in this computer's keychain." };
      await refreshKeys();
    } catch (e) {
      result = { ok: false, text: errorText(e) };
    } finally {
      busy = null;
    }
  }

  async function removeKey() {
    if (!provider) return;
    const ok = await app.confirm({ title: `Remove the ${provider.name} key?`, text: "It is deleted from this computer's keychain. The assistant falls back to the guided questions until you add one again.", confirmLabel: "Remove key", danger: true });
    if (!ok) return;
    busy = "remove";
    try {
      await api.aiKeyClear(provider.id);
      result = { ok: true, text: "Key removed." };
      models = [];
      await refreshKeys();
    } catch (e) {
      result = { ok: false, text: errorText(e) };
    } finally {
      busy = null;
    }
  }

  // ---------------------------------------------------------------- models
  let models = $state<AiModel[]>([]);
  let defaultModel = $state<string | null>(null);
  let modelsLoading = $state(false);
  let modelsError = $state<string | null>(null);
  let seq = 0;
  /** What the list says about a model, after its name in the menu. */
  const modelTag = (m: AiModel) => (m.recommended ? " — Recommended" : m.economical ? " — Economical" : m.small ? " — Small, may make more mistakes" : "");
  const chosen = $derived(models.find((m) => m.id === (ai.model || defaultModel)) ?? null);
  const economical = $derived(models.find((m) => m.economical) ?? null);
  async function loadModels() {
    if (!provider || !ready) return;
    const my = ++seq;
    const id = provider.id;
    modelsLoading = true;
    modelsError = null;
    try {
      const [list, def] = await Promise.all([api.aiListModels(id, baseUrl), api.aiDefaultModel(id, baseUrl).catch(() => null)]);
      if (my !== seq) return;
      models = list;
      defaultModel = def;
    } catch (e) {
      if (my === seq) ((models = []), (modelsError = errorText(e)));
    } finally {
      if (my === seq) modelsLoading = false;
    }
  }
  $effect(() => {
    provider?.id;
    ready;
    baseUrl;
    // untracked: the call itself touches the loading bar's counter
    untrack(() => {
      models = [];
      defaultModel = null;
      loadModels();
    });
  });
</script>

<section>
  <p class="muted small">The equalizer assistant can chat with an AI you already pay for: Claude, ChatGPT, Gemini and others, or a model running on this computer with Ollama. Without one, “Set up my sound” asks a few guided questions instead.</p>

  <h3>Provider</h3>
  <div class="segmented providers">
    <button class:selected={!ai.provider} onclick={() => pick("")}>
      <b>None</b><span>Guided questions only</span>
    </button>
    {#each providers as p (p.id)}
      <button class:selected={ai.provider === p.id} onclick={() => pick(p.id)}>
        <b>{p.name}</b>
        {#if p.badge}<em class="badge {p.badge}">{badgeLabel[p.badge]}</em>{/if}
        {#if p.hint}<span class="hint">{p.hint}</span>{/if}
        <span>{p.needs_key ? (keys[p.id] ? "Key saved" : "Needs a key") : p.custom_base_url && !p.default_base_url ? "Your own server" : "No key needed"}</span>
      </button>
    {/each}
  </div>

  {#if provider}
    {#if keyField}
      <h3>API key {#if !provider.needs_key}<small class="muted">optional</small>{/if}</h3>
      <div class="keyline">
        <span class="status" class:ok={hasKey}><span class="led"></span>{hasKey ? "A key is saved" : "No key saved yet"}</span>
        {#if provider.key_url}
          <button class="link-btn" onclick={() => openUrl(provider!.key_url)}><Icon name="external" size={13} /> Get a key</button>
        {/if}
      </div>
      <form class="key" onsubmit={(e) => (e.preventDefault(), saveKey())}>
        <input
          type="password"
          autocomplete="off"
          spellcheck="false"
          bind:value={key}
          oninput={() => (result = null)}
          placeholder={hasKey ? "Type a new key to replace the saved one" : `Paste your ${provider.name} key`}
          aria-label="{provider.name} API key"
        />
        <button type="button" class="btn tonal" disabled={!!busy || (!key.trim() && !hasKey)} onclick={test}>{busy === "test" ? "Testing…" : "Test"}</button>
        <button class="btn primary" disabled={!!busy || !key.trim()}>{busy === "save" ? "Saving…" : "Save"}</button>
        {#if hasKey}<button type="button" class="btn tonal danger" disabled={!!busy} onclick={removeKey}>Remove</button>{/if}
      </form>
    {:else}
      <h3>Connection</h3>
      <div class="keyline">
        <span class="status ok"><span class="led"></span>No key needed</span>
        {#if provider.key_url}<button class="link-btn" onclick={() => openUrl(provider!.key_url)}><Icon name="external" size={13} /> Get {provider.name}</button>{/if}
        <button class="btn tonal small" disabled={!!busy} onclick={test}>{busy === "test" ? "Testing…" : "Test connection"}</button>
      </div>
    {/if}
    {#if result}<p class="result" class:bad={!result.ok}><Icon name={result.ok ? "check" : "alert"} size={14} /> {result.text}</p>{/if}

    {#if provider.custom_base_url}
      <label class="field">
        <span>Server address</span>
        <input value={ai.base_url} placeholder={provider.default_base_url || "https://my-server.example/v1"} spellcheck="false" onchange={(e) => saveAi({ base_url: e.currentTarget.value.trim() })} />
        <small class="muted">{provider.default_base_url ? `Empty uses ${provider.default_base_url}` : "The address of an OpenAI-compatible server (LM Studio, vLLM, llama.cpp…)"}</small>
      </label>
    {/if}

    <h3>Model</h3>
    {#if !ready}
      <p class="muted small">Save a key to choose a model.</p>
    {:else}
      <div class="model">
        <select value={ai.model} disabled={modelsLoading} aria-label="Model" onchange={(e) => saveAi({ model: e.currentTarget.value })}>
          <option value="">Automatic{defaultModel ? ` (${defaultModel})` : ""}</option>
          {#each models as m (m.id)}<option value={m.id}>{m.name}{modelTag(m)}</option>{/each}
          {#if ai.model && !models.some((m) => m.id === ai.model)}<option value={ai.model}>{ai.model}</option>{/if}
        </select>
        <button class="icon-btn" title="Refresh the list" aria-label="Refresh the model list" disabled={modelsLoading} onclick={loadModels}><Icon name="refresh" size={16} /></button>
        {#if modelsLoading}<span class="muted small">Loading models…</span>{/if}
      </div>
      {#if chosen?.small}
        <p class="result warn"><Icon name="alert" size={14} /> {chosen.name} is a small model: it may make more mistakes with the equalizer. A bigger one is more reliable.</p>
      {:else if economical && !chosen?.economical}
        <p class="muted small">Economical: {economical.name} costs less and still works well.</p>
      {/if}
      {#if modelsError}<p class="result bad"><Icon name="alert" size={14} /> {modelsError}</p>{/if}
    {/if}
  {/if}

  <p class="privacy"><Icon name="shield" size={16} /> <span>Your key stays in this computer's keychain. Only what the conversation needs is sent to the provider — never your music or your Tidal session.</span></p>
  <button class="btn tonal" onclick={() => (app.view = { name: "assistant" })}><Icon name="ai" size={16} /> Set up my sound</button>
</section>

<style>
  .small {
    font-size: 12.5px;
  }
  h3 {
    margin: 20px 0 8px;
    font-size: 14px;
  }
  h3 small {
    font-weight: 400;
    font-size: 12px;
  }
  .segmented {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
    gap: 8px;
  }
  .segmented button {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px 12px;
    border-radius: min(12px, var(--r-max));
    background: var(--surface-2);
    border: 1px solid var(--border);
    text-align: left;
    transition: background 0.2s, border-color 0.2s, box-shadow 0.2s;
  }
  .segmented button span {
    font-size: 12px;
    color: var(--muted);
  }
  .segmented button:hover:not(.selected) {
    border-color: color-mix(in srgb, var(--primary) 45%, var(--border));
  }
  .segmented button.selected {
    border-color: var(--primary);
    box-shadow: inset 0 0 0 1px var(--primary);
    background: color-mix(in srgb, var(--primary) 14%, var(--surface));
  }
  .segmented button.selected b {
    color: var(--primary);
  }
  .segmented button .hint {
    color: var(--text);
    opacity: 0.8;
  }
  .badge {
    align-self: flex-start;
    margin: 2px 0;
    padding: 1px 7px;
    border-radius: var(--r-max);
    font-style: normal;
    font-size: 11px;
    font-weight: 600;
    color: var(--muted);
    background: color-mix(in srgb, var(--muted) 16%, transparent);
  }
  .badge.recommended {
    color: var(--primary);
    background: color-mix(in srgb, var(--primary) 18%, transparent);
  }
  .badge.cheapest,
  .badge.private_local {
    color: var(--ok);
    background: color-mix(in srgb, var(--ok) 16%, transparent);
  }
  .keyline {
    display: flex;
    align-items: center;
    gap: 14px;
    flex-wrap: wrap;
    margin-bottom: 8px;
    font-size: 13px;
  }
  .status {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--muted);
  }
  .led {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--faint);
  }
  .status.ok .led {
    background: var(--ok);
    box-shadow: 0 0 8px var(--ok);
  }
  .key {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    max-width: 640px;
  }
  .key input {
    flex: 1;
    min-width: 220px;
    font-family: var(--font-mono);
  }
  .link-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 0;
    font-size: 12.5px;
    color: var(--primary);
  }
  .link-btn:hover {
    text-decoration: underline;
  }
  .btn.small {
    height: 30px;
    padding: 0 12px;
  }
  .danger {
    color: var(--danger);
  }
  .result {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 8px 0 0;
    font-size: 12.5px;
    color: var(--ok);
  }
  .result.bad {
    color: var(--danger);
  }
  .result.warn {
    color: color-mix(in srgb, #e3a008 75%, var(--text));
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-width: 520px;
    margin-top: 16px;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--muted);
  }
  .field input {
    font-weight: 400;
    font-size: 14px;
    font-family: var(--font-mono);
  }
  .field small {
    font-weight: 400;
    font-size: 12px;
  }
  .model {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .model select {
    min-width: 260px;
  }
  .privacy {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    max-width: 640px;
    margin: 24px 0 12px;
    padding: 10px 12px;
    border-radius: min(12px, var(--r-max));
    background: var(--surface-2);
    font-size: 12.5px;
    color: var(--muted);
  }
  .privacy :global(svg) {
    flex: none;
    color: var(--ok);
  }
</style>
