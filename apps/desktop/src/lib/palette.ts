// Colors of the current album art, used to tint the whole "room".

export interface Palette {
  /** Light accent: highlights on the dark theme. */
  accent: string;
  glow: string;
  /** Dark background of the room. */
  deep: string;
  /** Dark accent: text and icons on the light theme. */
  ink: string;
  /** Light background of the room. */
  paper: string;
}

export const BRAND: Palette = { accent: "#5fd4ea", glow: "#0891b2", deep: "#06161d", ink: "#0e7490", paper: "#eef6f8" };

function rgbToHsl(r: number, g: number, b: number): [number, number, number] {
  r /= 255;
  g /= 255;
  b /= 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;
  if (max === min) return [0, 0, l];
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  const h = max === r ? (g - b) / d + (g < b ? 6 : 0) : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return [h * 60, s, l];
}

const hsl = (h: number, s: number, l: number) => `hsl(${h.toFixed(0)} ${(s * 100).toFixed(0)}% ${(l * 100).toFixed(0)}%)`;

const cache = new Map<string, Palette>();

/** Dominant vivid hue of an image, as accent / glow / deep background colors. */
export async function paletteOf(src: string): Promise<Palette> {
  const hit = cache.get(src);
  if (hit) return hit;
  try {
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.src = src;
    await img.decode();
    const size = 28;
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = size;
    const ctx = canvas.getContext("2d", { willReadFrequently: true })!;
    ctx.drawImage(img, 0, 0, size, size);
    const { data } = ctx.getImageData(0, 0, size, size);

    // weight 12 hue buckets by saturation so vivid colors win over greys
    const buckets = Array.from({ length: 12 }, () => ({ w: 0, h: 0, s: 0, l: 0 }));
    let greyL = 0;
    let grey = 0;
    for (let i = 0; i < data.length; i += 4) {
      const [h, s, l] = rgbToHsl(data[i], data[i + 1], data[i + 2]);
      if (s < 0.18 || l < 0.08 || l > 0.94) {
        greyL += l;
        grey++;
        continue;
      }
      const w = s * (1 - Math.abs(l - 0.5));
      const b = buckets[Math.floor(h / 30) % 12];
      b.w += w;
      b.h += h * w;
      b.s += s * w;
      b.l += l * w;
    }
    const best = buckets.reduce((a, b) => (b.w > a.w ? b : a));
    let result: Palette;
    if (best.w < 2) {
      // black & white artwork: keep the brand hue, follow its brightness
      const avg = grey ? greyL / grey : 0.3;
      result = {
        accent: hsl(190, 0.8, 0.66),
        glow: hsl(190, 0.6, 0.25 + avg * 0.2),
        deep: hsl(195, 0.5, 0.07),
        ink: hsl(192, 0.85, 0.3),
        paper: hsl(195, 0.35, 0.95),
      };
    } else {
      const h = best.h / best.w;
      const s = Math.max(0.55, best.s / best.w);
      result = {
        accent: hsl(h, s, 0.68),
        glow: hsl(h, s * 0.85, 0.36),
        deep: hsl(h, s * 0.55, 0.08),
        // dark enough for text on white (yellows need to go darker than blues)
        ink: hsl(h, Math.min(0.9, s), h > 40 && h < 75 ? 0.26 : 0.32),
        paper: hsl(h, s * 0.4, 0.95),
      };
    }
    cache.set(src, result);
    return result;
  } catch (e) {
    console.warn("[homefi] palette failed", e);
    return BRAND;
  }
}

export function applyPalette(p: Palette) {
  const root = document.documentElement.style;
  root.setProperty("--room-accent", p.accent);
  root.setProperty("--room-glow", p.glow);
  root.setProperty("--room-deep", p.deep);
  root.setProperty("--room-ink", p.ink);
  root.setProperty("--room-paper", p.paper);
}

const lightness = new Map<string, number>();

/**
 * How light the lower part of an image is, 0 (black) .. 1 (white): where the
 * immersive layout writes the title. 0.5 when the image can't be read.
 */
export async function lowerLightness(src: string): Promise<number> {
  const hit = lightness.get(src);
  if (hit != null) return hit;
  try {
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.src = src;
    await img.decode();
    const w = 32;
    const h = 32;
    const canvas = document.createElement("canvas");
    canvas.width = w;
    canvas.height = h;
    const ctx = canvas.getContext("2d", { willReadFrequently: true })!;
    ctx.drawImage(img, 0, 0, w, h);
    // the bottom 40 %, left two thirds: where the text sits
    const { data } = ctx.getImageData(0, Math.floor(h * 0.6), Math.floor(w * 0.67), Math.ceil(h * 0.4));
    let sum = 0;
    for (let i = 0; i < data.length; i += 4) sum += (0.2126 * data[i] + 0.7152 * data[i + 1] + 0.0722 * data[i + 2]) / 255;
    const value = sum / (data.length / 4);
    lightness.set(src, value);
    return value;
  } catch {
    return 0.5;
  }
}
