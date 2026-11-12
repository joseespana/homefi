<script lang="ts">
  // How hard: ten bars, the lit ones in the zone colour, and always the word and the number ("Hard 7").
  import { EFFORT_STEPS, effortHint, effortWord } from "../../lib/exercise-api";

  let { effort, level = null, levelCount = 0, hint = true }: { effort: number; level?: number | null; levelCount?: number; hint?: boolean } = $props();
</script>

<div class="effort">
  <span class="lbl">Effort</span>
  <div class="bars" role="img" aria-label="Effort {effort} of {EFFORT_STEPS}, {effortWord(effort)}{level ? `, level ${level} of ${levelCount}` : ''}">
    {#each { length: EFFORT_STEPS } as _, i}<i class:on={i < effort}></i>{/each}
  </div>
  <div class="word">{effortWord(effort)} <span class="n">{effort}</span>{#if level && levelCount}<span class="lv">· level {level}/{levelCount}</span>{/if}</div>
  {#if hint}<p class="hint">{effortHint(effort)}</p>{/if}
</div>

<style>
  .effort {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }
  .lbl {
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .bars {
    display: flex;
    gap: 6px;
  }
  i {
    width: 22px;
    height: 34px;
    border-radius: 5px;
    background: var(--track);
    transition: background 0.25s;
  }
  i.on {
    background: var(--c);
  }
  .word {
    font-size: 28px;
    font-weight: 800;
    line-height: 1.1;
  }
  .n {
    color: var(--c);
    font-variant-numeric: tabular-nums;
  }
  .lv {
    margin-left: 8px;
    font-size: 17px;
    font-weight: 700;
    color: var(--muted);
    white-space: nowrap;
  }
  .hint {
    margin: 0;
    color: var(--muted);
    font-size: 14px;
    line-height: 1.4;
  }
</style>
