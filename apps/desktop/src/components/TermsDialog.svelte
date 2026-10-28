<script lang="ts">
  // Accounts connected before the personal-use notice existed (or before its text
  // changed) see it once: accept it to go on, or sign out of Tidal. Nothing is lost
  // either way; the library and the downloaded files stay.
  import { dialog, fade } from "../lib/motion";
  import { api, errorText } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import TermsNotice from "./TermsNotice.svelte";

  let accepted = $state(false);
  let error = $state<string | null>(null);

  async function accept() {
    try {
      await app.acceptTerms();
    } catch (e) {
      error = errorText(e);
    }
  }

  async function signOut() {
    try {
      await api.logout();
      app.account = null;
    } catch (e) {
      error = errorText(e);
    }
  }
</script>

<div class="backdrop" role="presentation" transition:fade>
  <div class="dialog" transition:dialog role="dialog" aria-modal="true" aria-labelledby="terms-title" tabindex="-1">
    <h2 id="terms-title">Before you keep using Tidal</h2>
    <TermsNotice bind:checked={accepted} />
    {#if error}<p class="error">{error}</p>{/if}
    <div class="actions">
      <button type="button" class="btn tonal" onclick={signOut}>Sign out of Tidal</button>
      <button type="button" class="btn primary" disabled={!accepted} onclick={accept}>Continue</button>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 55;
    display: grid;
    place-items: center;
    background: rgb(5 4 12 / 0.6);
    backdrop-filter: blur(6px);
  }
  .dialog {
    width: min(440px, calc(100vw - 32px));
    padding: 24px 28px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: min(20px, var(--r-max));
    box-shadow: var(--shadow);
  }
  h2 {
    margin: 0;
    font-size: 18px;
  }
  .error {
    margin: 0;
    color: var(--danger);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }
</style>
