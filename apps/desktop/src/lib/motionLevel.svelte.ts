// The motion level in effect (Settings → Look → Motion, "auto" resolved), as something components can react to.
// `applyMotion` writes it to `data-motion` on <html>; this reads it back, so there is one place that decides.
export type MotionLevel = "full" | "subtle" | "off";

const read = (): MotionLevel => {
  const v = document.documentElement.dataset.motion;
  return v === "subtle" || v === "off" ? v : "full";
};

class MotionWatch {
  level = $state<MotionLevel>(read());
  constructor() {
    new MutationObserver(() => (this.level = read())).observe(document.documentElement, { attributes: true, attributeFilter: ["data-motion"] });
  }
}

export const motionWatch = new MotionWatch();
