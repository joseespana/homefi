// A Tidal list that arrives page by page: the first page shows right away,
// `more()` brings the next one (one request at a time) and `all()` the rest
// of it, for whatever needs every item (play all, shuffle, download, queue...).
// The rest streams in: each page joins the list as it arrives (a Tauri
// channel), so rows fill live while the user keeps navigating, and `cancel()`
// stops a load nobody waits for any more (the user went elsewhere).
import { errorText, type Hit, type HitPage } from "./api";

/** Fetches the page at `offset`; with `rest`, everything from there to the end. */
export type PageFetch = (offset: number, rest: boolean) => Promise<HitPage>;
/** Streams every page from `offset` to the end, in order, to `onPage`; `load` names it for `cancel`. */
export type PageStream = (offset: number, onPage: (page: HitPage) => void, load: number) => Promise<void>;
/** Stops the stream `load`. */
export type PageCancel = (load: number) => Promise<void>;

let loads = 0;

export class Paged {
  items = $state.raw<Hit[]>([]);
  total = $state(0);
  /** Where the next page starts; null once everything is here. */
  next = $state<number | null>(0);
  loading = $state(false);
  error = $state<string | null>(null);
  private running: Promise<void> | null = null;
  private seen = new Set<string>();
  /** The stream on its way (0: none); pages of another one are dropped. */
  private load = 0;
  /** `all()` calls waiting: a load they wait for is never cancelled. */
  private waiting = 0;

  /** `unique`: drop items already in the list (a collection can shift between pages). */
  constructor(
    private fetch: PageFetch,
    private unique = false,
    private stream: { pages: PageStream; cancel: PageCancel } | null = null,
  ) {}

  /** A list that already has its first page (or all of it, when `next` is null). */
  static from(
    fetch: PageFetch,
    items: Hit[],
    total: number,
    next: number | null,
    unique = false,
    stream: { pages: PageStream; cancel: PageCancel } | null = null,
  ): Paged {
    const p = new Paged(fetch, unique, stream);
    p.add(items);
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
    this.waiting++;
    try {
      while (this.running) await this.running;
      if (this.next != null) {
        await this.run(true);
        if (this.error) throw this.error;
      }
      return this.items;
    } finally {
      this.waiting--;
    }
  }

  /** The rest of the list, streaming in behind what's already shown (the "load lists as you scroll" setting off). */
  rest(): Promise<void> {
    if (this.running || this.next == null) return this.running ?? Promise.resolve();
    return this.run(true);
  }

  /** Whole list: the first page shows as soon as it comes, the rest streams in after it. */
  async whole(): Promise<void> {
    this.next = 0;
    this.items = [];
    this.seen.clear();
    await this.run(false);
    if (!this.error) await this.rest();
  }

  /** Stops a load still streaming, unless something waits for the whole list. It can go on later. */
  cancel() {
    if (!this.load || this.waiting) return;
    const load = this.load;
    this.load = 0;
    this.stream?.cancel(load).catch(() => {});
  }

  private run(rest: boolean): Promise<void> {
    const offset = this.next ?? 0;
    this.loading = true;
    this.error = null;
    const job = (async () => {
      try {
        if (rest && this.stream) {
          const load = (this.load = ++loads);
          await this.stream.pages(
            offset,
            (page) => {
              if (this.load !== load) return;
              this.add(page.items);
              this.total = page.total;
              this.next = page.next_offset;
            },
            load,
          );
          if (this.load === load) this.load = 0;
        } else {
          const page = await this.fetch(offset, rest);
          this.add(page.items);
          this.total = page.total;
          this.next = page.next_offset;
        }
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
    if (!items.length) return;
    if (!this.unique) {
      this.items = this.items.concat(items);
      return;
    }
    const fresh = items.filter((h) => !this.seen.has(h.id) && this.seen.add(h.id));
    if (fresh.length) this.items = this.items.concat(fresh);
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
