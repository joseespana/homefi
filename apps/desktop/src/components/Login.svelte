<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { dialog, fade } from "../lib/motion";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api, errorText, type DeviceLogin } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import Spinner from "./Spinner.svelte";
  import Icon from "./Icon.svelte";
  import TermsNotice from "./TermsNotice.svelte";

  let device = $state<DeviceLogin | null>(null);
  let error = $state<string | null>(null);
  let left = $state(0);
  let cancelled = false;
  // the personal-use notice comes first until it's accepted (once per notice version)
  let gate = $state(untrack(() => !app.termsAccepted));
  let accepted = $state(false);
  let accepting = $state(false);

  async function acceptAndStart() {
    if (accepting) return;
    accepting = true;
    try {
      await app.acceptTerms();
    } catch (e) {
      error = errorText(e);
      return;
    } finally {
      accepting = false;
    }
    gate = false;
    start();
  }

  async function start() {
    error = null;
    cancelled = false;
    try {
      device = await api.startLogin();
      const deadline = Date.now() + device.expiresIn * 1000;
      await openUrl(device.url);
      while (!cancelled) {
        left = Math.max(0, Math.round((deadline - Date.now()) / 1000));
        if (left === 0) throw "The code expired, try again.";
        await new Promise((r) => setTimeout(r, device!.interval * 1000));
        if (cancelled) return;
        const state = await api.pollLogin(device.deviceCode);
        if (state === "success") {
          app.account = await api.account();
          app.loadFavorites();
          app.loginOpen = false;
          return;
        }
        if (state === "expired") throw "The code expired, try again.";
      }
    } catch (e) {
      error = errorText(e);
      device = null;
    }
  }

  function close() {
    cancelled = true;
    app.loginOpen = false;
  }

  onMount(() => {
    if (!gate) start();
    return () => (cancelled = true);
  });
</script>

<div class="backdrop" role="presentation" onclick={close} transition:fade>
  <div class="dialog" transition:dialog role="dialog" aria-modal="true" aria-label="Connect Tidal" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.key === "Escape" && close()}>
    <button class="icon-btn close" onclick={close} aria-label="Close"><Icon name="x" /></button>
    <img src="/logo.svg" alt="" width="72" height="72" />
    <h1>Connect Tidal</h1>
    {#if gate}
      <TermsNotice bind:checked={accepted} />
      {#if error}<p class="error">{error}</p>{/if}
      <button class="btn primary" disabled={!accepted || accepting} aria-busy={accepting} onclick={acceptAndStart}>{#if accepting}<Spinner size={16} /> Signing in…{:else}Sign in with Tidal{/if}</button>
    {:else if error}
      <p class="error">{error}</p>
      <button class="btn primary" onclick={start}>Try again</button>
    {:else if device}
      <p class="muted">Check this code matches, then approve it in the browser window that just opened:</p>
      <div class="code">{device.userCode}</div>
      <button class="btn tonal" onclick={() => openUrl(device!.url)}><Icon name="external" size={16} /> Open Tidal again</button>
      <p class="muted small"><Spinner size={12} /> Waiting for approval… {Math.floor(left / 60)}:{String(left % 60).padStart(2, "0")}</p>
    {:else}
      <p class="muted" aria-busy="true"><Spinner size={14} /> Contacting Tidal…</p>
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    background: rgb(5 4 12 / 0.6);
    backdrop-filter: blur(6px);
  }
  .dialog {
    position: relative;
    width: min(420px, calc(100vw - 32px));
    padding: 32px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    text-align: center;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
    box-shadow: var(--shadow);
  }
  .dialog img {
    border-radius: var(--r-lg);
  }
  .close {
    position: absolute;
    top: 10px;
    right: 10px;
  }
  .code {
    font: 700 34px/1 var(--font-mono);
    letter-spacing: 0.2em;
    padding: 16px 24px;
    border-radius: var(--r-md);
    background: var(--surface-3);
    user-select: text;
  }
  .small {
    font-size: 12px;
  }
  .error {
    color: var(--danger);
  }
</style>
