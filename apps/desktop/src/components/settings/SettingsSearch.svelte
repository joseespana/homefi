<script lang="ts">
  // The search box. It lives in the page header (collapsing to an icon button in
  // the icon-rail layout) and on top of the page list in the narrow layout.
  import Icon from "../Icon.svelte";
  import { settingsSearch } from "./search.svelte";

  let { place }: { place: "head" | "list" } = $props();

  const shortcut = typeof navigator !== "undefined" && /mac/i.test(navigator.platform) ? "⌘K" : "Ctrl K";
</script>

<label class="search {place}" class:filled={!!settingsSearch.query}>
  <Icon name="search" size={16} />
  <input type="search" placeholder="Search settings" aria-label="Search settings" bind:value={settingsSearch.query} />
  <kbd aria-hidden="true">{shortcut}</kbd>
</label>

<style>
  .search {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    flex: none;
    width: clamp(220px, 28cqi, 320px);
    height: 36px;
    padding: 0 var(--s-3);
    border-radius: var(--set-field-r);
    background: var(--set-field-bg);
    border: 1px solid var(--set-field-border);
    box-shadow: var(--set-field-shadow);
    color: var(--muted);
    cursor: text;
    transition: width 0.2s;
  }
  .search:focus-within {
    border-color: var(--primary);
    outline: 2px solid var(--primary);
    outline-offset: 1px;
  }
  input {
    flex: 1;
    min-width: 0;
    /* the box around it is the field; skins style bare inputs, this one stays bare */
    border: 0 !important;
    outline: 0;
    background: transparent !important;
    color: var(--set-field-fg);
    font: inherit;
    padding: 0;
    box-shadow: none !important;
  }
  kbd {
    flex: none;
    padding: 1px 6px;
    border-radius: min(5px, var(--r-max));
    border: 1px solid var(--border);
    font: 500 11px var(--font);
    color: var(--faint);
  }
  .search:focus-within kbd,
  .filled kbd {
    display: none;
  }
  .list {
    width: 100%;
  }
  .list kbd {
    display: none;
  }
  /* the list is for the narrow layout, the header's box for the rest */
  @container settings (min-width: 600px) {
    .list {
      display: none;
    }
  }
  @container settings (max-width: 599px) {
    .head {
      display: none;
    }
  }
  /* icon rail: the box is an icon button (the input stays under it, invisible,
     so a click or the shortcut still focuses it) until it has focus or words */
  @container settings (min-width: 600px) and (max-width: 899px) {
    .head:not(:focus-within):not(.filled) {
      position: relative;
      width: var(--set-hit);
      justify-content: center;
      padding: 0;
    }
    .head:not(:focus-within):not(.filled) input {
      position: absolute;
      inset: 0;
      width: 100%;
      opacity: 0;
      cursor: pointer;
    }
    .head:not(:focus-within):not(.filled) kbd {
      display: none;
    }
  }
</style>
