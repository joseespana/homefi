<script lang="ts">
  // "Play next" / "Add to queue" for a track, an album or a playlist.
  import { pop } from "../lib/motion";
  import { app } from "../lib/state.svelte";
  import Icon from "./Icon.svelte";

  const menu = $derived(app.queueMenu!);
  const left = $derived(Math.max(12, Math.min(menu.x - 200, window.innerWidth - 212)));
  const top = $derived(Math.min(menu.y, window.innerHeight - 120));
  const close = () => (app.queueMenu = null);

  function run(next: boolean) {
    const m = menu;
    close();
    m.run(next);
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

<button class="scrim" aria-label="Close" onclick={close} oncontextmenu={(e) => (e.preventDefault(), close())}></button>
<div class="menu" style:left="{left}px" style:top="{top}px" role="menu" transition:pop>
  <div class="head ellipsis">{menu.label}</div>
  <button class="row" onclick={() => run(true)} role="menuitem"><Icon name="next" size={16} /> Play next</button>
  <button class="row" onclick={() => run(false)} role="menuitem"><Icon name="queue" size={16} /> Add to queue</button>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
    cursor: default;
  }
  .menu {
    position: fixed;
    z-index: 41;
    width: 200px;
    padding: 6px;
    border-radius: min(12px, var(--r-max));
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
  }
  .head {
    padding: 4px 8px 6px;
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
  }
  .row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px;
    border-radius: min(8px, var(--r-max));
    text-align: left;
  }
  .row:hover {
    background: var(--surface-2);
  }
</style>
