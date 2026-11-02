// The core draws a sound's curve (output profile + sound, taking its caps into
// account): this asks for it a moment after the last change, newest answer wins.
import { api, type TastePreview, type TasteRule } from "./api";

/** Wait this long after a change before asking, so a drag isn't one request per pixel. */
const PREVIEW_DELAY_MS = 50;

/** Call during component setup. `rule()` / `output()` are read reactively. */
export function usePreview(rule: () => TasteRule | null, output: () => string | null) {
  let value = $state.raw<TastePreview | null>(null);
  let seq = 0;
  $effect(() => {
    const r = rule();
    const out = output();
    if (!r) {
      value = null;
      return;
    }
    // a plain copy: the preview must not read the (proxied) draft again after the await
    const body: TasteRule = JSON.parse(JSON.stringify(r));
    const my = ++seq;
    const t = setTimeout(() => {
      api
        .tastePreview(body, out)
        .then((p) => my === seq && (value = p))
        .catch(() => my === seq && (value = null));
    }, PREVIEW_DELAY_MS);
    return () => clearTimeout(t);
  });
  return {
    get value() {
      return value;
    },
  };
}
