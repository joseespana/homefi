<script lang="ts">
  import { pop } from "../lib/motion";
  import { api, khz, outputIcon, type OutputDevice } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import Icon from "./Icon.svelte";

  let { onclose }: { onclose: () => void } = $props();
  let devices = $state<OutputDevice[]>([]);

  $effect(() => {
    api.outputDevices().then((d) => (devices = d));
  });

  const preferred = $derived(app.player?.preferred_device ?? null);
  const stream = $derived(app.player?.stream ?? null);

  function choose(name: string | null) {
    api.setOutput(name);
    onclose();
  }
</script>

<div class="menu" role="menu" transition:pop={{ origin: "bottom right", y: 8 }}>
  <div class="title">Audio output</div>
  <button class="item" class:selected={preferred === null} role="menuitemradio" aria-checked={preferred === null} onclick={() => choose(null)}>
    <Icon name="speaker" />
    <div class="grow">
      <div>System default</div>
      <div class="sub">Follows the output selected in your OS</div>
    </div>
    {#if preferred === null}<Icon name="check" />{/if}
  </button>
  {#each devices as d, i (d.name + "#" + i)}
    <button class="item" class:selected={preferred === d.name} role="menuitemradio" aria-checked={preferred === d.name} onclick={() => choose(d.name)}>
      <Icon name={outputIcon(d.name)} />
      <div class="grow">
        <div class="ellipsis">{d.name}{d.is_default ? " · default" : ""}</div>
        <div class="sub">{d.sample_rates.length ? `${d.sample_rates.map(khz).join(" / ")} kHz` : "any rate"} · {d.max_channels} ch</div>
      </div>
      {#if preferred === d.name}<Icon name="check" />{/if}
    </button>
  {/each}
  {#if stream}
    <div class="now">
      Now: <b>{stream.codec.toUpperCase()}</b>
      {stream.bits_per_sample ? `${stream.bits_per_sample}-bit / ` : ""}{khz(stream.source_rate)} kHz →
      <b>{khz(stream.output_rate)} kHz</b> on {stream.device}
      {#if stream.resampled}
        <div class="warn">Resampled: this output can't play {khz(stream.source_rate)} kHz.</div>
      {:else}
        <div class="ok">Native rate, no resampling.</div>
      {/if}
      {#if app.player?.eq_active}
        <div class="warn">Equalizer on (“{app.player.eq_profile}”): processed, not bit-perfect.</div>
      {:else if !stream.resampled}
        <div class="ok">Bit-perfect.</div>
      {/if}
    </div>
  {/if}
  <button class="item eq" role="menuitem" onclick={() => ((app.view = { name: "eq", output: stream?.device ?? preferred ?? undefined }), onclose())}>
    <Icon name="eq" />
    <div class="grow">
      <div>Equalizer…</div>
      <div class="sub">{app.settings?.eq.enabled ? "On" : "Off"} · a profile for each output</div>
    </div>
  </button>
</div>

<style>
  .menu {
    /* an ordinary surface, whatever colours the skin gives the deck */
    --text: var(--window-text);
    --muted: var(--window-muted);
    --surface: var(--window-surface);
    --surface-2: var(--window-surface-2);
    --border: var(--window-border);
    --accent-text: var(--window-accent-text);
    color: var(--text);
    position: absolute;
    right: 0;
    bottom: calc(100% + 12px);
    width: 360px;
    max-height: 70vh;
    overflow-y: auto;
    padding: 8px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: min(14px, var(--r-max));
    box-shadow: var(--shadow);
    z-index: 20;
  }
  .title {
    padding: 8px 10px;
    font-weight: 700;
  }
  .item {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px;
    border-radius: min(10px, var(--r-max));
    text-align: left;
    color: var(--muted);
  }
  .item:hover {
    background: var(--surface-2);
  }
  .item.selected {
    color: var(--primary);
  }
  .item.selected .grow > div:first-child {
    font-weight: 600;
  }
  .grow {
    flex: 1;
    min-width: 0;
    color: var(--text);
  }
  .sub {
    font-size: 12px;
    color: var(--muted);
  }
  .now {
    margin: 6px 4px 4px;
    padding: 10px;
    border-radius: min(10px, var(--r-max));
    background: var(--surface-2);
    font-size: 12.5px;
    color: var(--muted);
  }
  .now b {
    color: var(--text);
  }
  .warn {
    margin-top: 4px;
    color: #fbbf24;
  }
  .ok {
    margin-top: 4px;
    color: var(--ok);
  }
</style>
