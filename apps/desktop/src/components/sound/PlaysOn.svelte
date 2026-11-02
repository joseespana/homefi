<script lang="ts">
  // "{sound} plays on (N)": what the playing sound applies to, one row per
  // target, most specific kind first, × to take one out and "+ Add" to search
  // for more. Collapses after FOLD rows ("Show all").
  import type { TasteRule, TasteScope } from "../../lib/api";
  import { kindLabel, kindRank, sound } from "../../lib/sound.svelte";
  import Icon from "../Icon.svelte";
  import KindBadge from "./KindBadge.svelte";
  import TargetSearch from "./TargetSearch.svelte";

  let { rule }: { rule: TasteRule } = $props();

  /** Rows shown before "Show all". */
  const FOLD = 5;

  let open = $state(false);
  let all = $state(false);
  let searching = $state(false);

  const scopes = $derived<TasteScope[]>([...rule.targets.map((t) => t.scope)].sort((a, b) => kindRank(a.kind) - kindRank(b.kind) || a.name.localeCompare(b.name)));
  const shown = $derived(all ? scopes : scopes.slice(0, FOLD));
</script>

<div class="plays">
  <div class="head">
    <button class="toggle" aria-expanded={open} onclick={() => (open = !open)}>
      <Icon name={open ? "down" : "chevron"} size={14} />
      <span class="ellipsis"><b>{rule.label}</b> plays on ({scopes.length})</span>
    </button>
    {#if open}<button class="btn tonal small" aria-expanded={searching} onclick={() => (searching = !searching)}><Icon name="plus" size={14} /> Add</button>{/if}
  </div>
  {#if open}
    {#if searching}
      <TargetSearch soundId={rule.id} onadd={(h) => ((searching = false), sound.assign(rule.id, h.scope))} onclose={() => (searching = false)} />
    {/if}
    <ul>
      {#each shown as s (s.kind + (s.kind === "genre" ? "" : s.id) + s.name)}
        <li>
          <KindBadge kind={s.kind} />
          <span class="name ellipsis">{s.name || "Untitled"}</span>
          <span class="muted">{kindLabel(s.kind)}</span>
          <button class="icon-btn mini" aria-label="Remove {s.name} from {rule.label}" title="Remove" onclick={() => sound.unassign(rule.id, s)}><Icon name="x" size={14} /></button>
        </li>
      {/each}
    </ul>
    {#if scopes.length > FOLD}
      <button class="link" onclick={() => (all = !all)}>{all ? "Show fewer" : `Show all ${scopes.length}`}</button>
    {/if}
  {/if}
</div>

<style>
  .plays {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .toggle {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    font-size: 12.5px;
    color: var(--muted);
  }
  .toggle b {
    color: var(--text);
  }
  .small {
    height: 28px;
    padding: 0 12px;
    font-size: 12.5px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
  }
  li {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 10px;
    padding: 4px 0;
    font-size: 13px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
  }
  .name {
    font-weight: 600;
  }
  .muted {
    font-size: 12px;
  }
  .icon-btn.mini {
    width: 26px;
    height: 26px;
  }
  .link {
    align-self: flex-start;
    font-size: 12.5px;
    color: var(--primary);
  }
  .link:hover {
    text-decoration: underline;
  }
</style>
