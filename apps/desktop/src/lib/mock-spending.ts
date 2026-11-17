// Browser mock of the AI spending ledger (src-tauri/src/spending.rs), for the UI without the app.
import type { AiFeature, AiSpendDecision, AiSpendGuard, AiSpendItem, AiSpendSummary } from "./api";

const DAY_MS = 86_400_000;
const now = Date.now();
const item = (id: string, feature: AiFeature, label: string, daysAgo: number, cost: number | null, detail: string): AiSpendItem => ({
  id,
  feature,
  label,
  at_ms: now - daysAgo * DAY_MS,
  provider: "openrouter",
  model: "anthropic/claude-sonnet-5.5",
  calls: 3,
  input_tokens: 9_400,
  output_tokens: 1_200,
  web_searches: feature === "concert" ? 2 : 0,
  cost_usd: cost,
  cost_kind: cost === null ? "unknown" : "exact",
  cost_text: cost === null ? "Cost unknown" : `$${cost.toFixed(2)}`,
  detail,
});

const usd = (v: number) => `$${v.toFixed(2)}`;

export const spendMock = {
  guard: { ask: "never", ask_above_usd: 0.1, monthly_limit_usd: null } as AiSpendGuard,
  history: [
    item("a1", "party_playlist", "Party playlists", 0, 0.04, "anthropic/claude-sonnet-5.5 · 4 calls · 10.6k tokens"),
    item("a2", "concert", "Concert playlists", 1, 0.09, "anthropic/claude-sonnet-5.5 · 2 calls · 10.6k tokens · 2 web searches"),
    item("a3", "sound_reaction", "“How does it sound?”", 2, 0.003, "anthropic/claude-sonnet-5.5 · 1 call · 1.9k tokens"),
    item("a4", "party_playlist", "Party playlists", 3, 0.03, "anthropic/claude-sonnet-5.5 · 3 calls · 8.1k tokens"),
  ] as AiSpendItem[],

  monthTotal(): number {
    return this.history.reduce((s, h) => s + (h.cost_usd ?? 0), 0);
  },

  summary(): AiSpendSummary {
    const total = this.monthTotal();
    const by = new Map<AiFeature, AiSpendItem[]>();
    for (const h of this.history) by.set(h.feature, [...(by.get(h.feature) ?? []), h]);
    const features = [...by.entries()].map(([feature, items]) => {
      const sum = items.reduce((s, h) => s + (h.cost_usd ?? 0), 0);
      const avg = sum / items.length;
      return { feature, label: items[0].label, times: items.length, total_usd: sum, average_usd: avg, estimated: false, text: `${usd(sum)} in ${items.length} times · ≈${usd(avg)} each` };
    });
    return { month_total_usd: total, month_estimated: false, month_unknown: 0, month_actions: this.history.length, month_text: usd(total), features, history: this.history, guard: this.guard };
  },

  check(feature: AiFeature): AiSpendDecision {
    const month = this.monthTotal();
    const limit = this.guard.monthly_limit_usd;
    if (limit !== null && month >= limit) {
      return { kind: "blocked", month_total_usd: month, limit_usd: limit, prompt: { title: "You reached this month's limit", text: `You've spent ${usd(month)} on AI this month and your limit is ${usd(limit)}.` } };
    }
    if (this.guard.ask === "never") return { kind: "allow" };
    const past = this.history.filter((h) => h.feature === feature && h.cost_usd !== null);
    const estimate = past.length ? past.reduce((s, h) => s + (h.cost_usd ?? 0), 0) / past.length : null;
    if (this.guard.ask === "above" && estimate !== null && estimate <= this.guard.ask_above_usd) return { kind: "allow" };
    const cost = estimate === null ? "We don't know yet what this costs." : `This usually costs ≈${usd(estimate)}.`;
    return { kind: "confirm", estimate_usd: estimate, estimated: true, month_total_usd: month, prompt: { title: "Use the AI?", text: `${cost} You've spent ${usd(month)} this month. Continue?` } };
  },
};
