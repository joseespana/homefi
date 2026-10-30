<script lang="ts">
  // The list of pages. By the width Settings gets it is a full list (icon, title,
  // one line about the page), a rail of icons, or - on a narrow window - the
  // first screen of a master / detail pair, with the search on top.
  import { item } from "../../lib/motion";
  import Icon from "../Icon.svelte";
  import { NAV_GROUPS, type PageDef, type PageId } from "./pages";
  import SettingsSearch from "./SettingsSearch.svelte";

  let {
    pages,
    page,
    summaryOf,
    version,
    onselect,
  }: {
    /** The pages the search leaves. */
    pages: readonly PageDef[];
    page: PageId;
    summaryOf: (id: PageId) => string;
    version: string;
    onselect: (id: PageId) => void;
  } = $props();

  const groups = $derived(NAV_GROUPS.map((g) => ({ label: g.label, items: pages.filter((p) => g.pages.includes(p.id)) })).filter((g) => g.items.length));
</script>

<nav aria-label="Settings sections">
  <div class="top">
    <h1>Settings</h1>
    <SettingsSearch place="list" />
  </div>
  {#each groups as g (g.label)}
    <div class="group">
      <p class="caption">{g.label}</p>
      <ul>
        {#each g.items as p (p.id)}
          <li in:item|global>
            <button type="button" class:selected={p.id === page} aria-current={p.id === page ? "page" : undefined} aria-label={p.title} title={p.title} onclick={() => onselect(p.id)}>
              <span class="icon"><Icon name={p.icon} size={18} /></span>
              <span class="text"><b class="ellipsis">{p.title}</b><small class="ellipsis">{summaryOf(p.id)}</small></span>
              <span class="chevron"><Icon name="chevron" size={16} /></span>
            </button>
          </li>
        {/each}
      </ul>
    </div>
  {/each}
  {#if groups.length}<p class="version">HomeFi {version}</p>{/if}
</nav>

<style>
  nav {
    position: sticky;
    top: var(--set-drag-h);
    align-self: start;
    max-height: calc(100vh - var(--set-drag-h) - var(--set-deck-h));
    overflow-y: auto;
    overscroll-behavior: contain;
    padding: var(--set-nav-pad);
    border: 1px solid var(--set-nav-border);
    border-radius: var(--set-nav-r);
    background: var(--set-nav-bg);
    color: var(--set-nav-fg);
  }
  .top {
    display: none;
  }
  h1 {
    margin: 0 0 var(--s-3);
    color: var(--set-nav-fg);
  }
  .group + .group {
    margin-top: var(--s-4);
  }
  .caption {
    margin: 0 0 var(--s-1);
    padding: 0 var(--s-3);
    font: var(--set-group-font);
    letter-spacing: var(--set-group-spacing);
    text-transform: uppercase;
    color: var(--set-nav-caption);
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li + li {
    margin-top: 2px;
  }
  button {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--s-3);
    width: 100%;
    min-height: 48px;
    padding: var(--s-1) var(--s-3) var(--s-1) var(--s-2);
    border-radius: var(--set-nav-item-r);
    text-align: left;
    color: var(--set-nav-fg);
    transition: background 0.2s;
  }
  button:hover:not(.selected) {
    background: var(--set-nav-hover);
  }
  button.selected {
    background: var(--set-nav-sel-bg);
    color: var(--set-nav-sel-fg);
  }
  button.selected::before {
    content: "";
    position: absolute;
    left: 0;
    top: var(--s-2);
    bottom: var(--s-2);
    width: 3px;
    border-radius: min(2px, var(--r-max));
    background: var(--set-nav-indicator);
  }
  button:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: -2px;
  }
  .icon {
    display: grid;
    place-items: center;
    flex: none;
    width: 32px;
    height: 32px;
    border-radius: var(--set-badge-r);
    background: var(--set-badge-bg);
    color: var(--set-badge-fg);
    box-shadow: var(--set-badge-shadow);
    transition: background 0.2s, color 0.2s;
  }
  .selected .icon {
    background: var(--set-badge-sel-bg);
    color: var(--set-badge-sel-fg);
    animation: hf-pick calc(0.36s * var(--motion-speed)) var(--ease-pop);
  }
  .text {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .text b {
    font: var(--set-label-font);
  }
  .text small {
    font-size: 12px;
    color: var(--set-nav-muted);
  }
  .selected .text small {
    color: var(--set-nav-sel-muted);
  }
  .chevron {
    display: none;
    color: var(--set-nav-muted);
  }
  .version {
    margin: var(--s-4) var(--s-3) 0;
    font-size: 11.5px;
    color: var(--faint);
  }

  /* narrow window: the list is the first screen */
  @container settings (max-width: 599px) {
    nav {
      position: static;
      max-height: none;
      overflow: visible;
    }
    .top {
      display: block;
      margin-bottom: var(--s-4);
    }
    button {
      min-height: var(--set-row-min);
    }
    .chevron {
      display: block;
    }
  }
  /* icon rail */
  @container settings (min-width: 600px) and (max-width: 899px) {
    nav {
      padding-inline: var(--s-2);
    }
    .caption,
    .text,
    .version {
      display: none;
    }
    .group + .group {
      margin-top: var(--s-2);
      padding-top: var(--s-2);
      border-top: 1px solid var(--set-divider);
    }
    button {
      justify-content: center;
      width: 40px;
      min-height: 40px;
      margin-inline: auto;
      padding: 0;
    }
    button.selected::before {
      left: -8px;
      top: var(--s-2);
      bottom: var(--s-2);
    }
  }
</style>
