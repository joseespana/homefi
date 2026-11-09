// The DJ screen's layout as pure functions over homefi_core's `DjLayout` (order and visibility of the
// sections, the sound of each pad). The core clamps whatever is saved; these keep what the screen
// draws honest while it is edited, and group the sections into the rows the screen shows.
import { DJ_PADS, DJ_SECTION_KINDS, SFX_KINDS, defaultDjLayout, type DjLayout, type DjSection, type DjSectionKind, type SfxKind } from "./dj-api";

/** Each section exactly once and exactly `DJ_PADS` pads (a copy of `DjLayout::clamped`). */
export function normalizeLayout(layout: Partial<DjLayout> | null | undefined): DjLayout {
  const sections: DjSection[] = [];
  for (const s of layout?.sections ?? []) if (DJ_SECTION_KINDS.includes(s.kind) && !sections.some((x) => x.kind === s.kind)) sections.push({ kind: s.kind, visible: s.visible !== false });
  for (const kind of DJ_SECTION_KINDS) if (!sections.some((x) => x.kind === kind)) sections.push({ kind, visible: true });
  const given = layout?.pads ?? [];
  const pads = Array.from({ length: DJ_PADS }, (_, i) => (SFX_KINDS.includes(given[i] as SfxKind) ? given[i] : SFX_KINDS[i]));
  return { sections, pads };
}

/** `from` moved to `to` (both indexes into the sections); a no-op outside the list. */
export function moveSection(layout: DjLayout, from: number, to: number): DjLayout {
  if (from === to || from < 0 || to < 0 || from >= layout.sections.length || to >= layout.sections.length) return layout;
  const sections = [...layout.sections];
  const [moved] = sections.splice(from, 1);
  sections.splice(to, 0, moved);
  return { ...layout, sections };
}

export const setSectionVisible = (layout: DjLayout, kind: DjSectionKind, visible: boolean): DjLayout => ({
  ...layout,
  sections: layout.sections.map((s) => (s.kind === kind ? { ...s, visible } : s)),
});

export const setPad = (layout: DjLayout, pad: number, sound: SfxKind): DjLayout => (pad < 0 || pad >= layout.pads.length ? layout : { ...layout, pads: layout.pads.map((p, i) => (i === pad ? sound : p)) });

export const isDefaultLayout = (layout: DjLayout): boolean => JSON.stringify(layout) === JSON.stringify(defaultDjLayout());

export const samePads = (a: readonly SfxKind[], b: readonly SfxKind[]): boolean => a.length === b.length && a.every((p, i) => p === b[i]);

/** The screen's two bands: the decks and the mixer fill the height; the sampler and the crate sit lower. */
export type DjBand = "stage" | "low";
export const BAND_OF: Record<DjSectionKind, DjBand> = { deck_a: "stage", mixer: "stage", deck_b: "stage", sampler: "low", crate: "low" };

export interface DjRow {
  band: DjBand;
  kinds: DjSectionKind[];
}

/** The visible sections in order, consecutive ones of the same band sharing a row. The default
 *  order gives today's two rows: deck A | mixer | deck B, then sampler | crate. */
export function rowsOf(layout: DjLayout, hidden: readonly DjSectionKind[] = []): DjRow[] {
  const rows: DjRow[] = [];
  for (const s of layout.sections) {
    if (!s.visible || hidden.includes(s.kind)) continue;
    const band = BAND_OF[s.kind];
    const last = rows[rows.length - 1];
    if (last && last.band === band) last.kinds.push(s.kind);
    else rows.push({ band, kinds: [s.kind] });
  }
  return rows;
}

/** A setting's JSON with sorted keys, to tell whether two settings say the same. */
export function stableJson(value: unknown): string {
  return JSON.stringify(value, (_, v) => (v && typeof v === "object" && !Array.isArray(v) ? Object.fromEntries(Object.entries(v).sort(([a], [b]) => (a < b ? -1 : 1))) : v));
}
