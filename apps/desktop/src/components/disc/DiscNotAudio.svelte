<script lang="ts">
  // Why there is nothing to rip: no drive, no disc, a data disc, a copy-protected one. Plain
  // words and what to do next; HomeFi never works around copy protection.
  import type { DriveView } from "../../lib/api";
  import { disc } from "../../lib/disc.svelte";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";

  let { drive }: { drive: DriveView | null } = $props();

  const PROTECTION: Record<NonNullable<DriveView["protection"]>, string> = { css: "CSS", cppm: "CPPM", aacs: "AACS" };
  const kind = $derived(!drive ? "nodrive" : drive.media);
  const protection = $derived(drive?.protection ? PROTECTION[drive.protection] : null);
</script>

<section class="panel" data-kind={kind} aria-live="polite">
  <div class="art" aria-hidden="true"><Icon name={kind === "protected" ? "shield" : kind === "data" ? "folder" : "cd"} size={34} /></div>
  <div class="text">
    {#if kind === "nodrive"}
      <h2>No disc drive found</h2>
      <p>Plug in a USB drive and HomeFi will notice it.</p>
    {:else if kind === "none"}
      <h2>Insert an audio CD</h2>
      <p>HomeFi will tell you when the disc is ready in {drive?.name}.</p>
    {:else if kind === "protected"}
      <h2>This disc is copy-protected{protection ? ` (${protection})` : ""}</h2>
      <p>HomeFi doesn't bypass copy protection, so it can't copy this disc. Audio CDs, and discs with your own files, work fine.</p>
    {:else if kind === "data"}
      <h2>This is a data disc</h2>
      <p>Open its folder to play the files it contains.</p>
    {:else}
      <h2>HomeFi can't tell what this disc is</h2>
      <p>It may be blank, damaged or a kind HomeFi doesn't read. Try cleaning it, or eject it and insert it again.</p>
    {/if}
    <div class="actions">
      {#if kind === "data" && drive?.mount}
        <button type="button" class="btn primary" onclick={() => (app.view = { name: "folder", path: drive.mount! })}><Icon name="folder" size={16} /> Open folder</button>
      {/if}
      {#if drive && kind !== "none"}
        <button type="button" class="btn tonal" onclick={() => disc.eject()}><Icon name="eject" size={16} /> Eject</button>
      {/if}
      <button type="button" class="btn tonal" onclick={() => disc.enter()}><Icon name="refresh" size={16} /> Look again</button>
    </div>
  </div>
</section>

<style>
  .panel {
    display: flex;
    gap: 20px;
    align-items: flex-start;
    max-width: 640px;
    margin-top: 24px;
    padding: 24px;
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    background: var(--surface);
  }
  .art {
    display: grid;
    place-items: center;
    flex: none;
    width: 64px;
    height: 64px;
    border-radius: 50%;
    background: var(--surface-3);
    color: var(--muted);
  }
  [data-kind="protected"] .art {
    background: var(--warn-bg);
    color: var(--warn);
  }
  h2 {
    margin: 0 0 6px;
    font-size: 18px;
  }
  p {
    margin: 0;
    max-width: 52ch;
    line-height: 1.5;
    color: var(--muted);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 16px;
  }
</style>
