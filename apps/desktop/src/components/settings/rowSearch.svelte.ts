// What every searchable block of a page (a row, a choice list, a folder list)
// shares: it shows only when the search matches its words or its group's title,
// and reports itself while showing so the page can say "nothing here matches".
import { getContext, untrack } from "svelte";
import { GROUP_KEY, settingsSearch, type GroupContext } from "./search.svelte";

export function useRowSearch(words: () => string): { readonly visible: boolean } {
  const group = getContext<GroupContext | undefined>(GROUP_KEY);
  const visible = $derived(!settingsSearch.active || !!group?.matched || settingsSearch.matches(words()));
  $effect(() => {
    if (!visible) return;
    // untracked: counting must not make the effect depend on its own counter
    untrack(() => settingsSearch.shown++);
    return () => untrack(() => settingsSearch.shown--);
  });
  return {
    get visible() {
      return visible;
    },
  };
}
