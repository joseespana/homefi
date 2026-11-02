<script lang="ts">
  // The small "AI" tag on a sound the assistant made. Colours are tokens
  // (--ai-tag-*), checked at 4.5:1 on dark and light skins.
  import type { TasteCreator } from "../../lib/api";

  let { by }: { by: TasteCreator } = $props();
</script>

{#if by === "ai"}<span class="ai" title="Made with AI">AI</span>{/if}

<style>
  .ai {
    flex: none;
    padding: 1px 6px;
    border-radius: min(999px, var(--r-max));
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.04em;
    line-height: 1.5;
    background: var(--ai-tag-bg);
    color: var(--ai-tag-text);
  }
</style>
