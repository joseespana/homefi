<script lang="ts">
  // What comes next, so a change is never a surprise: a dashed card in the next part's colour; inside the notice
  // window it fills in and counts down.
  import { clock, positionLabel, shownLabel, zoneName, zoneVar, type NextView } from "../../lib/exercise-api";

  let { next, soonIn, notice }: { next: NextView; soonIn: number; notice: boolean } = $props();
</script>

<div class="next" class:soon={notice} style:--c={zoneVar(next.kind)}>
  <small>Next{notice ? ` · in ${soonIn < 60 ? `${soonIn} s` : clock(soonIn)}` : ""}</small>
  <b>{zoneName(next.kind)}</b>
  <span>{shownLabel(next.shown, next.target_rpm)} · {positionLabel(next.position)}</span>
</div>

<style>
  .next {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 12px 14px;
    border: 2px dashed var(--c);
    border-radius: var(--r-md);
    transition: background 0.3s;
  }
  .next.soon {
    border-style: solid;
    background: color-mix(in srgb, var(--c) 14%, transparent);
  }
  small {
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.1em;
    font-weight: 700;
    font-size: 12px;
  }
  b {
    color: var(--c);
    font-size: 22px;
    font-weight: 800;
  }
  span {
    font-size: 14px;
  }
</style>
