<script lang="ts">
  // The queue of someone's room, with who added each song and a menu per song
  // (play now, play next, move up / down, remove) as far as the hub allows.
  import { formatTime, type QueueItem } from "../../lib/api";
  import { share } from "../../lib/share.svelte";
  import Cover from "../Cover.svelte";
  import Icon from "../Icon.svelte";

  let { compact = false }: { compact?: boolean } = $props();

  const queue = $derived(share.guest.state?.queue ?? []);
  let menu = $state<{ item: QueueItem; index: number; x: number; y: number } | null>(null);

  function open(e: MouseEvent, item: QueueItem, index: number) {
    e.preventDefault();
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    menu = { item, index, x: Math.min(r.right, window.innerWidth - 12), y: r.bottom + 4 };
  }

  function run(f: () => void) {
    menu = null;
    f();
  }
</script>

<div class="queue" class:compact>
  {#each queue as item, i (item.queue_id)}
    <div class="row" oncontextmenu={(e) => open(e, item, i)} role="listitem">
      <span class="n">{i + 1}</span>
      <Cover url={item.cover_url} size={compact ? 38 : 44} radius={7} />
      <div class="grow">
        <div class="ellipsis title">{item.title}{#if item.explicit}<span class="e" title="Explicit">E</span>{/if}</div>
        <div class="muted ellipsis sub">{item.artist}{item.duration_ms ? ` · ${formatTime(item.duration_ms)}` : ""}</div>
      </div>
      <span class="who" title="Added by {item.added_by.name}">
        <span class="dot" style:background={item.added_by.color}></span>
        {#if !compact}<span class="ellipsis">{item.added_by.guest_id === share.guest.guest_id ? "You" : item.added_by.name}</span>{/if}
      </span>
      <button class="icon-btn" aria-label="Song options" title="Song options" onclick={(e) => open(e, item, i)}><Icon name="more" size={16} /></button>
    </div>
  {:else}
    <p class="muted empty">Nothing queued. Search and add a song.</p>
  {/each}
</div>

{#if menu}
  {@const m = menu}
  {@const edit = share.canEdit(m.item)}
  <button class="scrim" aria-label="Close" onclick={() => (menu = null)}></button>
  <div class="menu" role="menu" style:left="{m.x}px" style:top="{m.y}px">
    <button role="menuitem" disabled={!share.canPlayNow} title={share.canPlayNow ? "" : "The hub decides what plays"} onclick={() => run(() => share.playNow(m.item))}>
      <Icon name="play" size={14} /> Play now
    </button>
    <button role="menuitem" disabled={!edit || m.index === 0} onclick={() => run(() => share.move(m.item, 0))}><Icon name="next" size={14} /> Play next</button>
    <button role="menuitem" disabled={!edit || m.index === 0} onclick={() => run(() => share.move(m.item, m.index - 1))}><Icon name="up" size={14} /> Move up</button>
    <button role="menuitem" disabled={!edit || m.index >= queue.length - 1} onclick={() => run(() => share.move(m.item, m.index + 1))}><Icon name="down" size={14} /> Move down</button>
    <button role="menuitem" class="danger" disabled={!edit} onclick={() => run(() => share.remove(m.item))}><Icon name="trash" size={14} /> Remove</button>
    {#if share.editHint(m.item)}<p class="hint muted">{share.editHint(m.item)}</p>{/if}
  </div>
{/if}

<style>
  .queue {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 6px 6px 8px;
    border-radius: min(12px, var(--r-max));
    background: var(--glass);
    border: 1px solid var(--glass-border);
  }
  .n {
    width: 18px;
    text-align: right;
    font-size: 12px;
    color: var(--faint);
    font-variant-numeric: tabular-nums;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .title {
    font-weight: 600;
  }
  .sub {
    font-size: 12px;
  }
  .e {
    margin-left: 6px;
    padding: 0 4px;
    border-radius: min(3px, var(--r-max));
    font-size: 10px;
    background: var(--surface-3);
    color: var(--muted);
  }
  .who {
    display: flex;
    align-items: center;
    gap: 6px;
    max-width: 120px;
    font-size: 12px;
    color: var(--muted);
  }
  .dot {
    flex: none;
    width: 10px;
    height: 10px;
    border-radius: 50%;
  }
  .empty {
    padding: 12px 4px;
  }
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 55;
    cursor: default;
  }
  .menu {
    position: fixed;
    z-index: 56;
    transform: translateX(-100%);
    min-width: 200px;
    max-width: 260px;
    padding: 6px;
    display: flex;
    flex-direction: column;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: min(12px, var(--r-max));
    box-shadow: var(--shadow);
  }
  .menu button {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-radius: min(8px, var(--r-max));
    text-align: left;
  }
  .menu button:hover:not(:disabled) {
    background: var(--surface-2);
  }
  .menu .danger {
    color: var(--danger);
  }
  .hint {
    margin: 4px 10px 4px;
    font-size: 12px;
  }
</style>
