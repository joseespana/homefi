<script lang="ts">
  // Settings: the page list, the sticky header with the search, and the open page.
  // The layout follows the width Settings gets (container queries below): a
  // centred reading column beside a page list, an icon rail, or a narrow
  // window's list / page pair. Pages live in ./settings; this file owns the
  // choice of page, the search and the keyboard.
  import { onDestroy } from "svelte";
  import { api } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import AiOptions from "./AiOptions.svelte";
  import Icon from "./Icon.svelte";
  import PlaybackOptions from "./PlaybackOptions.svelte";
  import Skeleton from "./Skeleton.svelte";
  import { SettingsModel, provideSettings } from "./settings/model.svelte";
  import { PAGES, isPageId, type PageId } from "./settings/pages";
  import AboutPage from "./settings/pages/AboutPage.svelte";
  import DownloadsPage from "./settings/pages/DownloadsPage.svelte";
  import LibraryPage from "./settings/pages/LibraryPage.svelte";
  import LookPage from "./settings/pages/LookPage.svelte";
  import LyricsPage from "./settings/pages/LyricsPage.svelte";
  import MascotPage from "./settings/pages/MascotPage.svelte";
  import RoomPage from "./settings/pages/RoomPage.svelte";
  import SharePage from "./settings/pages/SharePage.svelte";
  import SoundPage from "./settings/pages/SoundPage.svelte";
  import TidalPage from "./settings/pages/TidalPage.svelte";
  import { settingsSearch } from "./settings/search.svelte";
  import SettingsNav from "./settings/SettingsNav.svelte";
  import SettingsPage from "./settings/SettingsPage.svelte";
  import { summaryOf } from "./settings/summary";

  const model = new SettingsModel();
  provideSettings(model);
  onDestroy(() => {
    model.dispose();
    settingsSearch.clear();
  });

  $effect(() => {
    let stale = false;
    api.getSettings().then((s) => {
      if (!stale) model.settings = s;
    });
    return () => (stale = true);
  });
  // options saved elsewhere (the mascot ones, the room menu) show up here too
  $effect(() => {
    if (app.settings) model.settings = app.settings;
  });

  const STORE = "homefi.settings.page";
  /** Below this width (px) Settings is a list, then a page (the CSS breakpoint says the same). */
  const LIST_MAX_PX = 600;

  function readPage(): PageId {
    try {
      const saved = localStorage.getItem(STORE);
      if (isPageId(saved)) return saved;
    } catch {}
    return "look";
  }

  let page = $state<PageId>(readPage());
  /** Narrow layout: showing a page rather than the list. */
  let detail = $state(false);
  let width = $state(0);
  const narrow = $derived(width > 0 && width < LIST_MAX_PX);

  function go(id: PageId) {
    page = id;
    detail = true;
    try {
      localStorage.setItem(STORE, id);
    } catch {}
  }

  // opened on a section (the equalizer's "Equalizer settings", the assistant's "Set up AI")
  $effect(() => {
    const v = app.view;
    if (v.name === "settings" && v.page && isPageId(v.page)) go(v.page);
  });

  const shown = $derived(PAGES.filter((p) => settingsSearch.matches(`${p.title} ${p.words}`)));
  // typing jumps to the first match
  $effect(() => {
    if (settingsSearch.active && shown.length && !shown.some((p) => p.id === page)) page = shown[0].id;
  });
  const current = $derived(PAGES.find((p) => p.id === page) ?? PAGES[0]);

  let root = $state<HTMLElement>();
  function focusSearch() {
    const box = [...(root?.querySelectorAll<HTMLInputElement>("input[type=search]") ?? [])].find((el) => el.getClientRects().length > 0);
    box?.focus();
    box?.select();
  }

  function onKey(e: KeyboardEvent) {
    if (app.view.name !== "settings") return;
    const target = e.target as HTMLElement | null;
    const typing = !!target && (target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName));
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
      e.preventDefault();
      focusSearch();
    } else if (e.key === "/" && !typing && !e.metaKey && !e.ctrlKey) {
      e.preventDefault();
      focusSearch();
    } else if (e.key === "Escape") {
      if (target instanceof HTMLInputElement && target.type === "search") {
        if (settingsSearch.query) settingsSearch.clear();
        else target.blur();
      } else if (narrow && detail && !typing) {
        detail = false;
      }
    }
  }
</script>

<svelte:window onkeydown={onKey} />

