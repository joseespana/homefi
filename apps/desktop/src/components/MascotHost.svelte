<script lang="ts">
  // The mascot wired to the player and the room settings. In the rack it sits
  // in the layout; floating, it stays where it was dragged. Click: a famous
  // dance (or a wave while paused). Right-click: next style. Drag: move it.
  // Wheel: resize (floating). Double-click: back to the room.
  import type { Specials, Temper } from "../lib/dance";
  import { dock, grab, nextStyle, place, resize, roomMood } from "../lib/mascotPlace.svelte";
  import { useSource } from "../lib/source.svelte";
  import { app } from "../lib/state.svelte";
  import { visual } from "../lib/visual.svelte";
  import Mascot from "./Mascot.svelte";

  // `singing`: karaoke, a microphone to its mouth while the song plays
  let { floating = false, size: fit = 180, asleep = false, singing = false }: { floating?: boolean; size?: number; asleep?: boolean; singing?: boolean } = $props();

  const room = $derived(app.settings?.room);
  const src = useSource();
  const playing = $derived(!asleep && src.player?.status === "playing");
  const size = $derived(room?.mascot_size || fit);
  let el: HTMLDivElement;
  let mascot: Mascot;

  // the style it just switched to, shown for a moment
  let styleTag = $state<string | null>(null);
  let tagTimer = 0;
  function restyle(e: MouseEvent) {
    if (!room?.mascot_right_click) return;
    e.preventDefault();
    styleTag = nextStyle();
    clearTimeout(tagTimer);
    tagTimer = window.setTimeout(() => (styleTag = null), 1400);
  }
  const hint = $derived(
    ["Click: dance", room?.mascot_right_click && "Right-click: next style", "Drag: move", floating && "Scroll: resize", floating && "Double-click: back to the room"]
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
  onpointerdown={(e) => grab(e, el, () => mascot.poke())}
  oncontextmenu={restyle}
  ondblclick={() => floating && dock()}
  onwheel={(e) => floating && resize(e, size)}
  onkeydown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), mascot.poke())}
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
</div>

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
  .dragging {
    cursor: grabbing;
  }
  .dragging :global(svg) {
    filter: drop-shadow(0 26px 30px rgb(0 0 0 / 0.4));
  }
</style>
