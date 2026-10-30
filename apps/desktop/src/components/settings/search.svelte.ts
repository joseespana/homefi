// The Settings search box. The page list is filtered by each page's keywords
// (pages.ts); inside the open page every row hides unless its label / hint
// (or its group's title) match, so the search reaches row level without a
// second list of keywords to keep in sync.
class SettingsSearch {
  query = $state("");
  /** Rows of the open page still showing (rows report themselves). */
  shown = $state(0);

  readonly words = $derived(this.query.trim().toLowerCase().split(/\s+/).filter(Boolean));
  readonly active = $derived(this.words.length > 0);

  matches(text: string): boolean {
    if (!this.active) return true;
    const haystack = text.toLowerCase();
    return this.words.every((w) => haystack.includes(w));
  }

  clear() {
    this.query = "";
  }
}

export const settingsSearch = new SettingsSearch();

/** A group tells its rows whether its own title already matched (then all rows show). */
export const GROUP_KEY = Symbol("settings-group");
export interface GroupContext {
  readonly matched: boolean;
}
