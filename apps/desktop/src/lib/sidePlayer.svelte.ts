// The song playing in a column on the right while you browse (Spotify-like), on windows wide
// enough for it. Its button opens the full room; hiding it lasts until it's asked for again.
import { app } from "./state.svelte";

/** From this window width (px) the column fits beside the content (the queue's own threshold). */
const MIN_WINDOW = 1180;

class SidePlayer {
  #width = $state(typeof window === "undefined" ? 0 : window.innerWidth);
  /** Hidden with its button; the player buttons bring it back. Not saved: a new launch shows it. */
  hidden = $state(false);

  constructor() {
    if (typeof window !== "undefined") window.addEventListener("resize", () => (this.#width = window.innerWidth));
  }

  /** On in Settings and the window is wide enough: the toggle button makes sense. */
  get available(): boolean {
    return app.settings?.room.side_player !== false && this.#width >= MIN_WINDOW;
  }

  /** Not over the room (it is the room, bigger), DJ mode or a guest's room, and only with a song. */
  get wanted(): boolean {
    const v = app.view.name;
    return this.available && !!app.current && v !== "room" && v !== "dj" && v !== "guest";
  }

  toggle() {
    this.hidden = !this.hidden;
  }

  maximize() {
    app.view = { name: "room" };
  }
}

export const sidePlayer = new SidePlayer();
