// The Settings screen's shared state: the settings being edited, saving them,
// the brief "Saved" confirmation and the error bar. Created once by
// Settings.svelte and handed to every page through the Svelte context.
import { getContext, setContext } from "svelte";
import { api, errorText, type RoomSettings, type Settings, type ShareSettings } from "../../lib/api";
import { app } from "../../lib/state.svelte";

/** How long the "Saved" check stays next to the page title. */
const SAVED_MS = 1200;

export class SettingsModel {
  settings = $state<Settings | null>(null);
  error = $state<string | null>(null);
  /** The "Saved" check is showing. */
  saved = $state(false);

  /** Only the newest save may overwrite the screen with the backend's answer. */
  #latest = 0;
  #savedTimer: ReturnType<typeof setTimeout> | undefined;

  /** Shows the change at once; puts the old values back if the backend refuses it. */
  async save(patch: Partial<Settings>) {
    const before = this.settings;
    if (!before) return;
    const mine = ++this.#latest;
    this.settings = { ...before, ...patch };
    try {
      const stored = await api.setSettings({ ...before, ...patch });
      if (mine === this.#latest) {
        this.settings = stored;
        app.settings = stored;
      }
      this.error = null;
      this.#flashSaved();
    } catch (e) {
      if (mine === this.#latest) this.settings = before;
      this.error = errorText(e);
    }
  }

  saveRoom(patch: Partial<RoomSettings>) {
    if (this.settings) this.save({ room: { ...this.settings.room, ...patch } });
  }

  saveShare(patch: Partial<ShareSettings>) {
    if (this.settings) this.save({ share: { ...this.settings.share, ...patch } });
  }

  dismissError() {
    this.error = null;
  }

  dispose() {
    clearTimeout(this.#savedTimer);
  }

  #flashSaved() {
    this.saved = true;
    clearTimeout(this.#savedTimer);
    this.#savedTimer = setTimeout(() => (this.saved = false), SAVED_MS);
  }
}

const KEY = Symbol("settings-model");

export function provideSettings(model: SettingsModel) {
  setContext(KEY, model);
}

export function useSettings(): SettingsModel {
  return getContext<SettingsModel>(KEY);
}
