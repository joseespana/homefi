// "What's new": the notes of the version just installed, shown once (the core decides and records),
// and reopened from Settings → About. The notes are embedded in the app: no network, no loading state.
import { api, type ReleaseNotes } from "./api";

class WhatsNew {
  notes = $state.raw<ReleaseNotes[]>([]);
  /** The dialog wants to be shown (App holds it back while login / terms are up). */
  open = $state(false);
  private checked = false;

  /** Once per launch, right after the app is ready. Failures are silent: it never interrupts. */
  async checkOnce() {
    if (this.checked) return;
    this.checked = true;
    try {
      const notes = await api.whatsNewCheck();
      if (notes.length) this.show(notes);
    } catch {
      /* no dialog */
    }
  }

  /** About → Show: the version that is running; with no notes the dialog says so. */
  async showLatest() {
    let latest: ReleaseNotes | null = null;
    try {
      latest = await api.whatsNewLatest();
    } catch {
      /* shows the empty state */
    }
    this.show(latest ? [latest] : [{ version: "", date: "", summary: null, sections: [] }]);
  }

  close() {
    this.open = false;
  }

  private show(notes: ReleaseNotes[]) {
    this.notes = notes;
    this.open = true;
  }
}

export const whatsNew = new WhatsNew();
