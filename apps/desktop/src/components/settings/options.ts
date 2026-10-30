// The choices several parts of Settings share (a page and the page list's summary line).
import type { Motion, ProgressStyle, Quality, Theme } from "../../lib/api";

export const QUALITIES: { id: Quality; label: string; hint: string }[] = [
  { id: "low", label: "Low", hint: "96 kbps AAC" },
  { id: "normal", label: "Normal", hint: "320 kbps AAC" },
  { id: "high", label: "Lossless", hint: "16-bit / 44.1 kHz FLAC" },
  { id: "max", label: "Max", hint: "up to 24-bit / 192 kHz FLAC" },
];

export const PROGRESS_STYLES: { id: ProgressStyle; label: string }[] = [
  { id: "classic", label: "Classic" },
  { id: "wavy", label: "Wavy" },
  { id: "waveform", label: "Waveform" },
  { id: "neon", label: "Neon" },
  { id: "led", label: "LED" },
  { id: "minimal", label: "Minimal" },
];

export const MOTIONS: { id: Motion; label: string; hint: string }[] = [
  { id: "auto", label: "Automatic", hint: "Full, or subtle when the system asks for less motion" },
  { id: "full", label: "Full", hint: "Views slide in, menus pop, lists cascade" },
  { id: "subtle", label: "Subtle", hint: "Short fades, nothing moves around" },
  { id: "off", label: "Off", hint: "Everything appears at once" },
];

export const THEMES: { id: Theme; label: string; hint: string }[] = [
  { id: "system", label: "System", hint: "Follows macOS / Windows / Linux" },
  { id: "light", label: "Light", hint: "Always light" },
  { id: "dark", label: "Dark", hint: "Always dark" },
];

export const SIZE_LABELS: Record<string, string> = { small: "Small", medium: "Medium", large: "Large" };

export const offsetLabel = (ms: number) => (ms === 0 ? "In time" : `${ms > 0 ? "+" : "−"}${(Math.abs(ms) / 1000).toFixed(1)} s ${ms > 0 ? "later" : "earlier"}`);

export const AI_NAMES: Record<string, string> = {
  anthropic: "Anthropic (Claude)",
  openai: "OpenAI",
  gemini: "Google Gemini",
  openrouter: "OpenRouter",
  mistral: "Mistral",
  groq: "Groq",
  xai: "xAI (Grok)",
  deepseek: "DeepSeek",
  ollama: "Ollama",
  openai_compatible: "Custom server",
};
