// The rules of the AI chats list that need no runes (so `pnpm test` runs them): the status
// words, when a chat is "old", and which chat gives way when the list is full.

export type ChatStatus = { kind: "waiting" } | { kind: "thinking" } | { kind: "building"; fraction: number | null } | { kind: "ready" } | { kind: "failed"; message: string };

/** What the list needs of a chat to pick the one that gives way. */
export interface ChatSummary {
  id: string;
  status: ChatStatus;
  lastActivityMs: number;
}

const PERCENT = 100;
const MS_PER_MINUTE = 60_000;
const MINUTES_PER_HOUR = 60;
const HOURS_PER_DAY = 24;
/** Under this a chat's last activity reads "just now". */
const JUST_NOW_MS = MS_PER_MINUTE;

/** The chat's state in plain words (the AI chats menu). */
export function statusWords(status: ChatStatus): string {
  switch (status.kind) {
    case "waiting":
      return "Waiting for you";
    case "thinking":
      return "Thinking…";
    case "building":
      return status.fraction == null ? "Building…" : `Building · ${Math.round(status.fraction * PERCENT)}%`;
    case "ready":
      return "Playlist ready";
    case "failed":
      return "Didn't work";
  }
}

/** The state a build or a turn in flight puts the chat in; the AI is working on it. */
export const isWorking = (status: ChatStatus): boolean => status.kind === "thinking" || status.kind === "building";

/**
 * The chat that gives way when the list is full: the oldest one whose build failed. A build in
 * progress, an unsaved draft or a chat half done is never dropped silently; null when none fits.
 */
export function evictable(list: readonly ChatSummary[]): string | null {
  let oldest: ChatSummary | null = null;
  for (const c of list) if (c.status.kind === "failed" && (!oldest || c.lastActivityMs < oldest.lastActivityMs)) oldest = c;
  return oldest?.id ?? null;
}

/** "just now", "5 min ago", "2 h ago", "3 d ago". */
export function agoWords(thenMs: number, nowMs: number): string {
  const elapsed = Math.max(0, nowMs - thenMs);
  if (elapsed < JUST_NOW_MS) return "just now";
  const minutes = Math.floor(elapsed / MS_PER_MINUTE);
  if (minutes < MINUTES_PER_HOUR) return `${minutes} min ago`;
  const hours = Math.floor(minutes / MINUTES_PER_HOUR);
  return hours < HOURS_PER_DAY ? `${hours} h ago` : `${Math.floor(hours / HOURS_PER_DAY)} d ago`;
}

/** The refusal when the list is full (the backend says the same if the UI ever let one through). */
export const tooManyChatsText = (max: number): string => `You already have ${max} AI chats open. Close one to start another.`;
