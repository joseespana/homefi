// The song's position, smooth between the player's updates (they come a few
// times a second; karaoke fills need a value every frame). Call it while a
// component initialises: it owns two effects.
import type { PlayerState } from "./api";

/** A report this far behind what we already show is ignored (it would make the fill jump back). */
const BEHIND_MS = 400;

export function useSongClock(player: () => PlayerState | null): { readonly now: number } {
  let base = { pos: 0, at: 0 };
  let now = $state(0);
  const playing = $derived(player()?.status === "playing");
  $effect(() => {
    const reported = player()?.position_ms ?? 0;
    const t = performance.now();
    const predicted = base.pos + (t - base.at);
    if (playing && reported <= predicted && predicted - reported < BEHIND_MS) return;
    base = { pos: reported, at: t };
    now = reported;
  });
  $effect(() => {
    if (!playing) return;
    let raf = 0;
    const step = () => {
      now = base.pos + (performance.now() - base.at);
      raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  });
  return {
    get now() {
      return now;
    },
  };
}
