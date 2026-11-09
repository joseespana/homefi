<script lang="ts">
  // A saved ride's mountain in miniature: its elevation profile on the shared course drawing (the parts are
  // not stored in the list, so the ridge has no zone colours), so a rider can tell a flat ride from a steep
  // one at a glance.
  import type { ProfilePoint } from "../../lib/exercise-api";
  import Mountain from "./Mountain.svelte";

  let { profile, width = 112, height = 44, label = "" }: { profile: readonly ProfilePoint[]; width?: number; height?: number; label?: string } = $props();
</script>

<span class="mini" style:width="{width}px" style:height="{height}px" aria-hidden={label ? undefined : "true"}>
  <Mountain parts={[]} {profile} mode="summary" {height} {label} summits={false} />
</span>

<style>
  .mini {
    display: block;
    flex: none;
  }
</style>
