<script lang="ts">
  // The DJ setups' list and actions, shared by the DJ screen's Setups menu and Settings → DJ: save
  // the current settings under a name, apply, update, rename, delete, export (.homefidj) and import.
  // A setup is the whole DJ configuration: layout, pad sounds, shortcuts and every option.
  import { djSetups, SETUP_NAME_MAX } from "../../lib/dj-setups.svelte";
  import type { DjSetupSummary } from "../../lib/dj-api";
  import { ago } from "../../lib/transfer";
  import Icon from "../Icon.svelte";

  const setups = $derived(djSetups.list);
  let name = $state("");
  let renaming = $state<string | null>(null);
  let draft = $state("");
  /** A step waiting for "yes": what it says and what it does. */
  let pending = $state<{ id: string; text: string; yes: string; run: () => void } | null>(null);
  let field = $state<HTMLInputElement>();
  let cancel = $state<HTMLButtonElement>();

  $effect(() => {
    if (renaming && field) {
      field.focus();
      field.select();
    }
  });
  $effect(() => {
    if (pending) cancel?.focus();
  });

  async function saveNew() {
    const wanted = name.trim();
    if (!wanted) return;
    if (await djSetups.save(wanted)) name = "";
  }
  function startRename(s: DjSetupSummary) {
    pending = null;
    renaming = s.id;
    draft = s.name;
  }
  async function commitRename(s: DjSetupSummary) {
    const wanted = draft.trim();
    renaming = null;
    if (wanted && wanted !== s.name) await djSetups.rename(s.id, wanted);
  }
  function ask(s: DjSetupSummary, kind: "apply" | "overwrite" | "delete") {
    renaming = null;
    if (kind === "apply") {
      if (!djSetups.dirty) return djSetups.apply(s.id);
      pending = { id: s.id, text: "Your current DJ settings are not saved in a setup. Applying this one replaces them.", yes: "Apply it", run: () => djSetups.apply(s.id) };
    } else if (kind === "overwrite") {
      pending = { id: s.id, text: `Replace “${s.name}” with your current settings?`, yes: "Replace it", run: () => djSetups.overwrite(s.id) };
    } else {
      pending = { id: s.id, text: `Delete “${s.name}”? The file is HomeFi's own; your songs are not touched.`, yes: "Delete", run: () => djSetups.remove(s.id) };
    }
  }
  function confirm() {
    const step = pending;
    pending = null;
    step?.run();
  }
</script>

<div class="panel">
  <form class="new" onsubmit={(e) => (e.preventDefault(), saveNew())}>
    <input class="field" bind:value={name} maxlength={SETUP_NAME_MAX} placeholder="Name this setup, e.g. Party night" aria-label="Setup name" />
    <button type="submit" class="btn primary" disabled={!name.trim()} title="Saves the layout, pad sounds, shortcuts and every DJ option as they are now"><Icon name="save" size={16} />Save current</button>
  </form>

  {#if setups.length === 0}
    <p class="empty">No setups yet. Arrange the screen your way, then save it here. You can export a setup to use it on another computer, phone or tablet.</p>
  {:else}
    <ul class="list" aria-label="Saved setups">
      {#each setups as s (s.id)}
        <li class:active={djSetups.activeId === s.id}>
          <div class="line">
            {#if renaming === s.id}
              <form class="rename" onsubmit={(e) => (e.preventDefault(), commitRename(s))}>
                <input bind:this={field} bind:value={draft} class="field" maxlength={SETUP_NAME_MAX} aria-label="Setup name" onkeydown={(e) => e.key === "Escape" && (e.stopPropagation(), (renaming = null))} />
                <button type="submit" class="icon-btn" aria-label="Save the name" title="Save the name"><Icon name="check" size={17} /></button>
                <button type="button" class="icon-btn" aria-label="Cancel renaming" title="Cancel" onclick={() => (renaming = null)}><Icon name="x" size={17} /></button>
              </form>
            {:else}
              <span class="who">
                <b class="ell" title={s.name}>{s.name}</b>
                <small>{#if djSetups.activeId === s.id}In use{#if djSetups.dirty}, changed since{/if}{" · "}{/if}Updated {ago(s.updated_ms)}</small>
              </span>
              <span class="acts">
                <button type="button" class="btn tonal apply" onclick={() => ask(s, "apply")}>Apply</button>
                <button type="button" class="icon-btn" aria-label="Update {s.name} with the current settings" title="Update it with the current settings" onclick={() => ask(s, "overwrite")}><Icon name="save" size={17} /></button>
                <button type="button" class="icon-btn" aria-label="Rename {s.name}" title="Rename" onclick={() => startRename(s)}><Icon name="edit" size={17} /></button>
                <button type="button" class="icon-btn" aria-label="Export {s.name} to a file" title="Export to a file (.homefidj): move it to another computer, phone or tablet, or save it in your Google Drive folder" onclick={() => djSetups.exportSetup(s)}><Icon name="upload" size={17} /></button>
                <button type="button" class="icon-btn" aria-label="Delete {s.name}" title="Delete" onclick={() => ask(s, "delete")}><Icon name="trash" size={17} /></button>
              </span>
            {/if}
          </div>
          {#if pending?.id === s.id}
            <div class="confirm" role="alertdialog" aria-label={pending.text}>
              <span>{pending.text}</span>
              <button type="button" class="btn tonal" bind:this={cancel} onclick={() => (pending = null)} onkeydown={(e) => e.key === "Escape" && (e.stopPropagation(), (pending = null))}>Cancel</button>
              <button type="button" class="btn primary" onclick={confirm}>{pending.yes}</button>
            </div>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}

  <div class="foot">
    <button type="button" class="btn tonal" onclick={() => djSetups.importSetup()}><Icon name="download" size={16} />Import a setup</button>
    <small>.homefidj files from HomeFi on another computer, phone or tablet.</small>
  </div>
</div>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-width: 0;
  }
  .new {
    display: flex;
    gap: 8px;
  }
  .field {
    flex: 1;
    min-width: 0;
    min-height: 40px;
    padding: 0 12px;
    user-select: text;
  }
  .btn {
    min-height: 40px;
    padding: 0 14px;
    flex: none;
  }
  .empty {
    margin: 0;
    color: var(--muted);
    font-size: 13px;
    line-height: 1.5;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  li {
    padding: 6px 8px;
    border-radius: var(--r-md);
    border: 1px solid var(--border);
    background: var(--surface-2);
  }
  li.active {
    border-color: color-mix(in srgb, var(--primary) 55%, var(--border));
  }
  .line {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    min-height: 44px;
  }
  .who {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .who small {
    color: var(--muted);
    font-size: 12px;
  }
  .ell {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .acts {
    display: flex;
    align-items: center;
    gap: 2px;
    flex: none;
  }
  .apply {
    min-height: 36px;
    margin-right: 4px;
  }
  .icon-btn {
    width: 36px;
    height: 36px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    color: var(--muted);
  }
  .icon-btn:hover {
    background: var(--surface-3);
    color: var(--text);
  }
  .rename {
    display: flex;
    align-items: center;
    flex: 1;
    gap: 4px;
  }
  .confirm {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    padding: 6px 2px 2px;
    font-size: 13px;
    color: var(--muted);
  }
  .confirm span {
    flex: 1 1 200px;
  }
  .confirm .btn {
    min-height: 34px;
  }
  .foot {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .foot small {
    color: var(--muted);
  }
</style>
