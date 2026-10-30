<script lang="ts">
  // The mascot wired to the player and the room settings. In the rack it sits
  // in the layout; floating, it stays where it was dragged. Click: a famous
  // dance (or a wave while paused). Right-click: its actions (or the next
  // style, see `mascot_hold`). Drag: move it.
  // Wheel: resize (floating). Double-click: back to the room.
  import type { Specials, Temper } from "../lib/dance";
  import { dock, grab, nextStyle, place, resize, roomMood } from "../lib/mascotPlace.svelte";
  import { useSource } from "../lib/source.svelte";
  import { app } from "../lib/state.svelte";
  import { visual } from "../lib/visual.svelte";
  import Icon from "./Icon.svelte";
  import Mascot from "./Mascot.svelte";
  import MascotMenu from "./MascotMenu.svelte";

  // `singing`: karaoke, a microphone to its mouth while the song plays
  let { floating = false, size: fit = 180, asleep = false, singing = false }: { floating?: boolean; size?: number; asleep?: boolean; singing?: boolean } = $props();

  const room = $derived(app.settings?.room);
  const src = useSource();
  const playing = $derived(!asleep && src.player?.status === "playing");
  const size = $derived(room?.mascot_size || fit);
  let el: HTMLDivElement;
  let mascot = $state<Mascot>();

  // the style it just switched to, shown for a moment
  let styleTag = $state<string | null>(null);
  let tagTimer = 0;
  function restyle() {
    styleTag = nextStyle();
    clearTimeout(tagTimer);
    tagTimer = window.setTimeout(() => (styleTag = null), 1400);
  }

  // where the right-click opened the actions menu
  let menu = $state<{ x: number; y: number } | null>(null);
  const hold = $derived(room?.mascot_hold ?? "menu");
  function rightClick(e: MouseEvent) {
    if (hold === "off") return;
    e.preventDefault();
    if (hold === "menu") menu = { x: e.clientX, y: e.clientY };
    else restyle();
  }
  const hint = $derived(
    [
      "Click: dance",
      hold === "menu" && "Right-click: dances and more",
      hold === "style" && "Right-click: next style",
      "Drag: move",
      floating && "Scroll: resize",
      floating && "Double-click: back to the room",
    ]
      .filter(Boolean)
      .join(" · "),
  );

  // the stored spot, unless a drag is moving it right now
  $effect(() => {
    if (room && !place.dragging) {
      place.x = room.mascot_x;
      place.y = room.mascot_y;
    }
  });
</script>

<div
  bind:this={el}
  class="host"
  class:floating
  class:dragging={place.dragging}
  style:left={floating ? `${place.x * 100}%` : undefined}
  style:top={floating ? `${place.y * 100}%` : undefined}
  title={hint}
  role="button"
  tabindex="0"
  aria-label="HomeFi mascot"
  onpointerdown={(e) => grab(e, el, () => mascot?.poke())}
  oncontextmenu={rightClick}
  ondblclick={() => floating && dock()}
  onwheel={(e) => floating && resize(e, size)}
  onkeydown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), mascot?.poke())}
>
  <Mascot
    bind:this={mascot}
    {playing}
    {singing}
    {size}
    mood={roomMood()}
    energy={visual.energy}
    kick={visual.kick}
    bpm={visual.bpm}
    dance={room?.mascot_dance ?? "auto"}
    specials={(room?.mascot_specials ?? "sometimes") as Specials}
    limbs={room?.mascot_limbs ?? "white"}
    turns={room?.mascot_turns ?? "all"}
    temper={(room?.mascot_temper ?? "normal") as Temper}
  />
  {#if styleTag}<span class="tag">{styleTag}</span>{/if}
  {#if room?.mascot_dance_button ?? true}
    <button
      type="button"
      class="dances"
      class:open={menu}
      title="Dances"
      aria-label="Dances"
      aria-haspopup="menu"
      onpointerdown={(e) => e.stopPropagation()}
      onclick={(e) => {
        const r = e.currentTarget.getBoundingClientRect();
        menu = { x: r.right, y: r.top };
      }}><Icon name="music" size={16} /></button>
  {/if}
</div>

{#if menu}
  <MascotMenu
    x={menu.x}
    y={menu.y}
    {playing}
    dancing={mascot?.dancing() ?? null}
    {floating}
    onclose={() => ((menu = null), el.focus())}
    ondance={(id) => mascot?.danceNow(id)}
    onwave={() => mascot?.wave()}
    onstyle={restyle}
    ondock={dock}
  />
{/if}

<style>
  .host {
    position: relative;
    flex: none;
    touch-action: none;
    user-select: none;
    cursor: grab;
    outline: none;
  }
  .host:focus-visible {
    outline: 2px solid var(--room-accent);
    outline-offset: 4px;
    border-radius: 16px;
  }
  .floating {
    position: fixed;
    z-index: 15;
    transform: translate(-50%, -50%);
  }
  .tag {
    position: absolute;
    left: 50%;
    top: -6px;
    transform: translateX(-50%);
    padding: 3px 10px;
    border-radius: 999px;
    background: var(--room-accent, #34d399);
    color: #0b1e3a;
    font-size: 12px;
    font-weight: 700;
    white-space: nowrap;
    pointer-events: none;
    animation: pop 1.4s ease-out forwards;
  }
  @keyframes pop {
    0% {
      opacity: 0;
      transform: translate(-50%, 8px) scale(0.8);
    }
    15%,
    75% {
      opacity: 1;
      transform: translate(-50%, 0) scale(1);
    }
    100% {
      opacity: 0;
      transform: translate(-50%, -10px);
    }
  }
  /* the dances button sits on the mascot's corner, quiet until the pointer comes near */
  .dances {
    position: absolute;
    right: 0;
    bottom: 6%;
    display: grid;
    place-content: center;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
    color: var(--room-accent, var(--primary));
    cursor: pointer;
    opacity: 0.55;
    transition: opacity 0.15s, transform 0.15s;
  }
  .host:hover .dances,
  .dances:focus-visible,
  .dances.open {
    opacity: 1;
  }
  .dances:hover {
    transform: scale(1.08);
  }
  .dragging .dances {
    opacity: 0;
  }
  .dragging {
    cursor: grabbing;
  }
  .dragging :global(svg) {
    filter: drop-shadow(0 26px 30px rgb(0 0 0 / 0.4));
  }
</style>
