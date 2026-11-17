// "Ask before spending": every AI action asks the core's guard first (homefi-core `ai::usage`).
// Confirm → the dialog with what it usually costs and the month so far; blocked → the monthly
// limit is reached, said plainly, with a way to the AI settings. The core also enforces the limit
// on every call, so this is the friendly part, not the only one.
import { api, type AiFeature, type AiSpendDecision } from "./api";
import { uiLanguage } from "./language";
import { app } from "./state.svelte";

/** True when the action may start. A failed check never stops the user (the core still guards). */
export async function spendOk(feature: AiFeature): Promise<boolean> {
  // no AI set up: nothing will be spent
  if (!app.settings?.ai?.provider) return true;
  let decision: AiSpendDecision;
  try {
    decision = await api.aiSpendCheck(feature, uiLanguage());
  } catch {
    return true;
  }
  const es = uiLanguage() === "es";
  switch (decision.kind) {
    case "allow":
      return true;
    case "confirm":
      return app.confirm({ title: decision.prompt.title, text: decision.prompt.text, confirmLabel: es ? "Continuar" : "Continue", cancelLabel: es ? "Cancelar" : "Cancel" });
    case "blocked": {
      const open = await app.confirm({ title: decision.prompt.title, text: decision.prompt.text, confirmLabel: es ? "Abrir ajustes de IA" : "Open AI settings", cancelLabel: es ? "Entendido" : "OK" });
      if (open) app.view = { name: "settings", page: "ai" };
      return false;
    }
  }
}
