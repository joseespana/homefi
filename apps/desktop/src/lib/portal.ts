// Moves a floating element (tooltip, menu) to <body>: panels clip their children,
// and a transform or backdrop filter on an ancestor makes `position: fixed` local.
export function portal(node: HTMLElement) {
  document.body.appendChild(node);
  return { destroy: () => node.remove() };
}
