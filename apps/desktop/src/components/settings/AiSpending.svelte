<script lang="ts">
  // Settings → AI → Spending: this month's total, what each kind of action costs, the history,
  // "ask before spending" and the monthly limit. The numbers and their words come from the core's
  // ledger (homefi-core `ai::usage`); only feature, time, model, tokens and cost are kept.
  import { api, errorText, type AiAskMode, type AiSpendGuard, type AiSpendSummary } from "../../lib/api";
  import { uiLanguage } from "../../lib/language";
  import { app } from "../../lib/state.svelte";
  import Spinner from "../Spinner.svelte";
  import ActionRow from "./ActionRow.svelte";
  import FieldRow from "./FieldRow.svelte";
  import SegmentedRow from "./SegmentedRow.svelte";
  import SettingRow from "./SettingRow.svelte";
  import SettingsGroup from "./SettingsGroup.svelte";
  import SettingsNote from "./SettingsNote.svelte";
  import SwitchRow from "./SwitchRow.svelte";

  /** The provider picked in the AI settings (its balance can be asked for when it's OpenRouter). */
  let { provider }: { provider: string } = $props();

  /** History rows shown at first, and added by "Show more". */
  const HISTORY_PAGE = 20;
  /** What a new monthly limit starts at (core `DEFAULT_MONTHLY_LIMIT_USD`). */
  const DEFAULT_LIMIT_USD = 5;

  let summary = $state<AiSpendSummary | null>(null);
  let loadError = $state<string | null>(null);
  let shown = $state(HISTORY_PAGE);

  async function load() {
    try {
      summary = await api.aiSpendingSummary(uiLanguage());
      loadError = null;
    } catch (e) {
      loadError = errorText(e);
    }
  }
  $effect(() => {
    load();
  });

  const guard = $derived<AiSpendGuard>(summary?.guard ?? { ask: "never", ask_above_usd: 0.1, monthly_limit_usd: null });

  async function saveGuard(patch: Partial<AiSpendGuard>) {
    try {
      await api.aiSpendingSetGuard({ ...guard, ...patch });
    } catch (e) {
      app.notify(errorText(e), true);
    }
    await load();
  }
  /** A typed amount in USD, or null when it isn't one. */
  const amount = (text: string) => {
    const n = Number(text.replace(",", ".").replace("$", "").trim());
    return Number.isFinite(n) && n > 0 ? n : null;
  };

  const askOptions: { id: AiAskMode; label: string; hint: string }[] = [
    { id: "never", label: "Don't ask", hint: "AI actions start right away" },
    { id: "always", label: "Always", hint: "Ask before every AI action" },
    { id: "above", label: "Above an amount", hint: "Ask only when it usually costs more than the amount, or nobody knows yet" },
  ];

  // ---------------------------------------------------------------- balance
  let balance = $state<number | null | undefined>(undefined);
  let balanceBusy = $state(false);
  let balanceError = $state<string | null>(null);
  async function checkBalance() {
    if (balanceBusy) return;
    balanceBusy = true;
    balanceError = null;
    try {
      balance = await api.aiBalance();
    } catch (e) {
      balanceError = errorText(e);
    } finally {
      balanceBusy = false;
    }
  }
  const usd = (v: number) => `$${v.toFixed(2)}`;

  // ---------------------------------------------------------------- history
  let clearing = $state(false);
  async function clear() {
    const ok = await app.confirm({ title: "Clear the AI spending history?", text: "The list and the totals start again from zero. What you spent at the AI service doesn't change, and your limit stays.", confirmLabel: "Clear history", danger: true });
    if (!ok) return;
    clearing = true;
    try {
      await api.aiSpendingClear();
      shown = HISTORY_PAGE;
      await load();
    } catch (e) {
      app.notify(errorText(e), true);
    } finally {
      clearing = false;
    }
  }
  const when = (ms: number) => new Date(ms).toLocaleString(undefined, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
</script>

<SettingsGroup title="Spending" description="What the AI has cost you, counted on this computer. Amounts with ≈ are estimates from the model's list price; the AI service's bill is what counts.">
  {#if loadError}<SettingsNote kind="error">{loadError}</SettingsNote>{/if}
  <SettingRow label="This month" hint={summary ? `${summary.month_actions} AI ${summary.month_actions === 1 ? "action" : "actions"}${summary.month_unknown ? ` · ${summary.month_unknown} without a known price, not counted` : ""}` : "Loading…"}>
    <strong class="total">{summary?.month_text ?? "—"}</strong>
  </SettingRow>
  {#if provider === "openrouter"}
    <ActionRow label="Left on OpenRouter" hint={balanceError ?? (balance === undefined ? "Asks OpenRouter; spends nothing" : balance === null ? "OpenRouter didn't say" : `${usd(balance)} left`)}>
      <button type="button" class="btn tonal" disabled={balanceBusy} aria-busy={balanceBusy} onclick={checkBalance}>{#if balanceBusy}<Spinner size={14} /> Checking…{:else}Check balance{/if}</button>
    </ActionRow>
  {/if}
  {#if summary?.features.length}
    {#each summary.features as f (f.feature)}
      <SettingRow label={f.label} hint={f.text} />
    {/each}
  {:else if summary}
    <SettingRow label="Nothing yet" hint="Every AI action (a ride, a concert, a party playlist, the sound assistant…) shows up here with what it cost." />
  {/if}
</SettingsGroup>

<SettingsGroup title="Ask before spending" description="So the AI never uses up your balance by surprise. The monthly limit is checked on every AI call.">
  <SegmentedRow label="Ask before an AI action" hint={askOptions.find((o) => o.id === guard.ask)?.hint} options={askOptions} value={guard.ask} onchange={(ask) => saveGuard({ ask })} />
  {#if guard.ask === "above"}
    <FieldRow label="Ask when it costs more than" hint="In US dollars, like the AI services bill" value={guard.ask_above_usd.toFixed(2)} type="text" inputmode="numeric" width="110px" onchange={(v) => saveGuard({ ask_above_usd: amount(v) ?? guard.ask_above_usd })} />
  {/if}
  <SwitchRow label="Monthly limit" hint="Stops AI actions once this month's spending reaches it" checked={guard.monthly_limit_usd !== null} onchange={(on) => saveGuard({ monthly_limit_usd: on ? DEFAULT_LIMIT_USD : null })} />
  {#if guard.monthly_limit_usd !== null}
    <FieldRow label="Limit per month" hint="In US dollars. Starts again on the 1st." value={guard.monthly_limit_usd.toFixed(2)} type="text" inputmode="numeric" width="110px" onchange={(v) => saveGuard({ monthly_limit_usd: amount(v) ?? guard.monthly_limit_usd })} />
  {/if}
</SettingsGroup>

<SettingsGroup title="Spending history">
  {#if summary?.history.length}
    <ul class="history">
      {#each summary.history.slice(0, shown) as h (h.id)}
        <li>
          <span class="what">
            <span class="label">{h.label}</span>
            <span class="detail">{when(h.at_ms)} · {h.detail}</span>
          </span>
          <span class="cost" class:unknown={h.cost_kind === "unknown"}>{h.cost_text}</span>
        </li>
      {/each}
    </ul>
    {#if summary.history.length > shown}
      <ActionRow label="{summary.history.length - shown} more">
        <button type="button" class="btn tonal" onclick={() => (shown += HISTORY_PAGE)}>Show more</button>
      </ActionRow>
    {/if}
    <ActionRow label="Clear history" hint="Only the list on this computer">
      <button type="button" class="btn tonal danger" disabled={clearing} aria-busy={clearing} onclick={clear}>{#if clearing}<Spinner size={14} /> Clearing…{:else}Clear history{/if}</button>
    </ActionRow>
  {:else}
    <SettingRow label="No AI actions yet" />
  {/if}
</SettingsGroup>

<style>
  .total {
    font-size: 20px;
    font-variant-numeric: tabular-nums;
  }
  .history {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .history li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s-3);
    padding: var(--s-2) var(--s-4);
    border-top: 1px solid var(--set-hairline, color-mix(in srgb, var(--text) 8%, transparent));
  }
  .history li:first-child {
    border-top: none;
  }
  .what {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .label {
    font-size: 14px;
  }
  .detail {
    font-size: 12px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cost {
    font-variant-numeric: tabular-nums;
    font-weight: 600;
    white-space: nowrap;
  }
  .cost.unknown {
    font-weight: 400;
    color: var(--muted);
  }
</style>
