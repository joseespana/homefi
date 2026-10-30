<script lang="ts">
  // A line of explanation at the top of a group: why something is locked, an
  // offline or error message. Errors are announced to screen readers.
  import type { Snippet } from "svelte";
  import Icon from "../Icon.svelte";

  let { kind = "info", children }: { kind?: "info" | "warn" | "error"; children: Snippet } = $props();
</script>

<p class="callout {kind}" role={kind === "error" ? "alert" : undefined}>
  <Icon name={kind === "info" ? "info" : "alert"} size={16} />
  <span>{@render children()}</span>
</p>

<style>
  .callout {
    display: flex;
    align-items: flex-start;
    gap: var(--s-2);
    margin: 0;
    padding: var(--s-3) var(--s-4);
    background: color-mix(in srgb, var(--primary) 8%, transparent);
    font: var(--set-hint-font);
    color: var(--text);
  }
  .callout :global(svg) {
    flex: none;
    margin-top: 1px;
    color: var(--primary);
  }
  .warn {
    background: color-mix(in srgb, #e3a008 12%, transparent);
  }
  .warn :global(svg) {
    color: #e3a008;
  }
  .error {
    background: color-mix(in srgb, var(--danger) 10%, transparent);
  }
  .error :global(svg) {
    color: var(--danger);
  }
</style>
