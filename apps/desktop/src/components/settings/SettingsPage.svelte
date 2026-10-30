<script lang="ts">
  // One page of Settings: the sticky header (back on narrow windows, the title and
  // a line about the page, the "Saved" check, the search) and the body that
  // holds its groups. The body rises in when the page changes.
  import type { Snippet } from "svelte";
  import { rise } from "../../lib/motion";
  import Icon from "../Icon.svelte";
  import { useSettings } from "./model.svelte";
  import { settingsSearch } from "./search.svelte";
  import SettingsEmpty from "./SettingsEmpty.svelte";
  import SettingsSearch from "./SettingsSearch.svelte";

  let { title, summary, onback, children }: { title: string; summary?: string; onback: () => void; children: Snippet } = $props();

  const model = useSettings();
  let head = $state<HTMLElement>();
  // the header turns into frosted glass once the page has scrolled under it
  let scrolled = $state(false);
  $effect(() => {
    if (!head) return;
    let box: HTMLElement | null = head.parentElement;
    while (box && !/(auto|scroll)/.test(getComputedStyle(box).overflowY)) box = box.parentElement;
    const scroller = box ?? (document.scrollingElement as HTMLElement);
    const update = () => (scrolled = scroller.scrollTop > 2);
    update();
    scroller.addEventListener("scroll", update, { passive: true });
    return () => scroller.removeEventListener("scroll", update);
  });
</script>

<header class="head" class:scrolled bind:this={head}>
  <div class="inner">
    <button type="button" class="back" onclick={onback}><Icon name="back" size={18} /> Settings</button>
    <div class="titles">
      <h2 class="title">{title}</h2>
      {#if summary}<p class="summary ellipsis">{summary}</p>{/if}
    </div>
    <span class="saved" aria-live="polite">{#if model.saved}<Icon name="check" size={14} /> Saved{/if}</span>
    <SettingsSearch place="head" />
  </div>
</header>

{#key title}
  <div class="body" in:rise={{ y: 12 }}>
    {@render children()}
    {#if settingsSearch.active && settingsSearch.shown === 0}<SettingsEmpty />{/if}
  </div>
{/key}

<style>
  /* clear at rest so it melts into the page; frosted glass once rows scroll under it.
     It reaches up under the window's drag strip (which stays on top and draggable), so
     one surface covers the strip too and nothing peeks over the title. */
  .head {
    position: sticky;
    top: 0;
    z-index: 2;
    margin-top: calc(-1 * var(--set-drag-h));
    padding-top: var(--set-drag-h);
    border-radius: 0 0 var(--set-card-r) var(--set-card-r);
    transition: background 0.2s, box-shadow 0.2s;
  }
  .head.scrolled {
    background: var(--set-head-bg);
    -webkit-backdrop-filter: blur(18px) saturate(1.3);
    backdrop-filter: blur(18px) saturate(1.3);
    box-shadow: 0 10px 20px -16px rgb(0 0 0 / 0.55);
  }
  .inner {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    max-width: var(--col);
    margin-inline: auto;
    padding: var(--s-4) 0 var(--s-3);
  }
  .titles {
    flex: 1;
    min-width: 0;
  }
  .titles h2.title {
    margin: 0;
    font: var(--set-title-font);
    color: var(--set-title-color);
    text-transform: var(--set-title-transform);
    letter-spacing: var(--set-title-spacing);
    width: fit-content;
    max-width: 100%;
    padding: var(--set-title-pad);
    background: var(--set-title-bar);
  }
  .summary {
    margin: var(--s-1) 0 0;
    font: 400 13px/1.3 var(--font);
    color: var(--muted);
  }
  .saved {
    display: inline-flex;
    align-items: center;
    gap: var(--s-1);
    flex: none;
    font: 500 12.5px var(--font);
    color: var(--ok);
    animation: saved-in 0.2s var(--ease-out);
  }
  .saved:empty {
    display: none;
  }
  @keyframes saved-in {
    from {
      opacity: 0;
    }
  }
  .back {
    display: none;
    align-items: center;
    gap: var(--s-1);
    flex: none;
    min-height: var(--set-hit);
    padding: 0 var(--s-2) 0 0;
    color: var(--primary);
    font: var(--set-label-font);
  }
  @container settings (max-width: 599px) {
    .back {
      display: inline-flex;
    }
    /* the title already says where you are */
    .titles .summary {
      display: none;
    }
  }
  .body {
    display: grid;
    grid-template-columns: var(--set-body-cols, minmax(0, 1fr));
    justify-content: center;
    align-content: start;
    gap: var(--s-5);
    max-width: var(--col);
    margin-inline: auto;
    /* room for the floating mascot, so it never sits on a control */
    padding: var(--s-2) 0 var(--set-mascot-clear);
  }
</style>
