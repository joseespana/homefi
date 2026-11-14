<script lang="ts">
  // The big message in the middle of the ride (the core's `RideView.callout`), as kinetic type: the words slam in
  // big and upper case with a hard shadow while outlined copies burst out above and below and fade. "Sprint in
  // 3 · 2 · 1" over a dimmed ride on the countdown's beeps, then GO!, cheers and the sprint's last seconds, and the
  // ride's last words. One spec with Android (`RideCallout.kt`) and the TV (`ride.html`). Draws only: clicks go through.
  import type { RideCallout } from "../../lib/exercise-api";

  /** `text` is the backend's, in the system's language (the countdown: the words over the digit). */
  let { callout, text: words }: { callout: RideCallout | null | undefined; text: string | null } = $props();

  /** Echo copies, above (-) and below (+). */
  const ECHOES = [-2, -1, 1, 2];

  const sprinting = $derived(!!callout && (callout.kind === "go" || callout.kind === "cheer" || callout.kind === "seconds_left"));
  const text = $derived(callout && callout.kind !== "countdown" ? (words ?? "").toUpperCase() : "");
  const size = $derived(text.length <= 8 ? "short" : text.length <= 18 ? "medium" : "long");
</script>

{#if callout?.kind === "countdown"}
  <div class="layer count" aria-live="polite" aria-label="{words ?? ''} {callout.n}">
    <span class="kin lbl" aria-hidden="true"><b>{words ?? ""}</b></span>
    {#key callout.n}
      <span class="kin digit" aria-hidden="true">
        {#each ECHOES as k (k)}<i style:--k={k} style:--o={1 / Math.abs(k)}>{callout.n}</i>{/each}
        <b>{callout.n}</b>
      </span>
    {/key}
  </div>
{:else if callout && text}
  <div class="layer pool" class:sprint={sprinting} aria-live="polite">
    {#key text}
      <span class="kin {size}" aria-label={text}>
        {#each ECHOES as k (k)}<i aria-hidden="true" style:--k={k} style:--o={1 / Math.abs(k)}>{text}</i>{/each}
        <b>{text}</b>
      </span>
    {/key}
  </div>
{/if}

<style>
  .layer {
    position: absolute;
    inset: 0;
    z-index: 6;
    display: grid;
    place-items: center;
    align-content: center;
    pointer-events: none;
    padding: 0 24px;
    --k-color: var(--c);
  }
  .layer.sprint,
  .count {
    --k-color: var(--z-sprint);
  }
  .pool {
    /* a soft dark pool under the words: readable over the ring, the mountain or the lyrics */
    background: radial-gradient(closest-side, rgb(0 0 0 / 0.55), transparent);
  }
  .count {
    background: rgb(0 0 0 / 0.75);
  }
  .kin {
    position: relative;
    display: grid;
    place-items: center;
    max-width: 92%;
    color: var(--k-color);
    font-weight: 900;
    line-height: 1.02;
    letter-spacing: -0.01em;
    text-align: center;
    text-transform: uppercase;
  }
  .kin.short {
    font-size: clamp(56px, 9cqi, 120px);
  }
  .kin.medium {
    font-size: clamp(40px, 6cqi, 84px);
  }
  .kin.long {
    font-size: clamp(28px, 4cqi, 56px);
  }
  .kin.lbl {
    font-size: clamp(22px, 3cqi, 40px);
  }
  .kin.digit {
    font-size: clamp(140px, 22cqi, 280px);
  }
  .kin b,
  .kin i {
    grid-area: 1 / 1;
    font-style: normal;
  }
  .kin b {
    text-shadow: 0.07em 0.07em 0 #000;
    animation: slam 0.26s cubic-bezier(0.2, 0, 0, 1) both;
  }
  .kin i {
    color: transparent;
    -webkit-text-stroke: 2px var(--k-color);
    opacity: 0;
    animation: echo 0.9s cubic-bezier(0.4, 0, 0.2, 1) both;
  }
  @keyframes slam {
    from {
      opacity: 0;
      transform: scale(1.45);
    }
    50% {
      opacity: 1;
    }
  }
  @keyframes echo {
    from {
      opacity: var(--o);
      transform: translateY(0);
    }
    to {
      opacity: 0;
      transform: translateY(calc(var(--k) * 0.8em));
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .kin b {
      animation: none;
    }
    .kin i {
      display: none;
    }
  }
</style>
