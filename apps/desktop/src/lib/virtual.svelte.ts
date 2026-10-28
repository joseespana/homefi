// Long lists keep only the rows near the screen in the DOM (a 2,000-track
// playlist is ~30 rows, not 2,000 components). Rows have one height, measured
// from the first one drawn; the list scrolls with the page (its scrolling
// ancestor), and spacers stand in for the rows above and below.

import { untrack } from "svelte";

/** Rows drawn beyond the visible ones, above and below. */
const OVERSCAN = 8;

export class Windowed {
  /** First row drawn. */
  start = $state(0);
  /** One past the last row drawn. */
  end = $state(40);
  /** Height of one row (px), measured once rows exist. */
  row = $state(60);
  /** The user scrolled: rows that appear now don't play their entrance. */
  scrolled = $state(false);
  private count = 0;

  constructor(private fallback = 60) {
    this.row = fallback;
  }

  /** Space above the first drawn row. */
  get before(): number {
    return this.start * this.row;
  }

  /** Space below the last drawn row. */
  get after(): number {
    return Math.max(0, this.count - this.end) * this.row;
  }

  /** Attach to the list element: `{@attach win.attach(() => items.length)}`. */
  attach(count: () => number) {
    return (node: HTMLElement) => {
      const scroller = scrollParent(node);
      let frame = 0;
      const update = () => {
        frame = 0;
        this.count = count();
        const first = node.querySelector<HTMLElement>(":scope > [data-row]");
        if (first && first.offsetHeight > 0) this.row = first.offsetHeight;
        const view = scroller ? scroller.getBoundingClientRect() : { top: 0, bottom: window.innerHeight };
        const top = node.getBoundingClientRect().top;
        const from = Math.floor((view.top - top) / this.row) - OVERSCAN;
        const to = Math.ceil((view.bottom - top) / this.row) + OVERSCAN;
        const start = clamp(from, 0, this.count);
        const end = clamp(Math.max(to, start + 1), 0, this.count);
        if (start !== this.start) this.start = start;
        if (end !== this.end) this.end = end;
      };
      const schedule = () => (frame ||= requestAnimationFrame(update));
      const onScroll = () => {
        this.scrolled = true;
        schedule();
      };
      const target: HTMLElement | Window = scroller ?? window;
      target.addEventListener("scroll", onScroll, { passive: true });
      const ro = new ResizeObserver(schedule);
      ro.observe(node);
      if (scroller) ro.observe(scroller);
      // the list grew or shrank (pages arriving, a filter): count() is read again
      $effect(() => {
        count();
        schedule();
      });
      untrack(update);
      return () => {
        target.removeEventListener("scroll", onScroll);
        ro.disconnect();
        if (frame) cancelAnimationFrame(frame);
      };
    };
  }

  /** Resets to the top (a new list in the same view). */
  reset() {
    this.start = 0;
    this.end = 40;
    this.scrolled = false;
  }
}

function clamp(n: number, lo: number, hi: number): number {
  return Math.min(hi, Math.max(lo, n));
}

function scrollParent(node: HTMLElement): HTMLElement | null {
  for (let n = node.parentElement; n; n = n.parentElement) {
    const y = getComputedStyle(n).overflowY;
    if (y === "auto" || y === "scroll") return n;
  }
  return null;
}

/**
 * Grows a count up to `total` a chunk per frame, so a grid of hundreds of
 * covers appears without one long task (the first chunk right away).
 */
export class Progressive {
  shown = $state(0);
  private frame = 0;

  constructor(
    private first = 48,
    private step = 48,
  ) {}

  /** Call from an effect with the full length; returns nothing, read `shown`. */
  track(total: number) {
    untrack(() => {
      if (this.shown > total || this.shown === 0) this.shown = Math.min(total, this.first);
    });
    const grow = () => {
      this.frame = 0;
      if (this.shown >= total) return;
      this.shown = Math.min(total, this.shown + this.step);
      this.frame = requestAnimationFrame(grow);
    };
    if (this.frame) cancelAnimationFrame(this.frame);
    this.frame = requestAnimationFrame(grow);
    return () => {
      if (this.frame) cancelAnimationFrame(this.frame);
      this.frame = 0;
    };
  }
}
