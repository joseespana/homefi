<script lang="ts">
  // Placeholder shaped like the content that's loading, with a slow sheen
  // sweeping across it (the same light as the playing row).
  let {
    kind = "rows",
    count = 8,
    round = false,
  }: { kind?: "rows" | "grid" | "detail"; count?: number; round?: boolean } = $props();
</script>

<div class="skeleton {kind}" aria-busy="true" aria-label="Loading">
  {#if kind === "detail"}
    <div class="head">
      <div class="block cover" class:round></div>
      <div class="lines">
        <div class="block line" style="width: 40%"></div>
        <div class="block line big" style="width: 60%"></div>
        <div class="block line" style="width: 30%"></div>
      </div>
    </div>
    {#each { length: count } as _, i (i)}
      <div class="row">
        <div class="block thumb"></div>
        <div class="lines">
          <div class="block line" style="width: {45 + ((i * 17) % 35)}%"></div>
          <div class="block line small" style="width: {25 + ((i * 11) % 25)}%"></div>
        </div>
      </div>
    {/each}
  {:else if kind === "grid"}
    {#each { length: count } as _, i (i)}
      <div class="card">
        <div class="block art" class:round></div>
        <div class="block line" style="width: {60 + ((i * 13) % 30)}%"></div>
        <div class="block line small" style="width: {35 + ((i * 7) % 30)}%"></div>
      </div>
    {/each}
  {:else}
    {#each { length: count } as _, i (i)}
      <div class="row">
        <div class="block thumb" class:round></div>
        <div class="lines">
          <div class="block line" style="width: {45 + ((i * 17) % 35)}%"></div>
          <div class="block line small" style="width: {25 + ((i * 11) % 25)}%"></div>
        </div>
        <div class="block line tiny"></div>
      </div>
    {/each}
  {/if}
</div>

<style>
  .skeleton {
    --sk: color-mix(in srgb, var(--text) 7%, transparent);
    --sk-shine: color-mix(in srgb, var(--text) 14%, transparent);
  }
  .block {
    border-radius: min(8px, var(--r-max));
    background: linear-gradient(100deg, var(--sk) 40%, var(--sk-shine) 50%, var(--sk) 60%) 0 0 / 300% 100%;
    animation: sk-shine 1.6s ease-in-out infinite;
  }
  @keyframes sk-shine {
    from {
      background-position: 100% 0;
    }
    to {
      background-position: -100% 0;
    }
  }
  .round {
    border-radius: 50% !important;
  }
  .rows,
  .detail {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 8px;
  }
  .thumb {
    width: 48px;
    height: 48px;
    flex: none;
  }
  .lines {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .line {
    height: 12px;
  }
  .line.big {
    height: 28px;
  }
  .line.small {
    height: 10px;
  }
  .line.tiny {
    width: 36px;
    height: 10px;
    flex: none;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(168px, 1fr));
    gap: 22px 18px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .art {
    aspect-ratio: 1;
    border-radius: min(12px, var(--r-max));
    margin-bottom: 4px;
  }
  .head {
    display: flex;
    gap: 24px;
    align-items: flex-end;
    margin-bottom: 18px;
  }
  .cover {
    width: 200px;
    height: 200px;
    flex: none;
    border-radius: min(16px, var(--r-max));
  }
</style>
