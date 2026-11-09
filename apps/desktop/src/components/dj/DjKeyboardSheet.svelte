<script lang="ts">
  // The optional shortcuts, read from the core keymap (with the user's changes). Esc closes it.
  import { actionLabel as name, dj, printKey } from "../../lib/dj.svelte";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";

  const groups = $derived([
    { title: "Deck A", list: dj.keymap.filter((b) => b.action.startsWith("deck_a.")) },
    { title: "Deck B", list: dj.keymap.filter((b) => b.action.startsWith("deck_b.")) },
    { title: "Mixer", list: dj.keymap.filter((b) => b.action.startsWith("crossfader.") || b.action.startsWith("auto_mix.")) },
    { title: "Sampler", list: dj.keymap.filter((b) => b.action.startsWith("sampler.")) },
  ]);

  let card = $state<HTMLElement>();
  $effect(() => {
    const before = document.activeElement as HTMLElement | null;
    card?.focus();
    return () => before?.focus?.();
  });
  function close() {
    dj.sheetOpen = false;
  }
</script>

<svelte:window onkeydowncapture={(e) => e.key === "Escape" && (e.stopPropagation(), close())} />

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && close()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="dj-keys-title" tabindex="-1" bind:this={card}>
    <header>
      <h2 id="dj-keys-title">Keyboard shortcuts</h2>
      <button class="icon-btn" type="button" aria-label="Close" onclick={close}><Icon name="x" size={18} /></button>
    </header>
    <p class="muted">All optional. You can do everything with the mouse. <kbd>?</kbd> opens this sheet.</p>
    {#if !dj.settings.keys_enabled}<p class="off">Shortcuts are off. Turn them on in Settings → DJ.</p>{/if}
    <div class="cols">
      {#each groups as g (g.title)}
        <section>
          <h3>{g.title}</h3>
          <dl>
            {#each g.list as b (b.action)}
              <dt>{name(b.action)}</dt>
              <dd><kbd>{printKey(b.key)}</kbd></dd>
            {/each}
          </dl>
        </section>
      {/each}
    </div>
    <button
      class="btn tonal"
      type="button"
      onclick={() => {
        close();
        app.view = { name: "settings", page: "dj" };
      }}>Change them in Settings</button
    >
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: grid;
    place-items: center;
    background: rgb(4 10 18 / 0.6);
  }
  .dialog {
    width: min(760px, calc(100vw - 32px));
    max-height: calc(100vh - 64px);
    overflow-y: auto;
    padding: 20px 24px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    align-items: flex-start;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
    box-shadow: var(--shadow);
  }
  .dialog:focus-visible {
    outline: 2px solid var(--primary);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
  }
  h2 {
    margin: 0;
  }
  p {
    margin: 0;
  }
  .off {
    color: var(--warn);
  }
  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
    gap: 16px;
    width: 100%;
  }
  h3 {
    margin: 0 0 6px;
    font-size: 12px;
    color: var(--faint);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  dl {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 4px 10px;
    margin: 0;
    font-size: 13px;
  }
  dt {
    color: var(--muted);
  }
  dd {
    margin: 0;
  }
  kbd {
    padding: 1px 6px;
    border-radius: var(--r-xs);
    border: 1px solid var(--border);
    background: var(--surface-2);
    font: 12px var(--font-mono);
  }
</style>
