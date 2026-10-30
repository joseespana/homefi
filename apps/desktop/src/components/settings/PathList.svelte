<script lang="ts">
  // Folders on disk: one row each with a remove button, then "Add folder". Empty, it says so.
  import Icon from "../Icon.svelte";
  import { useRowSearch } from "./rowSearch.svelte";

  let {
    label,
    paths,
    icon = "folder",
    addLabel,
    emptyText,
    onadd,
    onremove,
  }: {
    /** What the list is, for the search and screen readers. */
    label: string;
    paths: readonly string[];
    icon?: string;
    addLabel: string;
    emptyText: string;
    onadd: () => void;
    onremove: (path: string) => void;
  } = $props();

  const search = useRowSearch(() => `${label} ${addLabel} ${emptyText}`);
</script>

<div class="paths" role="group" aria-label={label} hidden={!search.visible}>
  {#each paths as path (path)}
    <div class="path">
      <Icon name={icon} />
      <span class="ellipsis" title={path}>{path}</span>
      <button type="button" class="icon-btn" title="Remove" aria-label="Remove {path}" onclick={() => onremove(path)}><Icon name="x" /></button>
    </div>
  {:else}
    <p class="empty">{emptyText}</p>
  {/each}
  <button type="button" class="add" onclick={onadd}><Icon name="plus" size={16} /> {addLabel}</button>
</div>

<style>
  .paths[hidden] {
    display: none;
  }
  .path {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    min-height: var(--set-row-min);
    padding: var(--s-2) var(--s-2) var(--s-2) var(--s-4);
    color: var(--muted);
  }
  .path span {
    flex: 1;
    min-width: 0;
    color: var(--text);
  }
  .path + .path,
  .path + .add,
  .empty + .add {
    border-top: 1px solid var(--set-divider);
  }
  .icon-btn {
    width: var(--set-hit);
    height: var(--set-hit);
  }
  .empty {
    margin: 0;
    padding: var(--s-4);
    font: var(--set-hint-font);
    color: var(--muted);
  }
  .add {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    width: 100%;
    min-height: var(--set-row-min);
    padding: var(--s-3) var(--s-4);
    font: var(--set-label-font);
    color: var(--primary);
    text-align: left;
    transition: background 0.15s;
  }
  .add:hover {
    background: var(--set-row-hover);
  }
  .add:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: -2px;
  }
</style>
