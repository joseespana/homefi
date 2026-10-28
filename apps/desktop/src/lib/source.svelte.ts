// What the player views (room, layouts, deck, mini player, mascot) draw and
// control: the app's own player, or someone else's room while this computer is
// a guest there. Views call `useSource()`; the guest view provides the room.
import { getContext, setContext } from "svelte";
import { api, type PlayerState, type Repeat, type Track } from "./api";
import { share } from "./share.svelte";
import { app } from "./state.svelte";

export interface PlayerSource {
  readonly player: PlayerState | null;
  readonly current: Track | null;
  /** Someone else's room (shared control). */
  readonly remote: boolean;
  /** "From <session>" while remote. */
  readonly label: string;
  /** What the controls may do (the hub's rules while remote). */
  readonly can: { toggle: boolean; next: boolean; previous: boolean; seek: boolean; jump: boolean; shuffle: boolean; repeat: boolean };
  trackAt(path: string): Track | undefined;
  toggle(): void;
  next(): void;
  previous(): void;
  seek(ms: number): void;
  jump(index: number): void;
  shuffle(on: boolean): void;
  repeat(mode: Repeat): void;
}

const ALL = { toggle: true, next: true, previous: true, seek: true, jump: true, shuffle: true, repeat: true };

export const localSource: PlayerSource = {
  get player() {
    return app.player;
  },
  get current() {
    return app.current;
  },
  remote: false,
  label: "",
  can: ALL,
  trackAt: (path) => app.trackAt(path),
  toggle: () => api.toggle(),
  next: () => api.next(),
  previous: () => api.previous(),
  seek: (ms) => api.seek(ms),
  jump: (index) => api.jump(index),
  shuffle: (on) => api.shuffle(on),
  repeat: (mode) => api.repeat(mode),
};

/** Someone else's room: no seeking or going back (the protocol has neither); the rest as the hub allows. */
export const roomSource: PlayerSource = {
  get player() {
    return share.roomPlayer;
  },
  get current() {
    return share.roomPlayer?.track ?? null;
  },
  remote: true,
  get label() {
    return `From ${share.guest.session_name}`;
  },
  get can() {
    return { toggle: share.canToggle, next: share.canSkip, previous: false, seek: false, jump: share.canPlayNow, shuffle: false, repeat: false };
  },
  trackAt: (path) => share.roomTrackAt(path),
  toggle: () => share.toggle(),
  next: () => share.skip(),
  previous: () => {},
  seek: () => {},
  // index 0 is the song playing now
  jump: (index) => {
    const item = share.roomItems[index];
    if (index > 0 && item) share.playNow(item);
  },
  shuffle: () => {},
  repeat: () => {},
};

/** The room while in one (unless the user's own music plays), else the app's player. */
function active(): PlayerSource {
  return share.inRoom && app.player?.status !== "playing" ? roomSource : localSource;
}

export const shownSource: PlayerSource = {
  get player() {
    return active().player;
  },
  get current() {
    return active().current;
  },
  get remote() {
    return active().remote;
  },
  get label() {
    return active().label;
  },
  get can() {
    return active().can;
  },
  trackAt: (path) => active().trackAt(path),
  toggle: () => active().toggle(),
  next: () => active().next(),
  previous: () => active().previous(),
  seek: (ms) => active().seek(ms),
  jump: (index) => active().jump(index),
  shuffle: (on) => active().shuffle(on),
  repeat: (mode) => active().repeat(mode),
};

const KEY = Symbol("player-source");

/** Makes the views inside this component draw `source`. */
export function provideSource(source: PlayerSource) {
  setContext(KEY, source);
}

/** The source the surrounding view provides, or what the app shows as playing. */
export function useSource(): PlayerSource {
  return getContext<PlayerSource | undefined>(KEY) ?? shownSource;
}
