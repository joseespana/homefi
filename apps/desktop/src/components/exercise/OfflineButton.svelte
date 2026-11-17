<script lang="ts">
  // "Download for offline" of one ride (the plan on the screen, or a ride in My rides): queues its TIDAL songs
  // that have no file here, under the ride's name, in the normal download queue. While they come it turns into a
  // spinner with "Downloading 3 of 12" (it can't be pressed twice); once every song has a file it says
  // "Ready offline". The count is the core's (homefi_core::playlist::LocalCopies), the per-song progress the queue's.
  import type { OfflineStatus } from "../../lib/exercise-api";
  import { exercise } from "../../lib/exercise.svelte";
  import Icon from "../Icon.svelte";
  import Spinner from "../Spinner.svelte";

  let { name, status }: { name: string; status: OfflineStatus | null | undefined } = $props();

  const view = $derived(exercise.offlineView(status));
  const missingText = (count: number) => `${count} ${count === 1 ? "song still needs" : "songs still need"} the internet`;
</script>

{#if view.kind === "ready"}
  <span class="ready" title="Every song of this ride is on this computer: you can ride it with no internet."><Icon name="check" size={16} />Ready offline</span>
{:else if view.kind === "missing"}
  <button type="button" class="btn tonal" title="{missingText(view.count)}. Download them to ride with no internet." aria-label="Download for offline: {missingText(view.count)}" onclick={() => exercise.downloadOffline(name, status)}><Icon name="download" size={16} />Download for offline</button>
{:else if view.kind === "downloading"}
  <button type="button" class="btn tonal" disabled aria-busy="true"><Spinner size={16} />{#if view.started}Downloading {Math.min(view.done + 1, view.of)} of {view.of}{:else}Waiting to download…{/if}</button>
{/if}

<style>
  .btn {
    height: 36px;
    padding: 0 14px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex: none;
  }
  .ready {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 36px;
    padding: 0 6px;
    flex: none;
    color: var(--ok);
    font-size: 13px;
    font-weight: 700;
  }
</style>
