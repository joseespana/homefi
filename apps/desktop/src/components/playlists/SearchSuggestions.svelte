<script lang="ts">
  // Google's Search Suggestions widget, which Google requires next to a grounded (Gemini) result.
  // Its HTML is third-party, so it runs in a sandboxed frame: scripts only, never same-origin,
  // no popups, no top navigation. The one script is a file under 'self' (the app's CSP is
  // inherited by srcdoc), which forwards clicks and the frame's height to this component.
  import { openUrl } from "@tauri-apps/plugin-opener";

  let { html }: { html: string } = $props();

  /** The frame's height stays between these (px). */
  const MIN_HEIGHT_PX = 40;
  const MAX_HEIGHT_PX = 320;
  const GOOGLE_HOST = "google.com";

  const doc = $derived('<!doctype html><meta charset="utf-8"><base target="_blank"><style>html{overflow:hidden}</style>' + html + '<script src="/search-widget-frame.js"><' + "/script>");
  let frame = $state<HTMLIFrameElement>();
  let height = $state(MIN_HEIGHT_PX);

  /** Only https links to Google are opened. */
  function allowed(href: unknown): href is string {
    if (typeof href !== "string") return false;
    try {
      const u = new URL(href);
      return u.protocol === "https:" && (u.hostname === GOOGLE_HOST || u.hostname.endsWith("." + GOOGLE_HOST));
    } catch {
      return false;
    }
  }

  function onMessage(e: MessageEvent) {
    if (!frame || e.source !== frame.contentWindow) return;
    const data = e.data as { homefiHeight?: unknown; homefiOpen?: unknown } | null;
    if (typeof data?.homefiHeight === "number") height = Math.min(MAX_HEIGHT_PX, Math.max(MIN_HEIGHT_PX, Math.ceil(data.homefiHeight)));
    else if (allowed(data?.homefiOpen)) openUrl(data.homefiOpen).catch(() => {});
  }
</script>

<svelte:window onmessage={onMessage} />

<iframe bind:this={frame} title="Google Search suggestions" sandbox="allow-scripts" srcdoc={doc} style:height="{height}px"></iframe>

<style>
  iframe {
    display: block;
    width: 100%;
    border: 0;
    background: transparent;
    color-scheme: normal;
  }
</style>
