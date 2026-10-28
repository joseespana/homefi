<script lang="ts">
  // "Find my headphones": AutoEq's measured corrections, searched by name.
  import { api, errorText, type EqProfile, type HeadphoneEntry } from "../../lib/api";
  import { item } from "../../lib/motion";
  import Icon from "../Icon.svelte";

  let { target, onapply }: { target: string; onapply: (profile: EqProfile, entry: HeadphoneEntry) => void } = $props();

  let query = $state("");
  let results = $state<HeadphoneEntry[] | null>(null);
  let searching = $state(false);
  let applying = $state<string | null>(null);
  let error = $state<string | null>(null);

  let timer: ReturnType<typeof setTimeout> | null = null;
  let seq = 0;
  $effect(() => {
    const q = query.trim();
    if (timer) clearTimeout(timer);
    if (q.length < 2) {
      results = null;
      error = null;
      return;
    }
    timer = setTimeout(() => search(q), 280);
  });

  async function search(q: string) {
    const my = ++seq;
    searching = true;
    try {
      const r = await api.autoeqSearch(q);
      if (my !== seq) return;
      results = r.slice(0, 30);
      error = null;
    } catch (e) {
      if (my === seq) error = errorText(e);
    } finally {
      if (my === seq) searching = false;
    }
  }

  async function apply(e: HeadphoneEntry) {
    applying = e.path;
    try {
      onapply(await api.autoeqProfile(e), e);
      error = null;
    } catch (err) {
      error = errorText(err);
    } finally {
      applying = null;
    }
  }
</script>

<div class="finder">
  <label class="searchbox">
    <Icon name="search" size={16} />
    <input type="search" placeholder="Brand and model, e.g. HD 650" bind:value={query} spellcheck="false" />
    {#if searching}<span class="spin" aria-label="Searching"></span>{/if}
  </label>
  {#if error}
    <p class="msg error"><Icon name="alert" size={14} /> {error}</p>
  {:else if results && !results.length && !searching}
    <p class="msg muted">No headphones match “{query.trim()}”. Try just the model (“650”, “XM5”).</p>
  {:else if !results}
    <p class="msg muted">Corrections measured by oratory1990, crinacle, Rtings and others. They make your headphones sound neutral; add your taste on top.</p>
  {/if}
  {#if results?.length}
    <ul class="results">
      {#each results as r (r.path)}
        <li in:item>
          <div class="grow">
            <b class="ellipsis">{r.name}</b>
            <span class="tags">
              <span class="badge">{r.kind}</span>
              <span class="muted">by {r.source}{r.rig ? ` · ${r.rig}` : ""}</span>
            </span>
          </div>
          <button class="btn tonal small" disabled={!!applying} onclick={() => apply(r)} title="Use this correction for {target}">
            {applying === r.path ? "Loading…" : "Apply"}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .finder {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .searchbox {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 12px;
    border-radius: min(10px, var(--r-max));
    background: var(--surface-2);
    border: 1px solid var(--border);
    color: var(--muted);
  }
  .searchbox input {
    flex: 1;
    min-width: 0;
    border: 0;
    padding: 0;
    background: transparent;
  }
  .spin {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid var(--glass-border);
    border-top-color: var(--accent-text);
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .msg {
    display: flex;
    gap: 6px;
    align-items: flex-start;
    margin: 0;
    font-size: 12.5px;
  }
  .error {
    color: var(--danger);
  }
  .results {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-height: 300px;
    overflow-y: auto;
  }
  .results li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-radius: min(10px, var(--r-max));
    background: var(--surface-2);
  }
  .grow {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .tags {
    display: flex;
    gap: 6px;
    align-items: center;
    font-size: 11.5px;
  }
  .btn.small {
    height: 30px;
    padding: 0 12px;
    font-size: 12.5px;
  }
</style>
