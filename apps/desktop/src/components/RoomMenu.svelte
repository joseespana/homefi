<script lang="ts">
  import { pop } from "../lib/motion";
  import type { PlayerLayout, RoomSettings } from "../lib/api";
  import { LAYOUTS, layoutOf } from "../lib/layouts";
  import { app } from "../lib/state.svelte";
  import MascotOptions from "./MascotOptions.svelte";

  let { onclose }: { onclose: () => void } = $props();
  const room = $derived(app.settings?.room);

  const all: { key: keyof RoomSettings; label: string; classic?: boolean }[] = [
    { key: "signal_chain", label: "Signal chain", classic: true },
    { key: "vu_meters", label: "VU meters", classic: true },
    { key: "visualizer", label: "Visualizer", classic: true },
    { key: "halo", label: "Halo around the cover", classic: true },
    { key: "mascot", label: "Mascot" },
    { key: "album_colors", label: "Colors from the album art" },
  ];
  // the other layouts draw their own panels: only what applies to them
  const toggles = $derived(layoutOf(room?.layout) === "classic" ? all : all.filter((t) => !t.classic));

  function set(patch: Partial<RoomSettings>) {
    if (room) app.saveSettings({ room: { ...room, ...patch } });
  }
</script>

<button class="scrim" aria-label="Close" onclick={onclose}></button>
<div class="menu" role="dialog" aria-label="Customize the room" transition:pop>
  <div class="title">Customize the room</div>
  {#if room}
    <label class="row">
      <span>Layout</span>
      <select value={layoutOf(room.layout)} onchange={(e) => set({ layout: e.currentTarget.value as PlayerLayout })}>
        {#each LAYOUTS as l}<option value={l.id}>{l.label}</option>{/each}
      </select>
    </label>
    <label class="row">
      <span>Use the layout everywhere</span>
      <input type="checkbox" checked={room.layout_everywhere ?? true} onchange={(e) => set({ layout_everywhere: e.currentTarget.checked })} />
    </label>
    {#if layoutOf(room.layout) === "immersive"}
      <label class="row">
        <span>Shade under the title</span>
        <select value={room.immersive_shade ?? "auto"} onchange={(e) => set({ immersive_shade: e.currentTarget.value as "auto" })}>
          <option value="auto">Auto</option>
          <option value="strong">Strong</option>
          <option value="soft">Soft</option>
          <option value="off">Off</option>
        </select>
      </label>
    {/if}
    {#each toggles as t}
      <label class="row">
        <span>{t.label}</span>
        <input type="checkbox" checked={room[t.key] as boolean} onchange={(e) => set({ [t.key]: e.currentTarget.checked })} />
      </label>
    {/each}
    {#if room.mascot}
      <MascotOptions />
    {/if}
  {/if}
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 20;
    cursor: default;
  }
  .menu {
    position: absolute;
    right: 0;
    top: calc(100% + 8px);
    z-index: 21;
    width: 320px;
    max-height: min(640px, calc(100vh - 160px));
    overflow-y: auto;
    padding: 10px;
    border-radius: min(16px, var(--r-max));
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
  }
  .title {
    font-weight: 700;
    padding: 4px 6px 8px;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 7px 6px;
    border-radius: min(8px, var(--r-max));
    cursor: pointer;
  }
  .row:hover {
    background: var(--surface-2);
  }
  .row select {
    padding: 4px 6px;
    border-radius: min(8px, var(--r-max));
    background: var(--surface-2);
    color: var(--text);
    border: 1px solid var(--border);
    font: inherit;
  }
  .row input {
    width: 16px;
    height: 16px;
    accent-color: var(--room-accent);
  }
</style>
