<script lang="ts">
  // "HomeFi 1.2.0 is available · What's new · Update now · Later": a small card
  // above the player bar. The music is never stopped without asking.
  import { toast } from "../lib/motion";
  import { noteBlocks } from "../lib/notes";
  import { app } from "../lib/state.svelte";
  import { manualReasonText, updates } from "../lib/update.svelte";
  import Icon from "./Icon.svelte";
  import UpdateNotes from "./UpdateNotes.svelte";

  const v = $derived(updates.view!);
  const playing = $derived(app.player?.status === "playing");
  const blocks = $derived(noteBlocks(v?.notes));
  const percent = $derived(v?.total ? Math.min(100, Math.round((v.downloaded / v.total) * 100)) : null);
  const mb = (n: number) => `${(n / 1_048_576).toFixed(n >= 10_485_760 ? 0 : 1)} MB`;
</script>

<div class="update" role="status" transition:toast|global={{ x: -40 }}>
  <div class="head">
    <span class="mark"><Icon name={v.status === "ready" ? "refresh" : "download"} size={16} /></span>
    <div class="grow">
      {#if v.status === "downloading"}
        <b>Downloading HomeFi {v.version}…</b>
        <small class="muted">{percent !== null ? `${percent}% · ${mb(v.downloaded)} of ${mb(v.total ?? 0)}` : mb(v.downloaded)}</small>
      {:else if v.status === "ready"}
        <b>HomeFi {v.version} is ready</b>
        <small class="muted">{v.afterSong ? "Restarting when this song ends" : "Restart to finish"}</small>
      {:else if v.status === "installing"}
        <b>Installing HomeFi {v.version}…</b>
        <small class="muted">HomeFi opens again in a moment</small>
      {:else if v.status === "error"}
        <b>HomeFi {v.version} couldn't be installed</b>
        <small class="muted">{v.error}</small>
      {:else}
        <b>HomeFi {v.version} is available{v.prerelease ? " (beta)" : ""}</b>
        <small class="muted">You have {v.current}{v.inPlace ? "" : " · download it from the release page"}</small>
      {/if}
    </div>
    <button class="icon-btn" title="Later" aria-label="Later" onclick={() => updates.later()}><Icon name="x" size={16} /></button>
  </div>

  {#if v.status === "downloading"}
    <div class="bar" role="progressbar" aria-label="Download progress" aria-valuenow={percent ?? undefined} aria-valuemin="0" aria-valuemax="100">
      <span class:indeterminate={percent === null} style:width={percent === null ? undefined : `${percent}%`}></span>
    </div>
  {/if}

  {#if updates.notesOpen && blocks.length}
    <div class="notes"><UpdateNotes {blocks} /></div>
  {/if}
  {#if !v.inPlace && v.status === "available" && v.manualReason !== "dev"}
    <p class="why muted">{manualReasonText(v.manualReason)}</p>
  {/if}

  <div class="actions">
    {#if blocks.length && v.status !== "installing"}
      <button class="link-btn" onclick={() => (updates.notesOpen = !updates.notesOpen)}>{updates.notesOpen ? "Hide what's new" : "What's new"}</button>
    {:else}
      <button class="link-btn" onclick={() => updates.openPage()}>Release page</button>
    {/if}
    <span class="grow"></span>
    {#if v.status === "available" || v.status === "error"}
      <button class="btn tonal small-btn" onclick={() => updates.later()}>Later</button>
      {#if v.inPlace}
        <button class="btn primary small-btn" onclick={() => updates.download()}>{v.status === "error" ? "Try again" : "Update now"}</button>
      {:else}
        <button class="btn primary small-btn" onclick={() => updates.openPage()}><Icon name="external" size={14} /> Download</button>
      {/if}
    {:else if v.status === "ready"}
      {#if playing && !v.afterSong}
        <button class="btn tonal small-btn" onclick={() => updates.restart("song_end")}>When the song ends</button>
      {:else if v.afterSong}
        <button class="btn tonal small-btn" onclick={() => updates.later()}>Not now</button>
      {:else}
        <button class="btn tonal small-btn" onclick={() => updates.later()}>Later</button>
      {/if}
      <button class="btn primary small-btn" onclick={() => updates.restart("now")}>Restart now</button>
    {/if}
  </div>
</div>

<style>
  .update {
    position: fixed;
    left: 92px;
    bottom: 114px;
    z-index: 59;
    width: min(400px, calc(100vw - 120px));
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    border-radius: min(16px, var(--r-max));
    background: color-mix(in srgb, var(--surface) 92%, transparent);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
    backdrop-filter: blur(18px) saturate(1.3);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .mark {
    display: grid;
    place-items: center;
    flex: none;
    width: 32px;
    height: 32px;
    border-radius: min(10px, var(--r-max));
    background: var(--gradient);
    color: #fff;
  }
  .grow {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  small {
    font-size: 12px;
  }
  .bar {
    position: relative;
    height: 4px;
    border-radius: 2px;
    overflow: hidden;
    background: color-mix(in srgb, var(--accent-text) 15%, transparent);
  }
  .bar span {
    position: absolute;
    inset: 0 auto 0 0;
    background: var(--primary-strong, var(--primary));
    transition: width 0.2s;
  }
  .bar span.indeterminate {
    width: 35%;
    animation: slide 1.2s ease-in-out infinite;
  }
  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(300%);
    }
  }
  .notes {
    max-height: 220px;
    overflow-y: auto;
    padding: 8px 10px;
    border-radius: min(10px, var(--r-max));
    background: var(--surface-2);
  }
  .why {
    margin: 0;
    font-size: 12px;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-direction: row;
  }
  .actions .grow {
    flex-direction: row;
  }
  .small-btn {
    height: 30px;
    padding: 0 12px;
    font-size: 13px;
  }
  .link-btn {
    padding: 2px 0;
    font-size: 12.5px;
    color: var(--primary);
  }
  .link-btn:hover {
    text-decoration: underline;
  }
</style>
