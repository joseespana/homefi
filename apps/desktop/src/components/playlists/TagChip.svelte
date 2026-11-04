<script lang="ts">
  // How often a song is played on the tour (Always / Usually / Sometimes / Encore), or "Not
  // found". Each has its own colour; the text keeps the theme's ink, so it reads in light and dark.
  import type { EntryTag } from "../../lib/api";

  type Kind = Exclude<EntryTag, "plain"> | "missing";
  let { tag }: { tag: EntryTag | "missing" } = $props();

  const LABELS: Record<Kind, string> = { always: "Always", usually: "Usually", sometimes: "Sometimes", encore: "Encore", missing: "Not found" };
  const kind = $derived(tag === "plain" ? null : tag);
</script>

{#if kind}<span class="chip {kind}">{LABELS[kind]}</span>{/if}

<style>
  .chip {
    --tone: var(--muted);
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 1px 8px;
    border: 1px solid color-mix(in srgb, var(--tone) 60%, transparent);
    border-radius: min(999px, var(--r-max));
    background: color-mix(in srgb, var(--tone) 18%, var(--surface));
    color: var(--text);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.02em;
    white-space: nowrap;
  }
  .chip::before {
    content: "";
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--tone);
  }
  .always {
    --tone: var(--primary-strong);
  }
  .usually {
    --tone: var(--ok);
  }
  .sometimes {
    --tone: var(--warn);
  }
  .encore {
    --tone: #c084fc;
  }
  .missing {
    --tone: var(--danger);
    border-style: dashed;
  }
  .missing::before {
    display: none;
  }
</style>
