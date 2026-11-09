// DJ setups on the screen: the named copies of the whole DJ settings (options, shortcuts, layout, pad
// sounds), saved, applied, exported to a `.homefidj` file and imported, as the Rust side holds them
// (src-tauri/src/dj_setups.rs). Shared by the DJ screen's Setups menu and Settings → DJ.
import { DEFAULT_DJ_SETTINGS, DJ_SETUP_FILE, djSetupApi, onDj, type DjSetupSummary } from "./dj-api";
import { stableJson } from "./dj-layout";
import { dj } from "./dj.svelte";
import { errorText } from "./api";
import { app } from "./state.svelte";
import { pickOpenPath, pickSavePath } from "./transfer";

/** The longest name a setup is given on this screen. */
export const SETUP_NAME_MAX = 60;

class DjSetupsStore {
  /** By name, as the backend lists them. */
  list = $state.raw<DjSetupSummary[]>([]);
  /** The setup the current settings were last saved to or loaded from, with what they said then. */
  private base = $state.raw<{ id: string; json: string } | null>(null);
  private started = false;

  /** The setup in use, if any. */
  activeId = $derived(this.base && this.list.some((s) => s.id === this.base?.id) ? this.base.id : null);
  /** The settings differ from the setup in use (or, with none, from the defaults): applying another would lose them. */
  dirty = $derived(stableJson(dj.settings) !== (this.base?.json ?? stableJson(DEFAULT_DJ_SETTINGS)));

  async init() {
    if (this.started) return;
    this.started = true;
    onDj.setups(() => this.refresh());
    await this.refresh();
  }

  async refresh() {
    try {
      this.list = await djSetupApi.list();
    } catch {}
  }

  /** Runs one call, says its error, re-reads the list. */
  private async run<T>(job: () => Promise<T>): Promise<T | null> {
    try {
      const out = await job();
      await this.refresh();
      return out;
    } catch (e) {
      app.note(errorText(e), "error", "dj-setups");
      return null;
    }
  }

  /** "Save current as": a new setup holding the settings now. */
  async save(name: string) {
    const made = await this.run(() => djSetupApi.save(name.trim().slice(0, SETUP_NAME_MAX)));
    if (made) {
      this.base = { id: made.id, json: stableJson(dj.settings) };
      app.note(`Saved the setup “${made.name}”`, "success", "dj-setups");
    }
    return made;
  }

  /** The setup takes what the settings say now. */
  async overwrite(id: string) {
    const made = await this.run(() => djSetupApi.overwrite(id));
    if (made) {
      this.base = { id, json: stableJson(dj.settings) };
      app.note(`Updated “${made.name}”`, "success", "dj-setups");
    }
  }

  rename = (id: string, name: string) => this.run(() => djSetupApi.rename(id, name.trim().slice(0, SETUP_NAME_MAX)));

  async remove(id: string) {
    if (await this.run(() => djSetupApi.remove(id))) {
      if (this.base?.id === id) this.base = null;
      app.note("Setup deleted", "success", "dj-setups");
    }
  }

  /** The setup's settings replace the current ones; the pads play after a restart of DJ mode. */
  async apply(id: string) {
    const applied = await this.run(() => djSetupApi.apply(id));
    if (!applied) return;
    if (app.settings) app.settings = { ...app.settings, dj: applied.settings };
    this.base = { id, json: stableJson(applied.settings) };
    dj.refreshKeys();
    app.note(applied.restart_needed ? "Setup applied. The new pad sounds play after you restart DJ mode." : "Setup applied", "success", "dj-setups");
  }

  async exportSetup(setup: DjSetupSummary) {
    try {
      const path = await pickSavePath(DJ_SETUP_FILE, setup.name);
      if (!path) return;
      app.note(`Exported to ${await djSetupApi.export(setup.id, path)}`, "success", "dj-setups");
    } catch (e) {
      app.note(errorText(e), "error", "dj-setups");
    }
  }

  async importSetup() {
    try {
      const path = await pickOpenPath(DJ_SETUP_FILE);
      if (!path) return;
      const made = await this.run(() => djSetupApi.import(path));
      if (made) app.note(`Imported “${made.name}”. Apply it to use it.`, "success", "dj-setups");
    } catch (e) {
      app.note(errorText(e), "error", "dj-setups");
    }
  }
}

export const djSetups = new DjSetupsStore();
