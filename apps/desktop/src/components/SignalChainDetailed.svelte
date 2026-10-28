<script lang="ts">
  import { khz, outputIcon, type StreamInfo } from "../lib/api";
  import Icon from "./Icon.svelte";

  // `eq`: the equalizer profile shaping the sound (not bit-perfect), if any;
  // `taste`: the taste layer on top; `loudness`: the normalization gain (dB)
  let { stream, eq = null, taste = null, loudness = 0 }: { stream: StreamInfo | null; eq?: string | null; taste?: string | null; loudness?: number } = $props();
  const gain = $derived(Math.abs(loudness) >= 0.05 ? loudness : 0);
  const processed = $derived(!!eq || gain !== 0);
  const what = $derived([eq ? "equalizer" : "", gain ? "loudness" : ""].filter(Boolean).join(" and "));
</script>

{#if stream}
  <div class="chain" aria-label="Signal path">
    <div class="node">
      <span class="cap">Source</span>
      <b>{stream.codec.toUpperCase()}</b>
      <span>{stream.bits_per_sample ? `${stream.bits_per_sample}-bit · ` : ""}{khz(stream.source_rate)} kHz · {stream.source_channels}ch</span>
    </div>
    {#if stream.resampled}
      <span class="wire"><i></i></span>
      <div class="node warn">
        <span class="cap">Resample</span>
        <b>{khz(stream.source_rate)} → {khz(stream.output_rate)}</b>
        <span>sinc, 256 taps</span>
      </div>
    {/if}
    {#if gain}
      <span class="wire"><i></i></span>
      <div class="node warn">
        <span class="cap">Loudness</span>
        <b>{gain > 0 ? "+" : "−"}{Math.abs(gain).toFixed(1)} dB</b>
        <span>ReplayGain</span>
      </div>
    {/if}
    {#if eq}
      <span class="wire"><i></i></span>
      <div class="node warn eq">
        <span class="cap">Equalizer</span>
        <b class="ellipsis">{eq}</b>
        <span class="ellipsis">{taste ? `+ ${taste}` : "64-bit float DSP"}</span>
      </div>
    {/if}
    <span class="wire"><i></i></span>
    <div class="node out">
      <span class="cap">Output</span>
      <b class="dev"><Icon name={outputIcon(stream.device)} size={14} /> <span class="ellipsis">{stream.device}</span></b>
      <span>{khz(stream.output_rate)} kHz · {stream.output_channels}ch</span>
    </div>
  </div>
  <div class="verdict" class:warn={stream.resampled || processed}>
    <span class="led"></span>
    {#if processed}
      {what[0].toUpperCase() + what.slice(1)} on: the sound is processed, not bit-perfect{stream.resampled ? ` · resampled to ${khz(stream.output_rate)} kHz` : ""}
    {:else}
      {stream.resampled ? `Resampled: this output can't run at ${khz(stream.source_rate)} kHz` : "Bit-perfect at the native rate: the device runs at the file's sample rate"}
    {/if}
  </div>
{/if}

<style>
  .chain {
    display: flex;
    align-items: stretch;
    gap: 0;
    font-family: var(--font-mono);
  }
  .node {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px 12px;
    border-radius: min(12px, var(--r-max));
    background: var(--glass-strong);
    border: 1px solid var(--glass-border);
    font-size: 11.5px;
    color: var(--muted);
  }
  .node b {
    color: var(--text);
    font-size: 14px;
  }
  .node.warn b {
    color: #fbbf24;
  }
  .node.out {
    border-color: color-mix(in srgb, var(--room-accent) 55%, transparent);
  }
  .cap {
    font-size: 9.5px;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--faint);
  }
  .dev {
    display: flex;
    align-items: center;
    gap: 5px;
    min-width: 0;
  }
  .wire {
    width: 22px;
    align-self: center;
    height: 2px;
    background: var(--glass-border);
    position: relative;
    overflow: hidden;
  }
  .wire i {
    position: absolute;
    inset: 0;
    width: 40%;
    background: var(--room-accent);
    animation: flow 1.1s linear infinite;
  }
  @keyframes flow {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(260%);
    }
  }
  .verdict {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 10px;
    font-size: 12px;
    color: var(--muted);
  }
  .led {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--ok);
    box-shadow: 0 0 8px var(--ok);
  }
  .verdict.warn .led {
    background: #fbbf24;
    box-shadow: 0 0 8px #fbbf24;
  }
</style>
