<script lang="ts">
  // The "Setups" button of the DJ top bar and the popover with the setups' list.
  import { djSetups } from "../../lib/dj-setups.svelte";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";
  import DjSetupsPanel from "./DjSetupsPanel.svelte";

  let open = $state(false);
  let root = $state<HTMLElement>();
  let pop = $state<HTMLElement>();
  let button = $state<HTMLButtonElement>();

  $effect(() => {
    if (open) pop?.focus();
  });
  function close(refocus = true) {
    open = false;
    if (refocus) button?.focus();
  }
</script>

<svelte:window onpointerdown={(e) => open && root && !root.contains(e.target as Node) && close(false)} />

<div class="menu" bind:this={root}>
  <button class="btn hbtn" class:on={open} type="button" bind:this={button} aria-label="Setups" aria-haspopup="dialog" aria-expanded={open} title="Save the way DJ mode is set up, or load another setup" onclick={() => (open = !open)}>
    <Icon name="layers" size={16} /><span class="long">Setups{djSetups.list.length ? ` · ${djSetups.list.length}` : ""}</span>
  </button>
  {#if open}
    <div class="pop" role="dialog" aria-label="DJ setups" tabindex="-1" bind:this={pop} onkeydown={(e) => e.key === "Escape" && (e.stopPropagation(), close())}>
      <header>
        <h2>Setups</h2>
        <button type="button" class="link" onclick={() => ((app.view = { name: "settings", page: "dj" }), close(false))}>In Settings</button>
      </header>
      <DjSetupsPanel />
    </div>
  {/if}
</div>

<style>
  .menu {
    position: relative;
  }
  .hbtn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  @media (max-width: 1179px) {
    .long {
      display: none;
    }
  }
  .pop {
    position: absolute;
    top: calc(100% + 8px);
    right: 0;
    z-index: 40;
    width: min(440px, calc(100vw - 32px));
    max-height: min(560px, calc(100vh - 140px));
    overflow-y: auto;
    padding: 14px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
    box-shadow: var(--shadow);
  }
  .pop:focus-visible {
    outline: 2px solid var(--primary);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 10px;
  }
  h2 {
    margin: 0;
    font-size: 15px;
  }
  .link {
    color: var(--primary);
    font-size: 13px;
    font-weight: 600;
  }
  .link:hover {
    text-decoration: underline;
  }
</style>
