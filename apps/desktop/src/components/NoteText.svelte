<script lang="ts">
  // One line of release notes as text nodes (bold, code, links): never HTML. Links open in the
  // system browser, not in this window.
  import { openUrl } from "@tauri-apps/plugin-opener";
  import type { NoteLine } from "../lib/api";

  let { line }: { line: NoteLine } = $props();

  function open(e: MouseEvent, url: string) {
    e.preventDefault();
    openUrl(url).catch(() => {});
  }
</script>

{#each line.spans as s}{#if s.link}<a href={s.link} title={s.link} rel="noopener noreferrer" onclick={(e) => open(e, s.link!)}>{#if s.bold}<b>{s.text}</b>{:else}{s.text}{/if}</a>{:else if s.code}<code>{s.text}</code>{:else if s.bold}<b>{s.text}</b>{:else}{s.text}{/if}{/each}

<style>
  b {
    font-weight: 650;
  }
  code {
    font: 12px var(--font-mono);
    padding: 0 4px;
    border-radius: var(--r-xs);
    background: var(--surface-2);
  }
  a {
    color: var(--primary);
    text-decoration: underline;
  }
  a:hover {
    color: var(--primary-strong);
  }
</style>
