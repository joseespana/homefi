<script lang="ts">
  // Settings → AI: bring your own key. The key goes to the OS keychain through
  // the backend and never comes back to the screen (only "saved / not saved").
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { untrack } from "svelte";
  import { api, errorText, type AiModel, type AiProviderBadge, type AiProviderId, type AiProviderInfo, type AiSettings, type Settings } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import Icon from "./Icon.svelte";
  import ActionRow from "./settings/ActionRow.svelte";
  import FieldRow from "./settings/FieldRow.svelte";
  import OptionGallery from "./settings/OptionGallery.svelte";
  import SettingRow from "./settings/SettingRow.svelte";
  import SettingsGroup from "./settings/SettingsGroup.svelte";
  import SettingsIntro from "./settings/SettingsIntro.svelte";
  import SettingsNote from "./settings/SettingsNote.svelte";

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
  // the provider cards: "None" first, then the list the backend ranks
  const choices = $derived([{ id: "" as AiProviderId | "", label: "None", hint: "Guided questions only" }, ...providers.map((p) => ({ id: p.id as AiProviderId | "", label: p.name, hint: p.hint }))]);
  const providerNote = (p: AiProviderInfo) => (p.needs_key ? (keys[p.id] ? "Key saved" : "Needs a key") : p.custom_base_url && !p.default_base_url ? "Your own server" : "No key needed");
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

<SettingsIntro>The equalizer assistant can chat with an AI you already pay for: Claude, ChatGPT, Gemini and others, or a model running on this computer with Ollama. Without one, “Set up my sound” asks a few guided questions instead.</SettingsIntro>

