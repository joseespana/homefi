// The looks of the now-playing room: the same layouts as the Android player.
import type { PlayerLayout } from "./api";

export const LAYOUTS: { id: PlayerLayout; label: string; hint: string }[] = [
  { id: "classic", label: "Classic", hint: "Cover, meters, visualizer and mascot" },
  { id: "immersive", label: "Immersive", hint: "The album fills the room" },
  { id: "vinyl", label: "Vinyl", hint: "A record turning on the platter" },
  { id: "studio", label: "Studio", hint: "Spectrum and VU meters" },
  { id: "retro_amp", label: "Retro Amp", hint: "Green LCD, faders and playlist" },
  { id: "luna", label: "Luna", hint: "Blue glass and an ambience around the album" },
  { id: "aluminum", label: "Aluminum", hint: "Brushed metal and Cover Flow" },
  { id: "web2000", label: "Web 2000", hint: "Tabs, glossy buttons and up next" },
];

export function layoutOf(id: string | undefined): PlayerLayout {
  return LAYOUTS.some((l) => l.id === id) ? (id as PlayerLayout) : "classic";
}

/** Layouts that take the whole height of the window (all but classic; Aluminum even with nothing playing). */
export function fillsRoom(layout: PlayerLayout, playing: boolean): boolean {
  return layout !== "classic" && (playing || layout === "aluminum");
}

/** Bits and kHz of a quality line like "FLAC · 24-bit / 96 kHz". */
export function qualityParts(quality: string | null | undefined): { bits: string; khz: string } {
  const q = quality ?? "";
  return { khz: /([\d.]+)\s*kHz/.exec(q)?.[1] ?? "44.1", bits: /(\d+)-bit/.exec(q)?.[1] ?? "16" };
}

/** The skin of the whole app: the room layout, unless it only dresses the room (`layout_everywhere` off). */
export function skinOf(room: { layout?: string; layout_everywhere?: boolean } | undefined): PlayerLayout {
  return room?.layout_everywhere === false ? "classic" : layoutOf(room?.layout);
}

/** Skins from an era with one look: they keep it whatever the Theme setting says. */
export function fixedTheme(skin: PlayerLayout): "light" | "dark" | null {
  if (skin === "studio" || skin === "retro_amp") return "dark";
  if (skin === "luna" || skin === "aluminum" || skin === "web2000") return "light";
  return null;
}

/**
 * The room layouts draw their own transport (play, next, shuffle, seek) only when
 * they dress the room alone: as the skin of the whole app the player bar has them.
 */
export function ownControls(room: { layout_everywhere?: boolean } | undefined): boolean {
  return room?.layout_everywhere === false;
}
