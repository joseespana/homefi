<script lang="ts">
  // "Where to watch": no TV, a window on another screen, or HomeFi's TV page in a TV's browser / the
  // Android TV app (a link and a QR), and where the music plays. The page itself is one renderer
  // (homefi-core's ride.html) whichever way it is opened.
  import { EX_TV_DISPLAY_AUTO, type TvAudio } from "../../lib/exercise-api";
  import { exercise, type TvMode } from "../../lib/exercise.svelte";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";
  import QrCode from "../share/QrCode.svelte";
  import ExCard from "./ExCard.svelte";
  import ExSegmented from "./ExSegmented.svelte";

  const MODES: { id: TvMode; label: string }[] = [
    { id: "none", label: "No TV" },
    { id: "window", label: "TV window" },
    { id: "app", label: "TV app / browser" },
  ];
  const AUDIO: { id: TvAudio; label: string }[] = [
    { id: "phone", label: "Here (headphones)" },
    { id: "tv", label: "On the TV" },
  ];
  const QR_PX = 112;

  const s = $derived(exercise.settings);
  const monitors = $derived(exercise.monitors);
  /** The screen the window will open on, as the setting and the screens say. */
  const target = $derived.by(() => {
    if (s.tv_window_display !== EX_TV_DISPLAY_AUTO) return monitors.find((m) => m.name === s.tv_window_display) ?? null;
    return monitors.find((m) => !m.primary) ?? monitors[0] ?? null;
  });
  const targetName = $derived(target ? (target.name ?? (target.primary ? "the main display" : "the second display")) : "the second display");

  $effect(() => {
    if (exercise.tvMode === "window" && !monitors.length) exercise.refreshMonitors();
  });

  async function copy() {
    if (!exercise.tv.url) return;
    try {
      await navigator.clipboard.writeText(exercise.tv.url);
      app.note("Link copied", "success", "exercise-copy");
    } catch {
      app.note("Could not copy the link", "error", "exercise-copy");
    }
  }
</script>

<ExCard title="Where to watch">
  <div class="stack">
    <ExSegmented label="Where to watch" options={MODES} value={exercise.tvMode} onchange={(m) => exercise.setTvMode(m)} />
    {#if exercise.tvMode !== "none"}
      <div class="line">
        <span class="lbl" id="ex-audio">Music plays</span>
        <ExSegmented label="Music plays" options={AUDIO} value={s.tv_audio} onchange={(tv_audio) => exercise.saveSettings({ tv_audio })} />
      </div>
    {/if}
    {#if exercise.tvMode === "window"}
      <p class="help">Opens on {targetName}, {s.tv_window_fullscreen ? "fullscreen" : "in a window"}.</p>
      {#if monitors.length > 1}
        <label class="pick">
          <span class="lbl">Display</span>
          <select value={s.tv_window_display} onchange={(e) => exercise.saveSettings({ tv_window_display: e.currentTarget.value })}>
            <option value={EX_TV_DISPLAY_AUTO}>Automatic</option>
            {#each monitors as m}<option value={m.name ?? ""}>{m.name ?? "Display"} · {m.width} × {m.height}{m.primary ? " (main)" : ""}</option>{/each}
          </select>
        </label>
      {/if}
    {:else if exercise.tvMode === "app"}
      <div class="link">
        {#if exercise.tv.url}
          <QrCode text={exercise.tv.url} size={QR_PX} />
          <div class="url">
            <span class="addr">{exercise.tv.url}</span>
            <button type="button" class="btn tonal" onclick={copy}><Icon name="copy" size={16} />Copy link</button>
          </div>
        {:else}
          <p class="help">Starting the TV page…</p>
        {/if}
      </div>
      <p class="help">Open this on the TV's browser or in HomeFi for Android TV.</p>
    {/if}
  </div>
</ExCard>

<style>
  .stack {
    display: flex;
    flex-direction: column;
    gap: 12px;
    align-items: flex-start;
  }
  .line {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .lbl {
    font-size: 13px;
    font-weight: 600;
    color: var(--muted);
  }
  .help {
    margin: 0;
    font-size: 13px;
    color: var(--muted);
    line-height: 1.4;
  }
  .pick {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-width: 100%;
  }
  select {
    min-height: 44px;
    max-width: 100%;
  }
  .link {
    display: flex;
    align-items: center;
    gap: 14px;
    min-width: 0;
  }
  .url {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
    align-items: flex-start;
  }
  .addr {
    font-family: var(--font-mono);
    font-size: 12px;
    user-select: text;
    overflow-wrap: anywhere;
  }
  .btn {
    height: 40px;
  }
</style>
