// The Homifito DJ setting (Settings → DJ, and the DJ screen's top bar). It is part of the DJ
// settings (`settings.dj.homifito`, homefi_core::dj::HomifitoDj), so it syncs to Android and
// travels in DJ setups and `.homefidj` files. A value an older version kept in localStorage is
// moved there once.
import { HOMIFITO_MODES, type HomifitoMode, type HomifitoPlace } from "./homifito-dj";
import { dj } from "./dj.svelte";
import { app } from "./state.svelte";

const LEGACY_KEY = "homefi.dj.homifito";

type Patch = Partial<{ mode: HomifitoMode; place: HomifitoPlace; look: boolean }>;

/** The localStorage value of the previous version, as a valid patch (or null when there is none). */
function readLegacy(): Patch | null {
  try {
    const raw = JSON.parse(localStorage.getItem(LEGACY_KEY) ?? "null") as Record<string, unknown> | null;
    if (!raw || typeof raw !== "object") return null;
    const patch: Patch = {};
    if (HOMIFITO_MODES.includes(raw.mode as HomifitoMode)) patch.mode = raw.mode as HomifitoMode;
    if (raw.place === "top" || raw.place === "bottom") patch.place = raw.place;
    if (typeof raw.look === "boolean") patch.look = raw.look;
    return patch;
  } catch {
    return null;
  }
}

class HomifitoDjStore {
  /** The size the top-bar button turns back on (this session only). */
  private lastOn: Exclude<HomifitoMode, "off"> = "booth";

  constructor() {
    // Once the settings are loaded: take the old localStorage value over, then forget it.
    $effect.root(() => {
      $effect(() => {
        if (!app.settings) return;
        const legacy = readLegacy();
        if (!legacy) return;
        try {
          localStorage.removeItem(LEGACY_KEY);
        } catch {}
        if (Object.keys(legacy).length) void this.set(legacy);
      });
    });
  }

  get mode(): HomifitoMode {
    return dj.settings.homifito.mode;
  }
  get place(): HomifitoPlace {
    return dj.settings.homifito.place;
  }
  get look(): boolean {
    return dj.settings.homifito.look;
  }
  get on(): boolean {
    return this.mode !== "off";
  }

  private async set(patch: Patch) {
    if (patch.mode && patch.mode !== "off") this.lastOn = patch.mode;
    await dj.saveSettings({ homifito: { ...dj.settings.homifito, ...patch } });
  }

  setMode(mode: HomifitoMode) {
    void this.set({ mode });
  }
  /** The top bar's one click: off, or back to the size he had. */
  toggle() {
    this.setMode(this.on ? "off" : this.lastOn);
  }
  setPlace(place: HomifitoPlace) {
    void this.set({ place });
  }
  setLook(look: boolean) {
    void this.set({ look });
  }
}

export const homifitoDj = new HomifitoDjStore();
