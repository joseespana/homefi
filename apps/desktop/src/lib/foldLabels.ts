// Header button rows fold their secondary buttons into round icons on a narrow content area
// (app.css, `.actions.fold`). A folded button keeps its name as a tooltip: this gives every
// button in the row that lacks a `title` its own visible text as one, and keeps it current.
export function foldLabels(row: HTMLElement) {
  const label = () => {
    for (const b of row.querySelectorAll<HTMLElement>(".btn")) {
      const text = b.textContent?.replace(/\s+/g, " ").trim();
      // only titles this action set are refreshed; an author's own title is left alone
      if (text && (!b.title || b.dataset.foldTitle !== undefined)) {
        b.title = text;
        b.dataset.foldTitle = "";
      }
    }
  };
  label();
  const watch = new MutationObserver(label);
  watch.observe(row, { subtree: true, childList: true, characterData: true });
  return { destroy: () => watch.disconnect() };
}
