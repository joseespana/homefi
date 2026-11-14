// New versions of HomeFi (src-tauri/src/updater.rs): the banner and Settings →
// About read this. Only GitHub is asked; nothing about the user is sent.
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errorText, on, type UpdateView } from "./api";
import { app } from "./state.svelte";

class Updates {
  view = $state<UpdateView | null>(null);
  /** The "What's new" notes are open in the banner. */
  notesOpen = $state(false);
  checking = $state(false);
  /** The button that started a download or a restart waits for the app to answer. */
  working = $state<"download" | "restart" | null>(null);

  async init() {
    try {
      this.view = await api.updateStatus();
    } catch {
      return;
    }
    on.update((v) => (this.view = v));
  }

  /** The banner shows: a version is out (or on its way in), and "Later" wasn't picked. */
  get banner(): boolean {
    const v = this.view;
    if (!v || !v.version || v.dismissed) return false;
    return v.status === "available" || v.status === "downloading" || v.status === "ready" || v.status === "installing" || v.status === "error";
  }

  async check() {
    this.checking = true;
    try {
      this.view = await api.updateCheck();
    } catch (e) {
      app.note(errorText(e), "error");
    } finally {
      this.checking = false;
    }
  }

  async download() {
    if (this.working) return;
    this.working = "download";
    try {
      await api.updateDownload();
    } catch (e) {
      app.note(errorText(e), "error");
    } finally {
      this.working = null;
    }
  }

  /** "Restart now" asks first while music plays; "When the song ends" waits for it. */
  async restart(when: "now" | "song_end") {
    if (when === "now" && app.player?.status === "playing") {
      const ok = await app.confirm({
        title: "Restart HomeFi now?",
        text: "The music stops while HomeFi installs the update and opens again. Pick “When the song ends” to finish the song first.",
        confirmLabel: "Restart now",
      });
      if (!ok) return;
    }
    if (this.working) return;
    this.working = "restart";
    try {
      await api.updateRestart(when);
    } catch (e) {
      app.note(errorText(e), "error");
    } finally {
      this.working = null;
    }
  }

  async later() {
    this.notesOpen = false;
    await api.updateDismiss();
  }

  /** The release page in the browser ("What's new" in full, or "Download"). */
  openPage() {
    if (this.view) openUrl(this.view.pageUrl);
  }
}

export const updates = new Updates();

/** Why this copy can't update itself, in plain words. */
export function manualReasonText(reason: UpdateView["manualReason"]): string {
  switch (reason) {
    case "dev":
      return "This is a test build: download the new version from the release page.";
    case "dmg":
      return "HomeFi is running from the installer window. Drag it to Applications to get updates from the app.";
    case "translocated":
      return "macOS is running HomeFi from a temporary spot. Move it to Applications and open it again to get updates from the app.";
    case "no-appimage":
      return "This kind of install can't update itself. Download the new version from the release page.";
    case "no-pkexec":
      return "This computer can't ask for the permission needed to install the update. Download it from the release page.";
    case "no-package":
      return "That release doesn't have a download for this computer yet. Download it from the release page.";
    default:
      return "This copy of HomeFi can't install updates by itself. Download the new version from the release page.";
  }
}
