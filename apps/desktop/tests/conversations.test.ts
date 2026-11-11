// The AI chats list rules. Run: `pnpm test`.
import { describe, it } from "node:test";
import { strict } from "node:assert";
import { agoWords, evictable, isWorking, statusWords, tooManyChatsText, type ChatSummary } from "../src/lib/conversations.ts";

const chat = (id: string, kind: ChatSummary["status"]["kind"], lastActivityMs: number): ChatSummary => ({
  id,
  lastActivityMs,
  status: kind === "building" ? { kind, fraction: null } : kind === "failed" ? { kind, message: "x" } : { kind },
});

describe("evictable", () => {
  it("gives way the oldest failed chat", () => {
    strict.equal(evictable([chat("a", "failed", 30), chat("b", "failed", 10), chat("c", "failed", 20)]), "b");
  });
  it("never drops a build, a draft or a chat half done", () => {
    const busy = [chat("a", "building", 1), chat("b", "ready", 2), chat("c", "waiting", 3), chat("d", "thinking", 4)];
    strict.equal(evictable(busy), null);
  });
  it("skips the live ones to find the failed one", () => {
    strict.equal(evictable([chat("a", "building", 1), chat("b", "failed", 50), chat("c", "ready", 2)]), "b");
  });
  it("has nothing to drop in an empty list", () => {
    strict.equal(evictable([]), null);
  });
});

describe("statusWords", () => {
  it("says each state in plain words", () => {
    strict.equal(statusWords({ kind: "waiting" }), "Waiting for you");
    strict.equal(statusWords({ kind: "thinking" }), "Thinking…");
    strict.equal(statusWords({ kind: "building", fraction: null }), "Building…");
    strict.equal(statusWords({ kind: "building", fraction: 0.4 }), "Building · 40%");
    strict.equal(statusWords({ kind: "ready" }), "Playlist ready");
    strict.equal(statusWords({ kind: "failed", message: "x" }), "Didn't work");
  });
  it("counts thinking and building as working", () => {
    strict.equal(isWorking({ kind: "thinking" }), true);
    strict.equal(isWorking({ kind: "building", fraction: 0 }), true);
    strict.equal(isWorking({ kind: "ready" }), false);
  });
});

describe("agoWords", () => {
  const now = 10_000_000_000;
  it("reads minutes, hours and days", () => {
    strict.equal(agoWords(now - 5_000, now), "just now");
    strict.equal(agoWords(now - 5 * 60_000, now), "5 min ago");
    strict.equal(agoWords(now - 2 * 3_600_000, now), "2 h ago");
    strict.equal(agoWords(now - 3 * 86_400_000, now), "3 d ago");
    strict.equal(agoWords(now + 1_000, now), "just now");
  });
});

describe("tooManyChatsText", () => {
  it("names the limit", () => {
    strict.equal(tooManyChatsText(3), "You already have 3 AI chats open. Close one to start another.");
  });
});
