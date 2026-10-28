<script lang="ts">
  import { coverUrl } from "../lib/api";
  import Icon from "./Icon.svelte";

  let {
    path = null,
    url = null,
    size = 48,
    radius = 8,
    icon = "music",
  }: { path?: string | null; url?: string | null; size?: number | string; radius?: number; icon?: string } = $props();

  let failed = $state(false);
  // the art fades in over the placeholder once it has loaded (not when it was already there)
  let loaded = $state(false);
  let img = $state<HTMLImageElement>();
  const src = $derived(url ?? (path ? coverUrl(path) : null));
  $effect(() => {
    src;
    failed = false;
    loaded = !!img?.complete && img.naturalWidth > 0;
  });
  const dim = $derived(typeof size === "number" ? `${size}px` : size);
</script>

<div class="cover" style:width={dim} style:height={typeof size === "number" ? dim : undefined} style:border-radius={radius >= 999 ? `${radius}px` : `min(${radius}px, var(--r-cover, 999px))`}>
  <span class="placeholder"><Icon name={icon} size={typeof size === "number" ? Math.max(14, size * 0.38) : 48} /></span>
  {#if src && !failed}
    <img {src} alt="" loading="lazy" decoding="async" draggable="false" bind:this={img} class:loaded onload={() => (loaded = true)} onerror={() => (failed = true)} />
  {/if}
</div>

<style>
  .cover {
    position: relative;
    flex: none;
    overflow: hidden;
    background: var(--surface-3);
    aspect-ratio: 1;
  }
  .placeholder {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: var(--faint);
  }
  img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    opacity: 0;
    transition: opacity calc(0.35s * var(--motion-speed)) ease-out;
  }
  img.loaded {
    opacity: 1;
  }
</style>
