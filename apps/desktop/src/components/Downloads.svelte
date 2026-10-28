<script lang="ts">
  import { item, reflow } from "../lib/motion";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { api } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import Icon from "./Icon.svelte";

  const active = $derived(Object.values(app.active));
  const icon: Record<string, string> = { done: "check", exists: "check", skipped: "x", error: "alert" };

  async function cancel() {
    const left = app.queued - app.finished;
    const ok = await app.confirm({
      title: "Cancel the downloads?",
      text: `${left > 0 ? (left === 1 ? "1 item that hasn't finished" : `${left} items that haven't finished`) : "What's still downloading"} will stop. Finished files stay in your library.`,
      confirmLabel: "Cancel downloads",
      cancelLabel: "Keep downloading",
      danger: true,
    });
    if (ok) api.cancelDownloads();
  }

  async function clear() {
    const ok = await app.confirm({
      title: "Clear the download list?",
      text: "The list of finished downloads is emptied. The files stay in your library.",
      confirmLabel: "Clear",
    });
    if (!ok) return;
    app.log = [];
    app.queued = 0;
    app.finished = 0;
  }

  function mb(bytes: number) {
    return `${(bytes / 1048576).toFixed(1)} MB`;
  }
</script>

<header class="head">
  <div>
    <h1>Downloads</h1>
    <div class="muted">
      {#if app.busy}{app.jobLabel} · {app.finished} of {app.queued} items{:else if app.log.length}{app.finished} items{:else}Nothing yet{/if}
    </div>
  </div>
  <div class="actions">
    {#if app.busy}
      <button class="btn tonal" onclick={cancel}><Icon name="x" size={16} /> Cancel</button>
    {:else if app.log.length}
      <button class="btn tonal" onclick={clear}>Clear</button>
    {/if}
  </div>
</header>

{#if app.busy}
  <div class="progress-total">
    <div class="bar" style:width="{app.queued ? (100 * app.finished) / app.queued : 0}%"></div>
  </div>
{/if}

{#each active as d (d.task)}
  <div class="row" in:item|global animate:reflow>
    <span class="spinner"></span>
    <div class="grow">
      <div class="ellipsis">{d.title}</div>
      <div class="muted small">{d.quality} · {mb(d.downloaded)}{d.total ? ` / ${mb(d.total)}` : ""}</div>
      <div class="track"><div class="fill" style:width="{d.total ? (100 * d.downloaded) / d.total : 30}%" class:indeterminate={!d.total}></div></div>
    </div>
  </div>
{/each}

{#each app.log as entry (entry.id)}
  <div class="row {entry.kind}" in:item|global animate:reflow>
    <span class="state"><Icon name={icon[entry.kind]} size={16} /></span>
    <div class="grow">
      <div class="ellipsis">{entry.title}</div>
      <div class="muted small ellipsis">{entry.detail}</div>
    </div>
    {#if entry.path}
      <button class="icon-btn" title="Play" aria-label="Play {entry.title}" onclick={() => api.play([entry.path!])}>
        <Icon name="play" size={16} />
      </button>
      <button class="icon-btn" title="Show in folder" aria-label="Show {entry.title} in folder" onclick={() => revealItemInDir(entry.path!)}>
        <Icon name="folder" size={16} />
      </button>
    {/if}
  </div>
{/each}

{#if !app.busy && !app.log.length}
  <div class="empty muted">
    <Icon name="download" size={40} />
    <p>Find something in <button class="link" onclick={() => (app.view = { name: "search" })}>Search</button> or paste a Tidal link there.</p>
  </div>
{/if}

<style>
  .head {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    margin-bottom: 16px;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
  .progress-total {
    height: 4px;
    border-radius: min(4px, var(--r-max));
    background: var(--surface-3);
    margin-bottom: 12px;
    overflow: hidden;
  }
  .progress-total .bar {
    height: 100%;
    background: var(--gradient);
    transition: width 0.3s;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px;
    border-radius: min(10px, var(--r-max));
  }
  .row:hover {
    background: var(--surface-2);
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .small {
    font-size: 12.5px;
  }
  .track {
    height: 4px;
    margin-top: 6px;
    border-radius: min(4px, var(--r-max));
    background: var(--surface-3);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--primary);
  }
  .fill.indeterminate {
    animation: slide 1.2s ease-in-out infinite;
  }
  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(330%);
    }
  }
  .state {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: var(--surface-3);
  }
  .done .state {
    color: var(--ok);
  }
  .exists .state {
    color: var(--brand-sky);
  }
  .error .state {
    color: var(--danger);
  }
  .spinner {
    width: 28px;
    height: 28px;
    border: 3px solid var(--surface-3);
    border-top-color: var(--primary);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 80px 20px;
  }
  .link {
    color: var(--primary);
    font-weight: 600;
  }
</style>
