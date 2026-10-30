<script lang="ts">
  // What a right-click on the mascot offers: a wave, the next style, sending it back
  // to the room when it floats, then a famous dance (or a surprise one for the mood)
  // while the music plays. It opens at the pointer and lives on <body>.
  import { DANCES, type DanceId } from "../lib/dance";
  import { pop } from "../lib/motion";
  import { portal } from "../lib/portal";
  import Icon from "./Icon.svelte";

  let {
    x,
    y,
    playing,
    dancing,
    floating,
    onclose,
    ondance,
    onwave,
    onstyle,
    ondock,
  }: {
    x: number;
    y: number;
    playing: boolean;
    dancing: DanceId | null;
    floating: boolean;
    onclose: () => void;
    ondance: (id?: DanceId) => void;
    onwave: () => void;
    onstyle: () => void;
    ondock: () => void;
  } = $props();

  const WIDTH = 210;
  let menu = $state<HTMLDivElement>();
  let height = $state(0);
  // beside the pointer (on its left when there is no room on the right, so the
  // mascot stays in view while it dances), kept inside the window
  const left = $derived(Math.max(12, x + 8 + WIDTH + 12 <= window.innerWidth ? x + 8 : x - WIDTH - 8));
  const top = $derived(Math.max(12, Math.min(y, window.innerHeight - height - 12)));

  function run(action: () => void) {
    onclose();
    action();
  }

  $effect(() => menu?.querySelector<HTMLButtonElement>(".row")?.focus());

  // arrow keys walk the rows, Escape closes
  function keys(e: KeyboardEvent) {
    if (e.key === "Escape") return onclose();
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const rows = [...(menu?.querySelectorAll<HTMLButtonElement>(".row") ?? [])];
    const at = rows.indexOf(document.activeElement as HTMLButtonElement);
    rows[(at + (e.key === "ArrowDown" ? 1 : rows.length - 1)) % rows.length]?.focus();
  }
</script>

<svelte:window onkeydown={keys} onresize={onclose} />

<div use:portal>
  <button class="scrim" aria-label="Close" onclick={onclose} oncontextmenu={(e) => (e.preventDefault(), onclose())}></button>
  <div
    class="menu"
    bind:this={menu}
    bind:clientHeight={height}
    style:left="{left}px"
    style:top="{top}px"
    style:width="{WIDTH}px"
    role="menu"
    aria-label="Mascot"
    transition:pop={{ origin: "top left" }}
  >
    <button class="row" role="menuitem" onclick={() => run(onwave)}><Icon name="smile" size={16} /> Wave</button>
    <button class="row" role="menuitem" onclick={() => run(onstyle)}><Icon name="palette" size={16} /> Next style</button>
    {#if floating}
      <button class="row" role="menuitem" onclick={() => run(ondock)}><Icon name="back" size={16} /> Back to the room</button>
    {/if}
    <div class="sep"></div>
    {#if playing}
      <div class="head">Dance</div>
      <button class="row strong" role="menuitem" onclick={() => run(() => ondance())}><Icon name="shuffle" size={16} /> Surprise me</button>
      {#each DANCES as d (d.id)}
        <button class="row" class:on={d.id === dancing} role="menuitem" title={d.hint} onclick={() => run(() => ondance(d.id))}>
          <Icon name="music" size={16} />
          {d.label}
        </button>
      {/each}
    {:else}
      <p class="note">Play some music and it dances.</p>
    {/if}
  </div>
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
    max-height: calc(100vh - 24px);
    overflow-y: auto;
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
  .note {
    margin: 0;
    padding: 6px 8px;
    font-size: 12.5px;
    color: var(--muted);
  }
  .sep {
    height: 1px;
    margin: 4px 6px;
    background: var(--border);
  }
  .row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 8px;
    border-radius: min(8px, var(--r-max));
    text-align: left;
  }
  .row:hover,
  .row:focus-visible {
    background: var(--surface-2);
    outline: none;
  }
  .strong {
    font-weight: 600;
  }
  .on {
    color: var(--primary);
    font-weight: 600;
  }
</style>
