<script lang="ts">
  // Every mascot option, shared by the room's Customize menu and Settings.
  import type { RoomSettings } from "../lib/api";
  import { DANCES } from "../lib/dance";
  import { MOODS } from "../lib/mood";
  import { app } from "../lib/state.svelte";

  const room = $derived(app.settings?.room);

  function set(patch: Partial<RoomSettings>) {
    if (room) app.saveSettings({ room: { ...room, ...patch } });
  }

  const specials = [
    { id: "off", label: "Off" },
    { id: "sometimes", label: "Sometimes" },
    { id: "often", label: "Often" },
  ] as const;
  const turns = [
    { id: "off", label: "Always facing you" },
    { id: "dances", label: "In famous dances" },
    { id: "all", label: "Always" },
  ] as const;
  const tempers = [
    { id: "off", label: "Never", hint: "Clicks only make it dance" },
    { id: "patient", label: "Patient", hint: "10 quick clicks" },
    { id: "normal", label: "Normal", hint: "6 quick clicks" },
    { id: "short", label: "Short-tempered", hint: "4 quick clicks" },
  ] as const;
  const limbs = [
    { id: "white", label: "White" },
    { id: "navy", label: "Navy" },
    { id: "accent", label: "Album color" },
    { id: "off", label: "None" },
  ] as const;
</script>

{#if room}
  <div class="sub">Style</div>
  <div class="chips">
    <button class:on={room.mascot_style === "auto"} onclick={() => set({ mascot_style: "auto" })} title="Follows the genre of each song">Auto</button>
    {#each MOODS as m}
      <button class:on={room.mascot_style === m.id} onclick={() => set({ mascot_style: m.id })} title={m.genres}>{m.label}</button>
    {/each}
  </div>

  <div class="sub">Famous dances</div>
  <div class="chips">
    {#each specials as s}
      <button class:on={room.mascot_specials === s.id} onclick={() => set({ mascot_specials: s.id })}>{s.label}</button>
    {/each}
  </div>

  <div class="sub">Dance</div>
  <div class="chips">
    <button class:on={room.mascot_dance === "auto"} onclick={() => set({ mascot_dance: "auto" })} title="The genre's groove, with famous dances now and then">Auto</button>
    {#each DANCES as d}
      <button class:on={room.mascot_dance === d.id} onclick={() => set({ mascot_dance: d.id })} title={d.hint}>{d.label}</button>
    {/each}
  </div>

  <div class="sub">Turn sideways</div>
  <div class="chips">
    {#each turns as t}
      <button class:on={(room.mascot_turns ?? "all") === t.id} onclick={() => set({ mascot_turns: t.id })}>{t.label}</button>
    {/each}
  </div>

  <div class="sub">Arms and legs</div>
  <div class="chips">
    {#each limbs as l}
      <button class:on={room.mascot_limbs === l.id} onclick={() => set({ mascot_limbs: l.id })}>{l.label}</button>
    {/each}
  </div>

  <div class="sub">Gets angry when clicked a lot</div>
  <div class="chips">
    {#each tempers as t}
      <button class:on={(room.mascot_temper ?? "normal") === t.id} onclick={() => set({ mascot_temper: t.id })} title={t.hint}>{t.label}</button>
    {/each}
  </div>
  <label class="row">
    <span>Right-click switches its style</span>
    <input type="checkbox" checked={room.mascot_right_click ?? true} onchange={(e) => set({ mascot_right_click: e.currentTarget.checked })} />
  </label>

  <div class="sub">Size</div>
  <div class="size">
    <button class="auto" class:on={!room.mascot_size} onclick={() => set({ mascot_size: 0 })}>Auto</button>
    <input
      type="range"
      min="80"
      max="520"
      step="10"
      value={room.mascot_size || 180}
      aria-label="Mascot size"
      onchange={(e) => set({ mascot_size: +e.currentTarget.value })}
    />
    <span class="px">{room.mascot_size ? `${room.mascot_size}px` : ""}</span>
  </div>

  <div class="sub">Place</div>
  <label class="row">
    <span>Floating (drag it anywhere)</span>
    <input type="checkbox" checked={room.mascot_free} onchange={(e) => set({ mascot_free: e.currentTarget.checked })} />
  </label>
  <label class="row" class:off={!room.mascot_free}>
    <span>Show it on every screen</span>
    <input type="checkbox" disabled={!room.mascot_free} checked={room.mascot_everywhere} onchange={(e) => set({ mascot_everywhere: e.currentTarget.checked })} />
  </label>
  {#if room.mascot_free}
    <button class="reset" onclick={() => set({ mascot_free: false, mascot_x: 0.85, mascot_y: 0.75 })}>Put it back in the room</button>
  {/if}
{/if}

<style>
  .sub {
    margin: 10px 6px 6px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--faint);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 0 4px 4px;
  }
  .chips button,
  .auto,
  .reset {
    padding: 5px 11px;
    border-radius: min(999px, var(--r-max));
    background: var(--surface-2);
    border: 1px solid var(--border);
    font-size: 12.5px;
  }
  .on {
    background: color-mix(in srgb, var(--room-accent) 28%, transparent) !important;
    border-color: var(--room-accent) !important;
  }
  .size {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 4px 4px;
  }
  .size input {
    flex: 1;
    accent-color: var(--room-accent);
  }
  .px {
    min-width: 44px;
    font-size: 12px;
    color: var(--muted);
    text-align: right;
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
  .row.off {
    opacity: 0.5;
  }
  .row input {
    width: 16px;
    height: 16px;
    accent-color: var(--room-accent);
  }
  .reset {
    margin: 4px 4px 2px;
  }
</style>
