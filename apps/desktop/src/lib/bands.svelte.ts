// The live spectrum, resampled to `n` bands and refreshed every frame, for the
// layouts that draw it with plain elements (faders, LEDs). Call during component init.
import { visual } from "./visual.svelte";

export function liveBands(n: number): { values: number[] } {
  const live = $state({ values: new Array<number>(n).fill(0) });
  $effect(() => {
    let raf = 0;
    const tick = () => {
      const s = visual.spectrum;
      live.values = Array.from({ length: n }, (_, i) => {
        const a = Math.floor((i * s.length) / n);
        const b = Math.min(s.length - 1, a + 1);
        return Math.min(1, Math.max(0, ((s[a] ?? 0) + (s[b] ?? 0)) / 2));
      });
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  });
  return live;
}
