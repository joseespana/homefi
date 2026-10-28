// A Tidal list that arrives page by page: the first page shows right away,
// `more()` brings the next one (one request at a time) and `all()` the rest
// of it, for whatever needs every item (play all, shuffle, download, queue...).
import { errorText, type Hit, type HitPage } from "./api";

/** Fetches the page at `offset`; with `rest`, everything from there to the end. */
export type PageFetch = (offset: number, rest: boolean) => Promise<HitPage>;

export class Paged {
  items = $state.raw<Hit[]>([]);
  total = $state(0);
  /** Where the next page starts; null once everything is here. */
  next = $state<number | null>(0);
  loading = $state(false);
  error = $state<string | null>(null);
  private running: Promise<void> | null = null;

  /** `unique`: drop items already in the list (a collection can shift between pages). */
  constructor(
    private fetch: PageFetch,
    private unique = false,
  ) {}

  /** A list that already has its first page (or all of it, when `next` is null). */
  static from(fetch: PageFetch, items: Hit[], total: number, next: number | null, unique = false): Paged {
    const p = new Paged(fetch, unique);
    p.items = items;
    p.total = total;
    p.next = next;
    return p;
  }

  get done(): boolean {
    return this.next == null;
  }

  /** Loads the next page; nothing when one is on its way or the list is complete. */
  more(): Promise<void> {
    if (this.running || this.next == null) return this.running ?? Promise.resolve();
    return this.run(false);
  }

  /** Every item: waits for the page on its way, then fetches what's still missing in one go. Throws on failure. */
  async all(): Promise<Hit[]> {
    while (this.running) await this.running;
    if (this.next != null) {
      await this.run(true);
      if (this.error) throw this.error;
    }
    return this.items;
  }

  /** Whole list, loaded before anything shows (the "load lists as you scroll" setting off). */
  async whole(): Promise<void> {
    this.next = 0;
    this.items = [];
    await this.run(true);
  }

  private run(rest: boolean): Promise<void> {
    const offset = this.next ?? 0;
    this.loading = true;
    this.error = null;
    const job = (async () => {
      try {
        const page = await this.fetch(offset, rest);
        this.add(page.items);
        this.total = page.total;
        this.next = page.next_offset;
      } catch (e) {
        this.error = errorText(e);
      } finally {
        this.loading = false;
        this.running = null;
      }
    })();
    this.running = job;
    return job;
  }

  private add(items: Hit[]) {
    if (!this.unique) {
      this.items = [...this.items, ...items];
      return;
    }
    const seen = new Set(this.items.map((h) => h.id));
    this.items = [...this.items, ...items.filter((h) => !seen.has(h.id) && seen.add(h.id))];
  }
}

/** Calls `load` whenever `node` comes near the visible part of its scroll container. */
export function nearEnd(node: HTMLElement, load: () => void) {
  let current = load;
  const io = new IntersectionObserver((entries) => entries.some((e) => e.isIntersecting) && current(), { rootMargin: "600px 0px" });
  io.observe(node);
  return {
    update(next: () => void) {
      current = next;
    },
    destroy() {
      io.disconnect();
    },
  };
}
