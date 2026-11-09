// Export and import files (rides, DJ setups): the save / open dialogs. A Google Drive for desktop
// folder is just a folder in the dialog, so there is no Drive code here. The file's text and its
// rules are homefi-core's; the app shell writes and reads it at the path picked here.
import { open, save } from "@tauri-apps/plugin-dialog";

export interface FileKind {
  extension: string;
  label: string;
}

/** The longest base name proposed to the dialog. */
const MAX_BASE_NAME = 80;

/** "30 min · Classic · Rock" → "30 min - Classic - Rock": a name any file system accepts. */
export function fileNameOf(name: string, kind: FileKind): string {
  const base = name
    .replace(/[\\/:*?"<>|\u0000-\u001f]/g, "-")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, MAX_BASE_NAME)
    .trim();
  return `${base || "HomeFi"}.${kind.extension}`;
}

/** Where to write: the save dialog with a name proposed; null when the user cancels. */
export async function pickSavePath(kind: FileKind, name: string): Promise<string | null> {
  return save({ defaultPath: fileNameOf(name, kind), filters: [{ name: kind.label, extensions: [kind.extension] }] });
}

/** What to read: the open dialog; null when the user cancels. */
export async function pickOpenPath(kind: FileKind): Promise<string | null> {
  const picked = await open({ multiple: false, directory: false, filters: [{ name: kind.label, extensions: [kind.extension] }] });
  return typeof picked === "string" ? picked : null;
}

/** "3 days ago", "today", "never": for the lists. */
export function ago(ms: number | null, now: number = Date.now()): string {
  if (ms === null) return "never";
  const days = Math.floor((now - ms) / 86_400_000);
  if (days <= 0) return "today";
  if (days === 1) return "yesterday";
  if (days < 30) return `${days} days ago`;
  const months = Math.floor(days / 30);
  return months < 12 ? `${months} ${months === 1 ? "month" : "months"} ago` : `${Math.floor(months / 12)} y ago`;
}