<SettingsGroup title="Provider" span={2}>
  <SettingRow label="Provider" hint="Which AI the assistant talks to" stack="always">
    {#snippet children(ids)}
      <OptionGallery options={choices} value={(ai.provider || "") as AiProviderId | ""} onchange={pick} labelledby={ids.label} min={190}>
        {#snippet extra(o)}
          {@const p = providers.find((x) => x.id === o.id)}
          {#if p?.badge}<em class="chip {p.badge}">{badgeLabel[p.badge]}</em>{/if}
          {#if p}<span class="state">{providerNote(p)}</span>{/if}
        {/snippet}
      </OptionGallery>
    {/snippet}
  </SettingRow>
</SettingsGroup>

{#if provider}
  <SettingsGroup title={keyField ? (provider.needs_key ? "API key" : "API key (optional)") : "Connection"}>
    {#if result}
      <SettingsNote kind={result.ok ? "info" : "error"}>{result.text}</SettingsNote>
    {/if}
    {#if keyField}
      <ActionRow label={hasKey ? "A key is saved" : "No key saved yet"} hint={provider.key_url ? undefined : "Paste it below"}>
        <span class="led" class:ok={hasKey} aria-hidden="true"></span>
        {#if provider.key_url}
          <button type="button" class="btn tonal" onclick={() => openUrl(provider!.key_url)}><Icon name="external" size={14} /> Get a key</button>
        {/if}
      </ActionRow>
      <SettingRow label="{provider.name} API key" stack="always">
        {#snippet children(ids)}
          <form class="key" onsubmit={(e) => (e.preventDefault(), saveKey())}>
            <input
              id={ids.control}
              type="password"
              autocomplete="off"
              spellcheck="false"
              bind:value={key}
              oninput={() => (result = null)}
              placeholder={hasKey ? "Type a new key to replace the saved one" : `Paste your ${provider.name} key`}
              aria-labelledby={ids.label}
            />
            <button type="button" class="btn tonal" disabled={!!busy || (!key.trim() && !hasKey)} onclick={test}>{busy === "test" ? "Testing…" : "Test"}</button>
            <button class="btn primary" disabled={!!busy || !key.trim()}>{busy === "save" ? "Saving…" : "Save"}</button>
            {#if hasKey}<button type="button" class="btn tonal danger" disabled={!!busy} onclick={removeKey}>Remove</button>{/if}
          </form>
        {/snippet}
      </SettingRow>
    {:else}
      <ActionRow label="No key needed" hint="This provider is used as it is">
        <span class="led ok" aria-hidden="true"></span>
        {#if provider.key_url}<button type="button" class="btn tonal" onclick={() => openUrl(provider!.key_url)}><Icon name="external" size={14} /> Get {provider.name}</button>{/if}
        <button type="button" class="btn tonal" disabled={!!busy} onclick={test}>{busy === "test" ? "Testing…" : "Test connection"}</button>
      </ActionRow>
    {/if}
    {#if provider.custom_base_url}
      <FieldRow
        label="Server address"
        hint={provider.default_base_url ? `Empty uses ${provider.default_base_url}` : "The address of an OpenAI-compatible server (LM Studio, vLLM, llama.cpp…)"}
        value={ai.base_url}
        placeholder={provider.default_base_url || "https://my-server.example/v1"}
        mono
        onchange={(v) => saveAi({ base_url: v.trim() })}
      />
    {/if}
  </SettingsGroup>

  <SettingsGroup title="Model">
    {#if !ready}
      <SettingRow label="Model" hint="Save a key to choose a model." />
    {:else}
      {#if chosen?.small}
        <SettingsNote kind="warn">{chosen.name} is a small model: it may make more mistakes with the equalizer. A bigger one is more reliable.</SettingsNote>
      {:else if economical && !chosen?.economical}
        <SettingsNote>Economical: {economical.name} costs less and still works well.</SettingsNote>
      {/if}
      {#if modelsError}<SettingsNote kind="error">{modelsError}</SettingsNote>{/if}
      <SettingRow label="Model" hint={modelsLoading ? "Loading models…" : undefined} labelFor>
        {#snippet children(ids)}
          <div class="model">
            <select id={ids.control} value={ai.model} disabled={modelsLoading} onchange={(e) => saveAi({ model: e.currentTarget.value })}>
              <option value="">Automatic{defaultModel ? ` (${defaultModel})` : ""}</option>
              {#each models as m (m.id)}<option value={m.id}>{m.name}{modelTag(m)}</option>{/each}
              {#if ai.model && !models.some((m) => m.id === ai.model)}<option value={ai.model}>{ai.model}</option>{/if}
            </select>
            <button type="button" class="icon-btn" title="Refresh the list" aria-label="Refresh the model list" disabled={modelsLoading} onclick={loadModels}><Icon name="refresh" size={16} /></button>
          </div>
        {/snippet}
      </SettingRow>
    {/if}
  </SettingsGroup>
{/if}

<SettingsGroup title="Privacy">
  <SettingsNote>Your key stays in this computer's keychain. Only what the conversation needs is sent to the provider — never your music or your Tidal session.</SettingsNote>
  <ActionRow label="Set up my sound" hint="Chat with the assistant, or answer its guided questions">
    <button type="button" class="btn tonal" onclick={() => (app.view = { name: "assistant" })}><Icon name="ai" size={16} /> Set up my sound</button>
  </ActionRow>
</SettingsGroup>

<style>
  .chip {
    align-self: flex-start;
    margin: 2px 0;
    padding: 1px 7px;
    border-radius: min(999px, var(--r-max));
    font-style: normal;
    font-size: 11px;
    font-weight: 600;
    color: var(--muted);
    background: color-mix(in srgb, var(--muted) 16%, transparent);
  }
  .chip.recommended {
    color: var(--primary);
    background: color-mix(in srgb, var(--primary) 18%, transparent);
  }
  .chip.cheapest,
  .chip.private_local {
    color: var(--ok);
    background: color-mix(in srgb, var(--ok) 16%, transparent);
  }
  .state {
    font-size: 12px;
    color: var(--text);
    opacity: 0.8;
  }
  .led {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--faint);
  }
  .led.ok {
    background: var(--ok);
    box-shadow: 0 0 8px var(--ok);
  }
  .key {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-2);
  }
  .key :global(.btn.danger) {
    color: var(--danger);
  }
  .key input {
    flex: 1;
    min-width: 220px;
    min-height: 36px;
    font-family: var(--font-mono);
  }
  .model {
    display: flex;
    align-items: center;
    gap: var(--s-2);
  }
  .model select {
    min-width: 220px;
    max-width: 100%;
    min-height: 36px;
  }
</style>
