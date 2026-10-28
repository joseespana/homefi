<script lang="ts">
  // A QR code drawn as SVG (qrcode-generator, no network): dark modules on a
  // white card, so phone cameras read it in both themes.
  import qrcode from "qrcode-generator";

  let { text, size = 200 }: { text: string; size?: number } = $props();

  const code = $derived.by(() => {
    const qr = qrcode(0, "M");
    qr.addData(text);
    qr.make();
    const n = qr.getModuleCount();
    let path = "";
    for (let r = 0; r < n; r++) for (let c = 0; c < n; c++) if (qr.isDark(r, c)) path += `M${c} ${r}h1v1h-1z`;
    return { n, path };
  });
</script>

<div class="qr" style:width="{size}px" style:height="{size}px">
  <svg viewBox="-2 -2 {code.n + 4} {code.n + 4}" width="100%" height="100%" shape-rendering="crispEdges" role="img" aria-label="QR code to join">
    <rect x="-2" y="-2" width={code.n + 4} height={code.n + 4} fill="#fff" />
    <path d={code.path} fill="#0b1e3a" />
  </svg>
</div>

<style>
  .qr {
    flex: none;
    border-radius: min(14px, var(--r-max));
    overflow: hidden;
    box-shadow: 0 0 0 1px var(--border);
  }
</style>
