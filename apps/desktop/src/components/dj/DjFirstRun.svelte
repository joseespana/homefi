<script lang="ts">
  // "Your first mix": three steps, once (Settings → DJ → Show tips brings it back).
  import { dj } from "../../lib/dj.svelte";
  import Icon from "../Icon.svelte";
</script>

<aside class="first" aria-labelledby="dj-first-title">
  <span class="mark" aria-hidden="true"><Icon name="sparkles" size={18} /></span>
  <div class="body">
    <h2 id="dj-first-title">Your first mix</h2>
    <ol>
      <li>Drag a song to deck A and press Play.</li>
      <li>Drag another to deck B.</li>
      <li>Press Mix to B.</li>
    </ol>
  </div>
  <button class="btn primary" type="button" onclick={() => dj.saveSettings({ show_tips: false })}>Got it</button>
</aside>

<style>
  .first {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px 14px;
    background: color-mix(in srgb, var(--primary) 10%, var(--surface));
    border: 1px solid color-mix(in srgb, var(--primary) 40%, var(--border));
    border-radius: var(--r-lg);
  }
  .mark {
    flex: none;
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border-radius: 50%;
    background: var(--dj-mix-gradient, var(--primary));
    color: var(--dj-mix-ink);
  }
  .body {
    flex: 1;
    display: flex;
    align-items: baseline;
    gap: 16px;
    flex-wrap: wrap;
    min-width: 0;
  }
  h2 {
    margin: 0;
    font-size: 15px;
  }
  ol {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 18px;
    margin: 0;
    padding: 0 0 0 18px;
    color: var(--muted);
  }
  .btn {
    flex: none;
    height: 40px;
  }
</style>
