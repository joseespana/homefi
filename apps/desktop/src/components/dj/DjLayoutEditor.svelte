<script lang="ts">
  // "Edit layout": arrange the DJ screen. Sections (deck A, mixer, deck B, sampler, crate) are put in
  // order by dragging (pointer, mouse first) or with the arrow buttons, and shown or hidden; each of the
  // 8 sampler pads gets the sound the rider picks. The screen below changes as it is edited, and every
  // change is saved at once (Settings → DJ → the layout, the same JSON as Android's).
  import { tick } from "svelte";
  import { dj } from "../../lib/dj.svelte";
  import { DJ_SECTION_INFO, SFX, SFX_KINDS } from "../../lib/dj-api";
  import { isDefaultLayout } from "../../lib/dj-layout";
  import type { HomifitoMode, HomifitoPlace } from "../../lib/homifito-dj";
  import { homifitoDj } from "../../lib/homifito-dj.svelte";
  import Icon from "../Icon.svelte";

  const sections = $derived(dj.layout.sections);
  const pads = $derived(dj.layout.pads);

  // ------------------------------------------------------------ order, by drag
  // The chips may wrap onto two lines, so the drop place is the chip nearest to the pointer (in both
  // directions); the one lifted follows the pointer and the place it will take is outlined.
  let list = $state<HTMLElement>();
  let drag = $state<{ from: number; to: number; dx: number; dy: number } | null>(null);
  let start = { x: 0, y: 0 };
  const DRAG_THRESHOLD_PX = 4;

  function down(e: PointerEvent, from: number) {
    if (e.button !== 0) return;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    start = { x: e.clientX, y: e.clientY };
    drag = { from, to: from, dx: 0, dy: 0 };
  }
  function move(e: PointerEvent) {
    if (!drag || !list) return;
    const dx = e.clientX - start.x;
    const dy = e.clientY - start.y;
    let to = drag.from;
    let best = Infinity;
    [...list.children].forEach((chip, i) => {
      if (i === drag!.from) return;
      const r = chip.getBoundingClientRect();
      const distance = Math.hypot(e.clientX - (r.left + r.width / 2), e.clientY - (r.top + r.height / 2));
      if (distance < best) {
        best = distance;
        to = i;
      }
    });
    drag = { ...drag, dx, dy, to };
  }
  async function up() {
    const d = drag;
    drag = null;
    if (d && Math.hypot(d.dx, d.dy) > DRAG_THRESHOLD_PX && d.to !== d.from) await dj.moveSection(d.from, d.to);
  }
  const lifted = (i: number) => drag?.from === i && Math.hypot(drag.dx, drag.dy) > DRAG_THRESHOLD_PX;

  // ------------------------------------------------------------ order, by arrows
  async function step(from: number, by: -1 | 1) {
    const kind = sections[from].kind;
    await dj.moveSection(from, from + by);
    await tick();
    list?.querySelector<HTMLElement>(`[data-kind="${kind}"] [data-dir="${by}"]:not(:disabled)`)?.focus();
  }

  // ------------------------------------------------------------ Homifito (settings.dj.homifito, see lib/homifito-dj.svelte.ts)
  const MODES: { id: HomifitoMode; label: string }[] = [
    { id: "off", label: "Off" },
    { id: "booth", label: "Booth" },
    { id: "big", label: "Big stage" },
  ];
  const PLACES: { id: HomifitoPlace; label: string }[] = [
    { id: "top", label: "Above the decks" },
    { id: "bottom", label: "Below" },
  ];

  // ------------------------------------------------------------ pads
  let picking = $state<number | null>(null);
  const keyOf = (n: number) => (dj.settings.keys_enabled ? dj.keyFor(`sampler.${n + 1}`) : null);
  async function pick(sound: (typeof SFX_KINDS)[number]) {
    const n = picking;
    picking = null;
    if (n !== null) await dj.setPadSound(n, sound);
    await tick();
    if (n !== null) document.querySelector<HTMLElement>(`[data-pad="${n}"]`)?.focus();
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && picking !== null && (e.stopPropagation(), (picking = null))} />

