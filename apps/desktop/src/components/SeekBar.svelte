<script lang="ts">
  // The seek bar in the style picked in Settings (same set as the Android app):
  // classic, wavy (flat when paused), waveform, neon, LED and minimal.
  import { api, type ProgressStyle } from "../lib/api";

  let {
    fraction,
    style = "wavy",
    thickness = "medium",
    animated = true,
    playing = false,
    seed = 0,
    disabled = false,
    onseek,
    onscrub,
  }: {
    fraction: number;
    style?: ProgressStyle;
    thickness?: "thin" | "medium" | "thick";
    animated?: boolean;
    playing?: boolean;
    seed?: number;
    disabled?: boolean;
    /** Final position (0..1) after a click or a drag. */
    onseek?: (f: number) => void;
    /** Position while dragging, to show the time. */
    onscrub?: (f: number | null) => void;
  } = $props();

  let canvas: HTMLCanvasElement;
  let dragging = $state<number | null>(null);
  let amp = 0;
  let spectrum: number[] = [];

  // pseudo-random bar heights per track (a look, not the song's real waveform)
  const heights = $derived.by(() => {
    let x = (seed | 0) || 1;
    return Array.from({ length: 72 }, () => {
      x ^= x << 13;
      x ^= x >> 17;
      x ^= x << 5;
      return 0.25 + ((x >>> 0) % 1000) / 1333;
    });
  });

  const px = $derived({ thin: 3, medium: 5, thick: 8 }[thickness]);

  function color(name: string, fallback: string) {
    return getComputedStyle(canvas).getPropertyValue(name).trim() || fallback;
  }

  function draw(t: number) {
    const dpr = window.devicePixelRatio || 1;
    const w = canvas.clientWidth;
    const h = canvas.clientHeight;
    if (canvas.width !== Math.round(w * dpr)) {
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
    }
    const g = canvas.getContext("2d")!;
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.clearRect(0, 0, w, h);
    const live = animated && playing && dragging === null;
    amp += ((live ? 1 : 0) - amp) * 0.12; // the wave settles when paused
    const f = dragging ?? fraction;
    const x = w * Math.min(1, Math.max(0, f));
    const y = h / 2;
    const accent = color("--room-accent", "#5fd4ea");
    const rest = color("--glass-border", "rgba(255,255,255,0.15)");
    const phase = (t / 1400) * Math.PI * 2;
    g.lineCap = "round";
    const line = (c: string, from: number, to: number, width: number) => {
      if (to <= from) return;
      g.strokeStyle = c;
      g.lineWidth = width;
      g.beginPath();
      g.moveTo(from, y);
      g.lineTo(to, y);
      g.stroke();
    };
    switch (style) {
      case "classic":
        line(rest, 0, w, px);
        line(accent, 0, x, px);
        g.fillStyle = "#fff";
        g.shadowColor = accent;
        g.shadowBlur = 10;
        g.beginPath();
        g.arc(x, y, px * 1.4 + 3, 0, Math.PI * 2);
        g.fill();
        g.shadowBlur = 0;
        break;
      case "wavy": {
        line(rest, x, w, px);
        g.strokeStyle = accent;
        g.lineWidth = px;
        g.beginPath();
        for (let i = 0; i <= x; i += 2) {
          const yy = y + 4 * amp * Math.sin((i / 26) * Math.PI * 2 - phase);
          if (i === 0) g.moveTo(i, yy);
          else g.lineTo(i, yy);
        }
        g.stroke();
        g.fillStyle = accent;
        g.beginPath();
        g.roundRect(x - px / 2, y - 9, px, 18, px);
        g.fill();
        break;
      }
      case "waveform": {
        const n = heights.length;
        const bw = w / n;
        for (let i = 0; i < n; i++) {
          const cx = i * bw + bw / 2;
          const near = 1 - Math.min(1, Math.abs(cx - x) / (w * 0.12));
          const livePart = (spectrum[i % Math.max(1, spectrum.length)] ?? 0) * amp;
          const hh = Math.max(0.1, Math.min(1, heights[i] * (1 - near * 0.4) + livePart * near)) * h * 0.9;
          g.fillStyle = cx <= x ? accent : rest;
          g.beginPath();
          g.roundRect(cx - bw * 0.3, y - hh / 2, bw * 0.6, hh, bw * 0.3);
          g.fill();
        }
        break;
      }
      case "neon": {
        line(rest, 0, w, px);
        const grad = g.createLinearGradient(0, 0, w, 0);
        grad.addColorStop(0, "#1d4ed8");
        grad.addColorStop(0.5, "#0891b2");
        grad.addColorStop(1, "#10b981");
        g.globalAlpha = 0.3;
        line(grad as unknown as string, 0, x, px * 3);
        g.globalAlpha = 1;
        line(grad as unknown as string, 0, x, px);
        const pulse = 0.7 + 0.3 * Math.sin(phase * 2) * amp;
        g.fillStyle = `rgba(16,185,129,${0.35 * pulse})`;
        g.beginPath();
        g.arc(x, y, px * 4, 0, Math.PI * 2);
        g.fill();
        g.fillStyle = "#fff";
        g.beginPath();
        g.arc(x, y, px * 1.4, 0, Math.PI * 2);
        g.fill();
        break;
      }
      case "led": {
        const n = 48;
        const gap = 2;
        const sw = (w - gap * (n - 1)) / n;
        const sh = px * 2.2;
        for (let i = 0; i < n; i++) {
          const lit = (i + 0.5) / n <= f;
          g.fillStyle = !lit ? rest : i > n * 0.85 ? "#e5484d" : i > n * 0.65 ? "#e5c04d" : "#3dd68c";
          g.fillRect(i * (sw + gap), y - sh / 2, sw, sh);
        }
        break;
      }
      case "minimal":
        line(rest, 0, w, px * 0.6);
        line(accent, 0, x, px * 0.6);
        break;
    }
  }

  $effect(() => {
    // read the props so the effect redraws when they change
    void [fraction, style, thickness, animated, playing, heights, dragging];
    let raf = 0;
    let poll = 0;
    const loop = (t: number) => {
      draw(t);
      // keep animating while the wave settles or the music plays
      if ((animated && playing) || amp > 0.01) raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);
    if (style === "waveform" && animated && playing) {
      const tick = async () => {
        spectrum = (await api.visual(24).catch(() => null))?.spectrum ?? [];
      };
      poll = window.setInterval(tick, 80);
    }
    return () => {
      cancelAnimationFrame(raf);
      clearInterval(poll);
    };
  });

  const at = (e: PointerEvent) => {
    const r = canvas.getBoundingClientRect();
    return Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
  };
</script>

<canvas
  bind:this={canvas}
  class:disabled
  role="slider"
  tabindex={disabled ? -1 : 0}
  aria-label="Seek"
  aria-valuemin={0}
  aria-valuemax={100}
  aria-valuenow={Math.round(fraction * 100)}
  onpointerdown={(e) => {
    if (disabled || !onseek) return;
    canvas.setPointerCapture(e.pointerId);
    dragging = at(e);
    onscrub?.(dragging);
  }}
  onpointermove={(e) => {
    if (dragging === null) return;
    dragging = at(e);
    onscrub?.(dragging);
  }}
  onpointerup={(e) => {
    if (dragging === null) return;
    onseek?.(at(e));
    dragging = null;
    onscrub?.(null);
  }}
  onkeydown={(e) => {
    if (disabled || !onseek) return;
    if (e.key === "ArrowRight") onseek(Math.min(1, fraction + 0.02));
    if (e.key === "ArrowLeft") onseek(Math.max(0, fraction - 0.02));
  }}
></canvas>

<style>
  canvas {
    flex: 1;
    width: 100%;
    height: 28px;
    cursor: pointer;
    touch-action: none;
  }
  canvas.disabled {
    cursor: default;
    opacity: 0.5;
  }
</style>
