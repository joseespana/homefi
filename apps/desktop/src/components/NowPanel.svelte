<script lang="ts">
  // The side player: the song playing, big, in a column beside what you browse. The player bar
  // keeps the transport; this column is for looking at it, its actions and what comes next.
  import { api } from "../lib/api";
  import { drawer } from "../lib/motion";
  import { sidePlayer } from "../lib/sidePlayer.svelte";
  import { app } from "../lib/state.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";
  import NowActions from "./NowActions.svelte";

  /** How many of the next songs the column lists (the queue panel has them all). */
  const NEXT_SHOWN = 4;

  let { onqueue }: { onqueue: () => void } = $props();

  const track = $derived(app.current);
  const queue = $derived(app.player?.queue ?? []);
  const at = $derived(app.player?.current ?? -1);
  const next = $derived(
    queue
      .slice(at + 1, at + 1 + NEXT_SHOWN)
      .map((path, i) => ({ path, index: at + 1 + i, track: app.trackAt(path) })),
  );
  const name = (p: string) => p.split(/[\\/]/).pop()?.replace(/\.[^.]+$/, "") ?? p;
  const quality = $derived(
    track ? [track.format?.toUpperCase(), track.bitDepth && `${track.bitDepth}-bit`, track.sampleRate && `${(track.sampleRate / 1000).toFixed(1).replace(/\.0$/, "")} kHz`].filter(Boolean).join(" · ") : "",
  );
</script>

{#if track}
  <aside class="panel now-panel" aria-label="Now playing" in:drawer>
    <div class="head">
      <button class="icon-btn" onclick={() => sidePlayer.toggle()} aria-label="Hide the side player" title="Hide (the player buttons bring it back)">
        <Icon name="x" />
      </button>
      <b class="grow">Now playing</b>
      <button class="icon-btn" onclick={() => sidePlayer.maximize()} aria-label="Open the full player" title="Full player">
        <Icon name="expand" />
      </button>
    </div>

    <button class="art" onclick={() => sidePlayer.maximize()} title="Open the full player" aria-label="Open the full player">
      <Cover path={track.path} size="100%" radius={14} />
    </button>

    <div class="info">
      <div class="title ellipsis" title={track.title}>{track.title}</div>
      <div class="sub ellipsis">{track.artist}{track.album ? ` · ${track.album}` : ""}</div>
      {#if quality}<div class="quality muted">{quality}</div>{/if}
    </div>
    <NowActions />

    {#if next.length || app.upcoming.length}
      <div class="next-head">
        <b>Next up</b>
        <button class="link" onclick={onqueue}>Open queue</button>
      </div>
      {#each next as n (n.index + n.path)}
        <button class="row" onclick={() => api.jump(n.index)} title="Play">
          <Cover path={n.path} size={40} radius={6} />
          <span class="grow">
            <span class="ellipsis row-title">{n.track?.title ?? name(n.path)}</span>
            <span class="ellipsis sub">{n.track?.artist ?? ""}</span>
          </span>
        </button>
      {:else}
        {#each app.upcoming.slice(0, NEXT_SHOWN) as u, j (u.id + j)}
          <button class="row" onclick={() => app.playUpcoming(j)} title="Play now">
            <Cover size={40} radius={6} icon="music" />
            <span class="grow">
              <span class="ellipsis row-title">{u.title}</span>
              <span class="ellipsis sub">{u.artist}</span>
            </span>
          </button>
        {/each}
      {/each}
    {/if}
  </aside>
{/if}

<style>
  .panel {
    min-width: 0;
    border-left: 1px solid var(--border);
    background: var(--sidebar);
    overflow-y: auto;
    padding: 0 16px 16px;
    scrollbar-gutter: stable;
  }
  .head {
    position: sticky;
    top: 0;
    z-index: 2;
    display: flex;
    gap: 6px;
    align-items: center;
    padding: 34px 0 10px;
    background: var(--sidebar);
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .art {
    display: block;
    width: 100%;
    aspect-ratio: 1;
    padding: 0;
    border: 0;
    background: none;
    cursor: zoom-in;
    border-radius: 14px;
    box-shadow: 0 18px 40px rgb(0 0 0 / 0.35);
  }
  .info {
    margin: 16px 0 8px;
  }
  .title {
    font-size: 1.25rem;
    font-weight: 700;
  }
  .sub {
    color: var(--muted);
    font-size: 0.9rem;
  }
  .quality {
    margin-top: 4px;
    font-size: 0.78rem;
  }
  .next-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    margin: 22px 0 6px;
  }
  .link {
    border: 0;
    background: none;
    color: var(--muted);
    font-size: 0.82rem;
    cursor: pointer;
  }
  .link:hover {
    color: var(--text);
    text-decoration: underline;
  }
  .row {
    display: flex;
    width: 100%;
    gap: 10px;
    align-items: center;
    padding: 6px;
    border: 0;
    border-radius: var(--r-sm);
    background: none;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .row:hover {
    background: var(--surface-2);
  }
  .row .grow {
    display: flex;
    flex-direction: column;
  }
  .row-title {
    font-size: 0.9rem;
  }
</style>