<section class="editor" aria-label="Edit layout">
  <header>
    <div>
      <h2>Arrange the screen</h2>
      <p>Drag a section or use its arrows, hide what you don't use, and pick the sound of each pad. Changes are saved as you make them.</p>
    </div>
    <span class="sp"></span>
    <button type="button" class="btn tonal" onclick={() => (dj.sheetOpen = true)}><Icon name="key" size={16} />Keyboard shortcuts</button>
    <button type="button" class="btn tonal" disabled={isDefaultLayout(dj.layout)} onclick={() => dj.resetLayout()}><Icon name="undo" size={16} />Reset to default</button>
    <button type="button" class="btn primary" onclick={() => (dj.editing = false)}>Done</button>
  </header>

  <div class="cols">
    <div class="block">
      <h3 id="dj-ed-sections">Sections, left to right</h3>
      <ol class="chips" bind:this={list} aria-labelledby="dj-ed-sections">
        {#each sections as s, i (s.kind)}
          {@const info = DJ_SECTION_INFO[s.kind]}
          <li class="chip" class:off={!s.visible} class:dragging={lifted(i)} class:drop={!!drag && lifted(drag.from) && drag.to === i && drag.from !== i} data-kind={s.kind} style:transform={drag?.from === i ? `translate(${drag.dx}px, ${drag.dy}px)` : undefined}>
            <span class="grip" role="presentation" title="Drag to move" onpointerdown={(e) => down(e, i)} onpointermove={move} onpointerup={up} onpointercancel={() => (drag = null)}><Icon name="grip" size={16} /></span>
            <span class="nm" title={info.hint}>{info.name}</span>
            <button type="button" class="icon-btn" data-dir="-1" disabled={i === 0} aria-label="Move {info.name} earlier" title="Move earlier" onclick={() => step(i, -1)}><Icon name="back" size={16} /></button>
            <button type="button" class="icon-btn" data-dir="1" disabled={i === sections.length - 1} aria-label="Move {info.name} later" title="Move later" onclick={() => step(i, 1)}><Icon name="chevron" size={16} /></button>
            <button type="button" class="icon-btn eye" aria-pressed={s.visible} aria-label={s.visible ? `Hide ${info.name}` : `Show ${info.name}`} title={s.visible ? "Hide it" : "Show it"} onclick={() => dj.setSectionVisible(s.kind, !s.visible)}><Icon name={s.visible ? "eye" : "eyeOff"} size={16} /></button>
          </li>
        {/each}
      </ol>
      <h3 id="dj-ed-homifito" class="sub">Homifito at his booth</h3>
      <div class="sounds" role="group" aria-labelledby="dj-ed-homifito">
        {#each MODES as m (m.id)}<button type="button" class="snd" class:on={homifitoDj.mode === m.id} aria-pressed={homifitoDj.mode === m.id} onclick={() => homifitoDj.setMode(m.id)}>{m.label}</button>{/each}
        {#if homifitoDj.on}
          <span class="lbl">Standing</span>
          {#each PLACES as p (p.id)}<button type="button" class="snd" class:on={homifitoDj.place === p.id} aria-pressed={homifitoDj.place === p.id} onclick={() => homifitoDj.setPlace(p.id)}>{p.label}</button>{/each}
        {/if}
        <button type="button" class="snd" class:on={homifitoDj.look} aria-pressed={homifitoDj.look} title="The whole board in Homifito's colours" onclick={() => homifitoDj.setLook(!homifitoDj.look)}>Homifito colours</button>
      </div>
    </div>

    <div class="block">
      <h3 id="dj-ed-pads">Sampler pads</h3>
      <div class="pads" role="group" aria-labelledby="dj-ed-pads">
        {#each pads as sound, n}
          {@const key = keyOf(n)}
          <button type="button" class="pad" class:open={picking === n} data-pad={n} aria-haspopup="true" aria-expanded={picking === n} aria-label="Pad {n + 1}: {SFX[sound].name}. Change the sound" title="Pad {n + 1}{key ? ` [${key}]` : ''}: choose its sound" onclick={() => (picking = picking === n ? null : n)}>
            <span class="num">{n + 1}</span>
            <Icon name={SFX[sound].icon} size={18} />
            <span class="pn">{SFX[sound].name}</span>
          </button>
        {/each}
      </div>
      {#if picking !== null}
        {@const at = picking}
        <div class="sounds" role="group" aria-label="Sound for pad {at + 1}">
          <span class="lbl">Pad {at + 1} plays</span>
          {#each SFX_KINDS as kind (kind)}
            <button type="button" class="snd" class:on={pads[at] === kind} aria-pressed={pads[at] === kind} onclick={() => pick(kind)}><Icon name={SFX[kind].icon} size={16} />{SFX[kind].name}</button>
          {/each}
        </div>
      {/if}
    </div>
  </div>
</section>

<style>
  .editor {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 12px 14px;
    background: var(--surface);
    border: 1px solid color-mix(in srgb, var(--primary) 50%, var(--border));
    border-radius: var(--r-lg);
  }
  header {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
  }
  h2 {
    margin: 0;
    font-size: 15px;
  }
  header p {
    margin: 2px 0 0;
    color: var(--muted);
    font-size: 13px;
  }
  .sp {
    flex: 1;
  }
  .btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 40px;
    padding: 0 14px;
  }
  .cols {
    display: grid;
    grid-template-columns: minmax(0, 1.15fr) minmax(0, 1fr);
    gap: 18px;
  }
  @media (max-width: 1179px) {
    .cols {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  h3.sub {
    margin-top: 14px;
  }
  h3 {
    margin: 0 0 6px;
    font-size: 11px;
    color: var(--faint);
    text-transform: uppercase;
    letter-spacing: 0.1em;
  }
  .chips {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .chip {
    display: flex;
    align-items: center;
    gap: 2px;
    min-height: 44px;
    padding: 0 4px 0 2px;
    border-radius: var(--r-pill);
    border: 1px solid var(--border);
    background: var(--surface-2);
    touch-action: none;
  }
  .chip.off {
    border-style: dashed;
    opacity: 0.6;
  }
  .chip.drop {
    outline: 2px dashed var(--primary);
    outline-offset: 2px;
  }
  .chip.dragging {
    z-index: 2;
    position: relative;
    background: var(--surface-3);
    box-shadow: var(--shadow);
    border-color: var(--primary);
  }
  .grip {
    display: grid;
    place-items: center;
    width: 28px;
    height: 36px;
    color: var(--muted);
    cursor: grab;
    touch-action: none;
  }
  .chip.dragging .grip {
    cursor: grabbing;
  }
  .nm {
    padding: 0 6px;
    font-weight: 600;
    white-space: nowrap;
  }
  .icon-btn {
    width: 32px;
    height: 36px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    color: var(--muted);
  }
  .icon-btn:hover:not(:disabled) {
    background: var(--surface-3);
    color: var(--text);
  }
  .icon-btn:disabled {
    opacity: 0.3;
  }
  .pads {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 8px;
  }
  .pad {
    position: relative;
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 44px;
    padding: 0 10px 0 24px;
    border-radius: var(--r-md);
    border: 1px solid var(--border);
    background: linear-gradient(var(--hw-top), var(--hw-bottom));
    font-size: 13px;
    font-weight: 600;
    text-align: left;
  }
  .pad:hover,
  .pad.open {
    border-color: var(--primary);
  }
  .num {
    position: absolute;
    left: 8px;
    top: 4px;
    font: 500 10px var(--font-mono);
    color: var(--faint);
  }
  .pn {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sounds {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-top: 8px;
  }
  .lbl {
    font-size: 12px;
    color: var(--muted);
    margin-right: 4px;
  }
  .snd {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 36px;
    padding: 0 12px;
    border-radius: var(--r-pill);
    border: 1px solid var(--border);
    background: var(--surface-2);
    font-size: 13px;
  }
  .snd:hover {
    background: var(--surface-3);
  }
  .snd.on {
    background: var(--primary);
    color: var(--on-primary);
    border-color: transparent;
  }
</style>
