<script lang="ts">
  import { api } from "../lib/api";
  import { share } from "../lib/share.svelte";
  import { shownSource } from "../lib/source.svelte";
  import { app } from "../lib/state.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";
  import RoomQueue from "./share/RoomQueue.svelte";
  import { drawer } from "../lib/motion";

  let { onclose }: { onclose: () => void } = $props();
  const queue = $derived(app.player?.queue ?? []);
  const current = $derived(app.player?.current ?? -1);
  const name = (p: string) => p.split(/[\\/]/).pop()?.replace(/\.[^.]+$/, "") ?? p;

  // drag to reorder (pointer events: HTML5 drag & drop is unreliable inside some webviews)
  let list = $state<HTMLElement>();
  let drag = $state<{ from: number; to: number; dy: number; startY: number; pointer: number } | null>(null);
  const ROW = 60;

  function grab(e: PointerEvent, i: number) {
    if (e.button !== 0) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    drag = { from: i, to: i, dy: 0, startY: e.clientY, pointer: e.pointerId };
  }

  function move(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.pointer) return;
    const dy = e.clientY - drag.startY;
    drag.dy = dy;
    drag.to = Math.max(0, Math.min(queue.length - 1, drag.from + Math.round(dy / ROW)));
    // keep the dragged row visible near the edges
    const box = list?.parentElement;
    if (box) {
      const r = box.getBoundingClientRect();
      if (e.clientY < r.top + 60) box.scrollTop -= 8;
      else if (e.clientY > r.bottom - 40) box.scrollTop += 8;
    }
  }

  function drop() {
    if (!drag) return;
    const { from, to } = drag;
    drag = null;
    if (from !== to) {
      // show the new order right away; the engine confirms with the next player event
      if (app.player) {
        const q = [...app.player.queue];
        q.splice(to, 0, ...q.splice(from, 1));
        const cur = app.player.current;
        const moved = cur === from ? to : cur != null && from < cur && to >= cur ? cur - 1 : cur != null && from > cur && to <= cur ? cur + 1 : cur;
        app.player = { ...app.player, queue: q, current: moved };
      }
      api.moveTrack(from, to);
    }
  }

  /** How far a row shifts while another one is dragged over it. */
  function offset(i: number): number {
    if (!drag) return 0;
    if (i === drag.from) return drag.dy;
    if (drag.from < i && i <= drag.to) return -ROW;
    if (drag.to <= i && i < drag.from) return ROW;
    return 0;
  }
</script>

<aside class="panel queue-panel" in:drawer>
  {#if shownSource.remote}
    <div class="head">
      <b class="ellipsis">{share.guest.session_name}</b>
      <span class="muted count">{share.guest.state?.queue.length ?? 0} up next</span>
      <button class="icon-btn" onclick={onclose} aria-label="Close queue"><Icon name="x" /></button>
    </div>
    <RoomQueue compact />
  {:else}
  <div class="head">
    <b>Queue</b>
    <span class="muted count">{queue.length + app.upcoming.length ? `${queue.length + app.upcoming.length} tracks` : ""}</span>
    <button class="icon-btn" onclick={onclose} aria-label="Close queue"><Icon name="x" /></button>
  </div>
  <div class="list" bind:this={list} class:dragging={drag}>
    {#each queue as path, i (i + path)}
      {@const t = app.trackAt(path)}
      <div
        class="row"
        class:current={i === current}
        class:lifted={drag?.from === i}
        style:transform="translateY({offset(i)}px)"
      >
        <button class="item" onclick={() => api.jump(i)} title="Play">
          <span class="n">{#if i === current}<Icon name="play" size={12} />{:else}{i + 1}{/if}</span>
          <Cover {path} size={42} radius={6} />
          <div class="grow">
            <div class="ellipsis title">{t?.title ?? name(path)}</div>
            <div class="sub ellipsis">{t?.artist ?? ""}{t?.album ? ` · ${t.album}` : ""}</div>
          </div>
        </button>
        <span
          class="handle"
          role="button"
          tabindex="-1"
          aria-label="Drag to reorder"
          title="Drag to reorder"
          onpointerdown={(e) => grab(e, i)}
          onpointermove={move}
          onpointerup={drop}
          onpointercancel={drop}
        >
          <svg viewBox="0 0 12 20" width="10" height="16" aria-hidden="true">
            {#each [3, 10, 17] as y}<circle cx="3" cy={y} r="1.6" /><circle cx="9" cy={y} r="1.6" />{/each}
          </svg>
        </span>
      </div>
    {:else}
      {#if !app.upcoming.length}<p class="muted empty">Nothing queued</p>{/if}
    {/each}
    {#if app.upcoming.length}
      <!-- the rest of a Tidal stream: fetched just before each one plays -->
      <div class="later-head muted">Coming up from Tidal · {app.upcoming.length}</div>
      {#each app.upcoming as u, j (u.id + j)}
        <div class="row later">
          <button class="item" onclick={() => app.playUpcoming(j)} title="Play now">
            <span class="n">{queue.length + j + 1}</span>
            <Cover size={42} radius={6} icon="music" />
            <div class="grow">
              <div class="ellipsis title">{u.title}</div>
              <div class="sub ellipsis">{u.artist}</div>
            </div>
          </button>
        </div>
      {/each}
    {/if}
  </div>
  {/if}
</aside>

<style>
  .later {
    opacity: 0.7;
  }
  .later-head {
    margin: 12px 10px 4px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .panel {
    width: 320px;
    max-width: 100vw;
    border-left: 1px solid var(--border);
    background: var(--sidebar);
    overflow-y: auto;
    padding: 0 14px 16px;
    scrollbar-gutter: stable;
  }
  .head {
    position: sticky;
    top: 0;
    z-index: 2;
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 34px 4px 10px;
    background: var(--sidebar);
  }
  .count {
    flex: 1;
    font-size: 12px;
  }
  .list {
    position: relative;
  }
  .row {
    position: relative;
    height: 60px;
    display: flex;
    align-items: center;
    border-radius: min(10px, var(--r-max));
    transition: transform 0.18s ease;
  }
  .row:hover {
    background: var(--surface-2);
  }
  .dragging .row {
    user-select: none;
  }
  .row.lifted {
    z-index: 1;
    transition: none;
    background: var(--surface-3);
    box-shadow: var(--shadow);
  }
  .item {
    flex: 1;
    min-width: 0;
    height: 100%;
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 0 4px 0 8px;
    text-align: left;
  }
  .current .title {
    color: var(--primary);
    font-weight: 600;
  }
  .n {
    width: 20px;
    display: flex;
    justify-content: flex-end;
    font-size: 12px;
    color: var(--faint);
    flex: none;
  }
  .current .n {
    color: var(--primary);
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .sub {
    font-size: 12px;
    color: var(--muted);
  }
  .handle {
    flex: none;
    width: 32px;
    height: 40px;
    margin-right: 6px;
    border-radius: min(8px, var(--r-max));
    display: grid;
    place-items: center;
    cursor: grab;
    color: var(--faint);
    fill: currentColor;
    touch-action: none;
    opacity: 0;
  }
  .handle:hover {
    background: var(--surface-3);
    color: var(--text);
  }
  .row:hover .handle,
  .lifted .handle {
    opacity: 1;
  }
  .lifted .handle {
    cursor: grabbing;
  }
  .empty {
    padding: 12px;
  }
</style>
