/** "es" when the system speaks Spanish, else "en": the language the assistant and the ride's words use. */
export function uiLanguage(): "es" | "en" {
  const langs = typeof navigator === "undefined" ? [] : [navigator.language, ...(navigator.languages ?? [])];
  return langs.some((l) => l?.toLowerCase().startsWith("es")) ? "es" : "en";
}
