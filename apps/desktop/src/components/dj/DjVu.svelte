<script lang="ts">
  // A level meter: green to 60 %, amber to 85 %, red above (the colours sit at fixed levels; a
  // cover hides what is unlit). Sized by CSS from the 20 Hz snapshot; no state of its own.
  let { level, horizontal = false, label }: { level: number; horizontal?: boolean; label: string } = $props();
  const pct = $derived(Math.round(Math.min(1, Math.max(0, level)) * 100));
</script>

<span class="vu" class:h={horizontal} role="meter" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={pct}>
  <i style={horizontal ? `width:${100 - pct}%` : `height:${100 - pct}%`}></i>
</span>

<style>
  .vu {
    position: relative;
    display: block;
    width: 12px;
    align-self: stretch;
    margin: 14px 0;
    border-radius: var(--r-xs);
    background: linear-gradient(0deg, var(--ok) 0 60%, var(--warn) 60% 85%, var(--danger) 85%);
    box-shadow: inset 0 0 0 1px var(--border);
    overflow: hidden;
  }
  .vu.h {
    width: 84px;
    height: 10px;
    margin: 0;
    align-self: center;
    background: linear-gradient(90deg, var(--ok) 0 60%, var(--warn) 60% 85%, var(--danger) 85%);
  }
  i {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    background: var(--dj-well);
    transition: height 0.05s linear, width 0.05s linear;
  }
  .h i {
    left: auto;
    bottom: 0;
  }
  :global(:root[data-motion="off"]) i {
    transition: none;
  }
</style>
