// "Thanos snap": an element crumbles into dust that blows away.
// A turbulence displacement filter tears the element apart while particles
// sampled from its area drift off to the upper right.

let filters = 0;

function svgFilter(): { id: string; map: SVGFEDisplacementMapElement; remove: () => void } {
  const ns = "http://www.w3.org/2000/svg";
  const id = `homefi-snap-${++filters}`;
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("width", "0");
  svg.setAttribute("height", "0");
  svg.style.position = "absolute";
  svg.innerHTML = `<filter id="${id}" x="-20%" y="-50%" width="160%" height="200%">
    <feTurbulence type="fractalNoise" baseFrequency="0.035 0.08" numOctaves="2" seed="${filters}" result="noise"/>
    <feDisplacementMap in="SourceGraphic" in2="noise" scale="0" xChannelSelector="R" yChannelSelector="G"/>
  </filter>`;
  document.body.append(svg);
  const map = svg.querySelector("feDisplacementMap") as SVGFEDisplacementMapElement;
  return { id, map, remove: () => svg.remove() };
}

function dust(rect: DOMRect, color: string, count: number, duration: number) {
  const layer = document.createElement("div");
  layer.style.cssText = "position:fixed;inset:0;pointer-events:none;z-index:70;overflow:hidden";
  document.body.append(layer);
  for (let i = 0; i < count; i++) {
    const p = document.createElement("i");
    const size = 2 + Math.random() * 4;
    // particles leave from left to right, like the snap in the film
    const x = rect.left + Math.random() * rect.width;
    const y = rect.top + Math.random() * rect.height;
    const delay = ((x - rect.left) / Math.max(1, rect.width)) * duration * 0.45 + Math.random() * 120;
    p.style.cssText = `position:absolute;left:${x}px;top:${y}px;width:${size}px;height:${size}px;border-radius:50%;background:${
      Math.random() < 0.6 ? color : "rgb(180 180 180 / 0.8)"
    };opacity:0`;
    layer.append(p);
    p.animate(
      [
        { opacity: 0.9, transform: "translate(0, 0) scale(1)" },
        {
          opacity: 0,
          transform: `translate(${40 + Math.random() * 120}px, ${-20 - Math.random() * 60}px) scale(${0.3 + Math.random() * 0.5})`,
        },
      ],
      { duration: duration * (0.6 + Math.random() * 0.4), delay, easing: "cubic-bezier(.2,.6,.4,1)", fill: "forwards" },
    );
  }
  setTimeout(() => layer.remove(), duration * 1.6 + 200);
}

const reduced = () => window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;

/**
 * Crumbles `el` away. With `reform`, it fades back in afterwards (the row stays,
 * only what it represented is gone). Resolves when the dust settled.
 */
export async function snap(el: HTMLElement, { reform = true, duration = 1100 } = {}): Promise<void> {
  if (reduced()) {
    await el.animate([{ opacity: 1 }, { opacity: 0.2 }, { opacity: 1 }], { duration: 400 }).finished;
    return;
  }
  const rect = el.getBoundingClientRect();
  const color = getComputedStyle(document.documentElement).getPropertyValue("--room-accent").trim() || "#5fd4ea";
  dust(rect, color, Math.min(220, Math.round((rect.width * rect.height) / 150)), duration);

  const { id, map, remove } = svgFilter();
  el.style.filter = `url(#${id})`;
  const start = performance.now();
  await new Promise<void>((done) => {
    const frame = (t: number) => {
      const k = Math.min(1, (t - start) / duration);
      map.setAttribute("scale", String(160 * k * k));
      // stays solid while it tears apart, then fades
      el.style.opacity = String(1 - k ** 2.6);
      el.style.transform = `translate(${24 * k}px, ${-8 * k}px)`;
      if (k < 1) requestAnimationFrame(frame);
      else done();
    };
    requestAnimationFrame(frame);
  });
  el.style.filter = "";
  remove();
  if (reform) {
    el.style.transform = "";
    await el.animate([{ opacity: 0, transform: "scale(0.98)" }, { opacity: 1, transform: "none" }], {
      duration: 380,
      easing: "ease-out",
    }).finished;
    el.style.opacity = "";
  }
}

/** Time for `n` rows snapped one after another `stagger` ms apart. */
export const snapTime = (n: number, stagger = 60, duration = 1100) => duration + Math.min(n, 25) * stagger;
