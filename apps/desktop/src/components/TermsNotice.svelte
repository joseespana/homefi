<script lang="ts">
  // The personal-use notice and its "I understand and accept" box, shown before
  // connecting Tidal (the login dialog and, once, to accounts signed in before it existed).
  import { TERMS_CHECK, TERMS_TEXT } from "../lib/terms";

  let { checked = $bindable(false) }: { checked?: boolean } = $props();
</script>

<div class="terms">
  <p class="muted">{TERMS_TEXT}</p>
  <label class="check"><input type="checkbox" bind:checked /> {TERMS_CHECK}</label>
</div>

<style>
  .terms {
    display: flex;
    flex-direction: column;
    gap: 10px;
    width: 100%;
    text-align: left;
  }
  p {
    margin: 0;
    max-height: 190px;
    overflow-y: auto;
    padding: 10px 12px;
    font-size: 12px;
    line-height: 1.5;
    border-radius: min(10px, var(--r-max));
    background: var(--surface-2);
    border: 1px solid var(--border);
  }
  .check {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    font-size: 14px;
    line-height: 1.4;
    cursor: pointer;
  }
  .check input {
    flex: none;
    width: 16px;
    height: 16px;
    margin: 2px 0 0;
    accent-color: var(--primary-strong);
  }
</style>
