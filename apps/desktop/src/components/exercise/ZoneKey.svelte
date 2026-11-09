<script lang="ts">
  // The small key of the course colours (cool = easy, green = steady, amber = climbs, purple = jumps, red = all out).
  import { ZONE_ORDER, zoneName, zoneVar, type SegmentKind } from "../../lib/exercise-api";

  let { kinds = ZONE_ORDER }: { kinds?: readonly SegmentKind[] } = $props();
  const shown = $derived(ZONE_ORDER.filter((k) => kinds.includes(k)));
</script>

<ul class="key" aria-label="Colours of the course">
  {#each shown as k}<li><i style:background={zoneVar(k)}></i>{zoneName(k)}</li>{/each}
</ul>

<style>
  .key {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 6px 14px;
    font-size: 12px;
    color: var(--muted);
  }
  li {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  i {
    width: 10px;
    height: 10px;
    border-radius: 50%;
  }
</style>
