<script lang="ts">
  // Two letters for what a target is (AR / SO / AL / PL / GE); ALL for "everything else".
  import type { TasteScopeKind } from "../../lib/api";
  import { kindBadge, kindLabel } from "../../lib/sound.svelte";

  let { kind = null, all = false }: { kind?: TasteScopeKind | null; all?: boolean } = $props();
  const text = $derived(all ? "ALL" : kind ? kindBadge(kind) : "··");
  const title = $derived(all ? "Everything else" : kind ? kindLabel(kind) : "No music yet");
</script>

<span class="kind" class:all {title}>{text}</span>

<style>
  .kind {
    flex: none;
    display: grid;
    place-items: center;
    min-width: 28px;
    height: 26px;
    padding: 0 5px;
    border-radius: var(--r-sm);
    font-size: 10px;
    font-weight: 800;
    letter-spacing: 0.06em;
    color: var(--accent-text);
    background: color-mix(in srgb, var(--primary) 14%, var(--surface-2));
    border: 1px solid color-mix(in srgb, var(--primary) 30%, transparent);
  }
  .kind.all {
    color: var(--muted);
    background: var(--surface-2);
    border-color: var(--border);
  }
</style>
