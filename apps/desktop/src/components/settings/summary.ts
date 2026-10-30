// The one line about a page under its name in the page list and in the page header.
import type { Settings } from "../../lib/api";
import { LAYOUTS, fixedTheme, layoutOf, skinOf } from "../../lib/layouts";
import { share } from "../../lib/share.svelte";
import { app } from "../../lib/state.svelte";
import { updates } from "../../lib/update.svelte";
import type { PageId } from "./pages";
import { AI_NAMES, MOTIONS, PROGRESS_STYLES, QUALITIES, SIZE_LABELS, THEMES, offsetLabel } from "./options";

export function summaryOf(id: PageId, s: Settings): string {
  switch (id) {
    case "look": {
      const fixed = fixedTheme(skinOf(s.room));
      const theme = fixed ? THEMES.find((t) => t.id === fixed)?.label : THEMES.find((t) => t.id === (s.theme ?? "system"))?.label;
      const motion = s.room.motion && s.room.motion !== "auto" ? ` · ${MOTIONS.find((m) => m.id === s.room.motion)?.label.toLowerCase()} motion` : "";
      return `${theme} · ${PROGRESS_STYLES.find((p) => p.id === (s.progress_style ?? "wavy"))?.label} bar${motion}`;
    }
    case "room":
      return `${LAYOUTS.find((l) => l.id === layoutOf(s.room.layout))?.label ?? ""}${s.room.layout_everywhere === false ? " · room only" : " · everywhere"}`;
    case "lyrics":
      return `${s.room.lyrics_mode === "karaoke" ? "Karaoke" : "Normal"} · ${SIZE_LABELS[s.room.lyrics_size] ?? "Medium"} text${s.room.lyrics_offset_ms ? ` · ${offsetLabel(s.room.lyrics_offset_ms)}` : ""}`;
    case "mascot":
      return s.room.mascot ? "On" : "Off";
    case "sound": {
      const own = Object.keys(s.eq?.perOutput ?? {}).length;
      const rules = s.taste?.rules?.length ?? 0;
      const loud = s.loudness?.mode && s.loudness.mode !== "off" ? " · loudness" : "";
      return `Equalizer ${s.eq?.enabled ? "on" : "off"}${own ? ` · ${own === 1 ? "1 output" : `${own} outputs`}` : ""}${rules && s.taste?.enabled ? ` · ${rules} taste ${rules === 1 ? "layer" : "layers"}` : ""}${loud}`;
    }
    case "playback": {
      const xf = s.crossfade?.mode === "on" ? `Crossfade ${s.crossfade.seconds} s` : "Gapless";
      const sur = s.output?.surround && s.output.surround !== "frontOnly" ? ` · ${s.output.surround === "upmix" ? "upmix" : "all speakers"}` : "";
      return `${xf}${sur}${s.output?.resampler === "fast" ? " · fast resampler" : ""}`;
    }
    case "ai":
      return s.ai?.provider ? (AI_NAMES[s.ai.provider] ?? s.ai.provider) : "None · guided questions";
    case "share":
      return share.sharing ? `Sharing · ${share.hub.guests.length} guests` : share.inRoom ? `In ${share.guest.session_name}` : `As “${s.share.name || share.name}”`;
    case "downloads":
      return QUALITIES.find((q) => q.id === s.quality)?.label ?? "";
    case "library":
      return s.library_dirs.length === 1 ? "1 folder" : `${s.library_dirs.length} folders`;
    case "tidal":
      return `${app.account ? "Connected" : "Not connected"}${s.lazy_lists === false ? " · whole lists" : ""}${s.album_versions?.group === false ? " · every release" : ""}`;
    case "about": {
      const u = updates.view;
      const out = u?.version && ["available", "downloading", "ready"].includes(u.status) ? ` · ${u.version} available` : "";
      return `HomeFi ${app.info?.version ?? ""}${out}`;
    }
  }
}
