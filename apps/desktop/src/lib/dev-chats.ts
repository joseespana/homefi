// Dev only (the Vite mock, `?chats`): three AI chats already going, so the AI chats menu and the
// Rail's entry can be seen in screenshots. Never imported in a release build.
import { conversations, type Conversation } from "./party.svelte";

/** Waits for a chat's session to open (the mock answers in a few ms). */
async function opened(c: Conversation) {
  for (let i = 0; i < 50 && !c.opened; i++) await new Promise((r) => setTimeout(r, 40));
}

export async function seedDevChats() {
  const building = conversations.start("chat");
  const failing = conversations.start("chat");
  const idle = conversations.start("party");
  if (!building || !failing || !idle) return;
  await Promise.all([opened(building), opened(failing), opened(idle)]);
  await building.send("Study music, slow and calm");
  void building.build();
  await failing.send("Road trip, make it fail");
  await failing.build();
  conversations.menuOpen = true;
}
