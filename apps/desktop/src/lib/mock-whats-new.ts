// Development-only backend for "What's new" (mock.ts spreads `handlers`). Same shapes as
// src-tauri/src/whats_new.rs, with the real 1.5.1 / 1.5.0 entries of CHANGELOG.md.
// Flags: ?whatsnew opens the dialog with one version, ?whatsnewmany with two, ?whatsnewempty with
// a version without notes; About → What's new → Show always works.
import type { NoteLine, NoteSpan, ReleaseNotes } from "./api";

const url = new URLSearchParams(location.search);

const t = (text: string, extra: Partial<NoteSpan> = {}): NoteSpan => ({ text, bold: false, code: false, link: null, ...extra });
const line = (...spans: NoteSpan[]): NoteLine => ({ spans });

const v151: ReleaseNotes = {
  version: "1.5.1",
  date: "2026-11-02",
  summary: line(t("One Sound screen instead of the equalizer and its taste rules, built around what is playing now, on desktop and Android.")),
  sections: [
    {
      title: "Added",
      items: [
        line(t("Sound: ", { bold: true }), t("one sound can apply to many songs, albums, artists, playlists and genres, and can be limited to some outputs (\"only on my WH-1000XM5\"). Add or remove music from the Playing now card, the Edit sound panel or the new "), t("Sound ▸", { code: true }), t(" menu on any song, album, artist or playlist. Moving music between sounds can be undone for 8 seconds.")),
        line(t("Try: ", { bold: true }), t("hear a sound (yours or one the AI proposes) on the song playing without saving it; Keep saves it for that song, album, artist or playlist. Leaving the song ends the try.")),
        line(t("By music: ", { bold: true }), t("every song, album, artist, playlist and genre with its sound, sortable and filterable, to move or remove them in one place.")),
      ],
    },
    {
      title: "Changed",
      items: [
        line(t("The equalizer and the taste rules are now one screen: "), t("Sound", { bold: true }), t(". Your saved profiles and rules carry over untouched.")),
        line(t("Read the full story in the "), t("changelog", { link: "https://github.com/crodaso/homefi/blob/main/CHANGELOG.md" }), t(".")),
      ],
    },
  ],
};

const v150: ReleaseNotes = {
  version: "1.5.0",
  date: "2026-10-30",
  summary: null,
  sections: [
    {
      title: "Fixed",
      items: [
        line(t("Desktop (Windows): ", { bold: true }), t("surround and virtual surround outputs such as Razer's \"7.1 Surround Sound\" played with heavy interference. Streams now carry the device's speaker positions instead of none, through a patched copy of cpal 0.16 ("), t("third_party/cpal", { code: true }), t(", see its "), t("HOMEFI.md", { code: true }), t(").")),
      ],
    },
  ],
};

const empty: ReleaseNotes = { version: "1.5.2", date: "", summary: null, sections: [] };

export const whatsNewMock = {
  handlers: {
    whats_new_check: (): ReleaseNotes[] => (url.has("whatsnewmany") ? [v151, v150] : url.has("whatsnewempty") ? [empty] : url.has("whatsnew") ? [v151] : []),
    whats_new_latest: (): ReleaseNotes | null => (url.has("whatsnewempty") ? null : v151),
  },
};
