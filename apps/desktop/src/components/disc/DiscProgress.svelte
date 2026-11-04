<script lang="ts">
  // A rip under way (or just finished): one row per song with what it is doing, how many
  // re-reads it needed and whether the copy can be trusted. The verdict comes from the reading
  // itself: parts that came out the same on every re-read are good; doubtful or unreadable
  // parts are counted, never hidden.
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { formatTime } from "../../lib/api";
  import { disc, formatLabel, trackMs, type RowProgress } from "../../lib/disc.svelte";
  import { item, reflow } from "../../lib/motion";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";

  const PHASE_TEXT: Record<RowProgress["phase"], string> = { waiting: "Waiting", reading: "Reading", encoding: "Saving", verifying: "Checking", done: "Done", failed: "Failed" };
  const rows = $derived((disc.current?.toc.tracks ?? []).filter((t) => disc.rows[t.number]));
  const finished = $derived(rows.filter((t) => disc.rows[t.number].phase === "done").length);
  const failed = $derived(rows.filter((t) => disc.rows[t.number].phase === "failed").length);
  const active = $derived(rows.find((t) => ["reading", "encoding", "verifying"].includes(disc.rows[t.number].phase)));
  const overall = $derived(rows.length ? (finished + failed + (active ? disc.rows[active.number].fraction : 0)) / rows.length : 0);
  const mode = $derived(disc.settings.mode === "secure" ? "Secure" : "Fast");
  const albumDir = $derived(disc.summary?.albumDir ?? null);

  const summary = $derived(
    disc.ripping
      ? disc.cancelling
        ? "Stopping…"
        : `Ripping song ${Math.min(rows.length, finished + failed + 1)} of ${rows.length} · ${formatLabel(disc.settings.format)} · ${mode} reading`
      : disc.summary
        ? failed
          ? `${finished} of ${rows.length} songs saved, ${failed} failed`
          : `Done: ${finished} ${finished === 1 ? "song" : "songs"} saved`
        : `Stopped: ${finished} of ${rows.length} songs saved`,
  );

  /** What the row says about trust; null while it is still being read. */
  function verdict(r: RowProgress): { text: string; tone: "ok" | "warn" | "bad" } | null {
    if (r.phase !== "done") return null;
    if (r.unreadable > 0) return { text: `${r.unreadable.toLocaleString("en-US")} samples couldn't be read`, tone: "bad" };
    if (r.suspicious > 0) return { text: `${r.suspicious.toLocaleString("en-US")} doubtful samples`, tone: "warn" };
    return { text: disc.settings.mode === "secure" ? "Verified" : "Read once", tone: "ok" };
  }
  const rereads = (n: number) => (n === 1 ? "1 re-read" : `${n} re-reads`);
</script>

<section class="card" aria-labelledby="rip-h">
  <header>
    <div>
      <h2 id="rip-h">{disc.ripping ? "Ripping" : "Rip"}</h2>
      <p class="muted" role="status" aria-live="polite">{summary}</p>
    </div>
    {#if disc.ripping}
      <button type="button" class="btn tonal" onclick={() => disc.cancel()} disabled={disc.cancelling}><Icon name="x" size={16} /> Cancel</button>
    {/if}
  </header>
  <div class="total" role="progressbar" aria-label="Whole disc" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(overall * 100)}><div class="bar" style:width="{overall * 100}%"></div></div>

  <ul>
    {#each rows as t (t.number)}
      {@const r = disc.rows[t.number]}
      {@const v = verdict(r)}
      <li class="row {r.phase}" in:item|global animate:reflow>
        <span class="state" aria-hidden="true">
          {#if r.phase === "done"}<Icon name="check" size={16} />{:else if r.phase === "failed"}<Icon name="alert" size={16} />{:else if r.phase === "waiting"}<span class="num">{String(t.number).padStart(2, "0")}</span>{:else}<span class="spinner"></span>{/if}
        </span>
        <div class="grow">
          <div class="ellipsis">{disc.resolved().titleOf(t.number)}</div>
          {#if r.phase === "failed"}
            <div class="small bad">{r.message ?? "This song couldn't be saved."}</div>
          {:else if r.phase === "done"}
            <div class="small muted">{formatTime(trackMs(t.startLba, t.endLba))}{r.rereads ? ` · ${rereads(r.rereads)}` : ""}</div>
          {:else if r.phase !== "waiting"}
            <div class="small muted">{PHASE_TEXT[r.phase]} {Math.round(r.fraction * 100)}%{r.rereads ? ` · ${rereads(r.rereads)}` : ""}{r.suspicious ? ` · ${r.suspicious.toLocaleString("en-US")} doubtful samples` : ""}</div>
            <div class="track"><div class="fill" style:width="{r.fraction * 100}%"></div></div>
          {:else}
            <div class="small muted">{PHASE_TEXT.waiting}</div>
          {/if}
        </div>
        {#if v}<span class="badge verdict {v.tone}">{v.text}</span>{/if}
      </li>
    {/each}
  </ul>

  {#if !disc.ripping && rows.length}
    <footer>
      {#if finished}<button type="button" class="btn primary" onclick={() => disc.showInLibrary()}><Icon name="music" size={16} /> Show in library</button>{/if}
      {#if albumDir}<button type="button" class="btn tonal" onclick={() => revealItemInDir(albumDir)}><Icon name="folder" size={16} /> Show in folder</button>{/if}
      <button type="button" class="btn tonal" onclick={() => disc.startOver()}><Icon name="refresh" size={16} /> Rip again</button>
      <button type="button" class="btn tonal" onclick={() => disc.eject()}><Icon name="eject" size={16} /> Eject</button>
      {#if disc.summary?.logPath}<span class="muted small">Log saved next to the songs.</span>{/if}
    </footer>
  {/if}
</section>

<style>
  .card {
    padding: 16px 12px 12px;
    border: 1px solid var(--border);
    border-radius: min(16px, var(--r-max));
    background: var(--surface);
  }
  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    padding: 0 8px 10px;
  }
  h2 {
    margin: 0;
    font-size: 16px;
  }
  header p {
    margin: 2px 0 0;
    font-size: 13.5px;
  }
  .total {
    height: 4px;
    margin: 0 8px 10px;
    border-radius: min(4px, var(--r-max));
    background: var(--surface-3);
    overflow: hidden;
  }
  .bar {
    height: 100%;
    background: var(--gradient);
    transition: width 0.3s;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 14px;
    min-height: 52px;
    padding: 8px;
    border-radius: min(10px, var(--r-max));
  }
  .row.waiting {
    opacity: 0.6;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .small {
    font-size: 12.5px;
  }
  .bad {
    color: var(--danger);
  }
  .track {
    height: 4px;
    margin-top: 6px;
    border-radius: min(4px, var(--r-max));
    background: var(--surface-3);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--primary);
    transition: width 0.15s linear;
  }
  .state {
    display: grid;
    place-items: center;
    flex: none;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: var(--surface-3);
  }
  .num {
    color: var(--faint);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
  }
  .done .state {
    color: var(--ok);
  }
  .failed .state {
    color: var(--danger);
  }
  .spinner {
    width: 18px;
    height: 18px;
    border: 3px solid var(--surface-3);
    border-top-color: var(--primary);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .verdict {
    flex: none;
    font-size: 12px;
  }
  .verdict.ok {
    color: var(--ok);
  }
  .verdict.warn {
    color: var(--warn);
  }
  .verdict.bad {
    color: var(--danger);
  }
  footer {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    padding: 14px 8px 4px;
  }
</style>
