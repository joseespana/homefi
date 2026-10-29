// Release notes (Markdown from the CHANGELOG) as plain blocks for the screen:
// headings, bullets and paragraphs with **bold** and `code`; links keep their
// text. Nothing is rendered as HTML.

export type Inline = { text: string; bold?: boolean; code?: boolean };
export type Block = { kind: "h" | "li" | "p"; parts: Inline[] };

function inline(text: string): Inline[] {
  // [text](url) -> text
  const plain = text.replace(/\[([^\]]+)\]\([^)]*\)/g, "$1");
  const parts: Inline[] = [];
  const re = /(\*\*[^*]+\*\*|`[^`]+`)/g;
  let last = 0;
  for (const m of plain.matchAll(re)) {
    if (m.index! > last) parts.push({ text: plain.slice(last, m.index) });
    const t = m[0];
    parts.push(t.startsWith("**") ? { text: t.slice(2, -2), bold: true } : { text: t.slice(1, -1), code: true });
    last = m.index! + t.length;
  }
  if (last < plain.length) parts.push({ text: plain.slice(last) });
  return parts;
}

export function noteBlocks(markdown: string | null | undefined, max = 60): Block[] {
  const blocks: Block[] = [];
  let para: string[] = [];
  const flush = () => {
    if (para.length) blocks.push({ kind: "p", parts: inline(para.join(" ")) });
    para = [];
  };
  for (const raw of (markdown ?? "").split(/\r?\n/)) {
    const line = raw.trim();
    if (!line || /^-{3,}$/.test(line)) {
      flush();
      continue;
    }
    const h = line.match(/^#{1,6}\s+(.*)$/);
    const li = line.match(/^[-*+]\s+(.*)$/);
    if (h) {
      flush();
      blocks.push({ kind: "h", parts: inline(h[1]) });
    } else if (li) {
      flush();
      blocks.push({ kind: "li", parts: inline(li[1]) });
    } else if (raw.startsWith("  ") && blocks.at(-1)?.kind === "li" && !para.length) {
      // a bullet's wrapped line
      blocks.at(-1)!.parts.push(...inline(" " + line));
    } else {
      para.push(line);
    }
  }
  flush();
  return blocks.slice(0, max);
}
