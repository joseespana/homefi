// Arrow-key navigation for radiogroups (a Svelte action): the arrows move focus
// to the next / previous option and choose it (a click), Home / End jump to the ends.
export interface RovingOptions {
  /** The focusable options inside the group. */
  selector: string;
  /** Up / Down also move (lists and galleries); false: only Left / Right. */
  vertical: boolean;
}

export function roving(node: HTMLElement, options: RovingOptions) {
  let current = options;
  function onkeydown(e: KeyboardEvent) {
    const keys = current.vertical ? ["ArrowRight", "ArrowDown", "ArrowLeft", "ArrowUp", "Home", "End"] : ["ArrowRight", "ArrowLeft", "Home", "End"];
    if (!keys.includes(e.key)) return;
    const items = [...node.querySelectorAll<HTMLElement>(current.selector)];
    const at = items.indexOf(document.activeElement as HTMLElement);
    if (!items.length || at < 0) return;
    e.preventDefault();
    const forward = e.key === "ArrowRight" || e.key === "ArrowDown";
    const to = e.key === "Home" ? 0 : e.key === "End" ? items.length - 1 : (at + (forward ? 1 : -1) + items.length) % items.length;
    items[to].focus();
    items[to].click();
  }
  node.addEventListener("keydown", onkeydown);
  return {
    update(next: RovingOptions) {
      current = next;
    },
    destroy() {
      node.removeEventListener("keydown", onkeydown);
    },
  };
}