{#if model.settings}
  {@const settings = model.settings}
  <div class="settings" data-width={settings.room.settings_width ?? "comfortable"} bind:this={root} bind:clientWidth={width}>
    <div class="frame" data-detail={detail} data-empty={shown.length === 0 || undefined}>
      <SettingsNav pages={shown} {page} summaryOf={(id) => summaryOf(id, settings)} version={app.info?.version ?? ""} onselect={go} />
      <div class="pane">
        <!-- with no match the header (and its search box) stays, so the words can be edited -->
        <SettingsPage title={shown.length ? current.title : "Settings"} summary={shown.length ? summaryOf(page, settings) : undefined} onback={() => (detail = false)}>
          {#if shown.length}
            {#if page === "look"}
              <LookPage {settings} goto={go} />
            {:else if page === "room"}
              <RoomPage {settings} />
            {:else if page === "lyrics"}
              <LyricsPage {settings} />
            {:else if page === "mascot"}
              <MascotPage {settings} />
            {:else if page === "sound"}
              <SoundPage {settings} />
            {:else if page === "playback"}
              <PlaybackOptions {settings} save={(patch) => model.save(patch)} />
            {:else if page === "ai"}
              <AiOptions {settings} save={(patch) => model.save(patch)} />
            {:else if page === "share"}
              <SharePage {settings} />
            {:else if page === "downloads"}
              <DownloadsPage {settings} />
            {:else if page === "library"}
              <LibraryPage {settings} />
            {:else if page === "tidal"}
              <TidalPage {settings} />
            {:else if page === "about"}
              <AboutPage {settings} />
            {/if}
          {/if}
        </SettingsPage>
        {#if model.error}
          <div class="errbar" role="alert">
            <Icon name="alert" size={16} />
            <span>{model.error}</span>
            <button type="button" class="icon-btn" aria-label="Dismiss" onclick={() => model.dismissError()}><Icon name="x" size={16} /></button>
          </div>
        {/if}
      </div>
    </div>
  </div>
{:else}
  <div class="settings loading" aria-busy="true" aria-label="Loading settings">
    {#each { length: 3 } as _, i (i)}
      <div class="skel-card"><Skeleton kind="rows" count={3} /></div>
    {/each}
  </div>
{/if}

<style>
  .settings {
    container: settings / inline-size;
    width: 100%;
  }
  .frame {
    --col: var(--set-col-w);
    display: grid;
    grid-template-columns: var(--set-nav-w) minmax(0, 1fr);
    column-gap: var(--set-gap);
    align-items: start;
    /* the page list and the column are one block, centred as a block */
    max-width: calc(var(--set-nav-w) + var(--set-gap) + var(--col));
    margin-inline: auto;
    padding-top: var(--s-2);
  }
  .pane {
    min-width: 0;
  }

  /* the reading column grows a little, never the lines */
  @container settings (min-width: 1280px) {
    .frame {
      --col: var(--set-col-w-lg);
    }
  }
  /* very wide: groups flow in two columns */
  @container settings (min-width: 1800px) {
    .settings:not([data-width="wide"]) .frame {
      --col: calc(2 * var(--set-colgroup-w) + var(--s-5));
      --set-body-cols: repeat(2, minmax(0, var(--set-colgroup-w)));
    }
  }
  /* "wide" (Look → Settings width): one broader column */
  .settings[data-width="wide"] .frame {
    --col: var(--set-col-w-wide);
  }
  /* icon rail */
  @container settings (min-width: 600px) and (max-width: 899px) {
    .frame {
      --set-nav-w: var(--set-rail-w);
      --set-gap: var(--s-4);
    }
  }
  /* narrow: the list, then the page (back returns to the list) */
  @container settings (max-width: 599px) {
    .frame {
      grid-template-columns: minmax(0, 1fr);
      max-width: none;
    }
    .frame[data-detail="false"]:not([data-empty]) .pane {
      display: none;
    }
    .frame[data-detail="true"] :global(nav) {
      display: none;
    }
    .frame[data-empty] :global(nav .group) {
      display: none;
    }
  }

  .errbar {
    position: sticky;
    bottom: var(--s-4);
    z-index: 3;
    display: flex;
    align-items: center;
    gap: var(--s-3);
    max-width: var(--col);
    margin: var(--s-4) auto 0;
    padding: var(--s-2) var(--s-2) var(--s-2) var(--s-4);
    border: 1px solid var(--danger);
    border-radius: var(--set-card-r);
    background: color-mix(in srgb, var(--danger) 14%, var(--surface));
    color: var(--text);
    box-shadow: var(--shadow);
  }
  .errbar :global(svg) {
    flex: none;
    color: var(--danger);
  }
  .errbar span {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .loading {
    display: grid;
    gap: var(--s-5);
    max-width: var(--set-col-w);
    margin-inline: auto;
    padding-top: var(--s-5);
  }
  .skel-card {
    padding: var(--s-4);
    border: 1px solid var(--set-card-border);
    border-radius: var(--set-card-r);
    background: var(--set-card-bg);
  }
</style>
