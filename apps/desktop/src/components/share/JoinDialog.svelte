<script lang="ts">
  // Joins a session: a hub found nearby (only the code is missing) or one typed
  // by hand (an address, `host:port`, or its homefi://join link).
  import { dialog, fade } from "../../lib/motion";
  import { api } from "../../lib/api";
  import { share } from "../../lib/share.svelte";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";

  const form = $derived(share.joinForm!);
  const found = $derived(!!form.sessionName && !!form.host);
  let host = $state(share.joinForm?.host ?? "");
  let port = $state(share.joinForm?.port ?? 47800);
  let code = $state(share.joinForm?.code ?? "");
  let sessionName = $state(share.joinForm?.sessionName ?? "");
  let name = $state(app.settings?.share.name || share.name);
  let color = $state(app.settings?.share.color ?? "#0891B2");
  let busy = $state(false);
  let error = $state<string | null>(null);
  const COLORS = ["#0891B2", "#1D4ED8", "#10B981", "#F59E0B", "#EF4444", "#EC4899", "#8B5CF6", "#64748B"];

  const close = () => (share.joinForm = null);

  /** A pasted link or `host:port` fills the fields. */
  async function readAddress() {
    const text = host.trim();
    if (text.startsWith("homefi://")) {
      const link = await api.shareParseLink(text);
      if (!link) {
        error = "Not a HomeFi join link";
        return;
      }
      host = link.host;
      port = link.port;
      if (link.code) code = link.code;
      if (link.session_name) sessionName = link.session_name;
      error = null;
      return;
    }
    const m = /^([^\s:]+|\[[^\]]+\]):(\d{1,5})$/.exec(text);
    if (m) {
      host = m[1].replace(/^\[|\]$/g, "");
      port = Number(m[2]);
    }
  }

  async function join(e: SubmitEvent) {
    e.preventDefault();
    if (busy) return;
    await readAddress();
    if (!host.trim()) {
      error = "Enter the hub’s address (or paste its homefi:// link)";
      return;
    }
    busy = true;
    error = null;
    // a typed address: say whose session it is, when it answers
    if (!sessionName) sessionName = (await api.shareProbe(host.trim(), port).catch(() => null))?.name ?? "";
    const ok = await share.join(host.trim(), port, code.trim(), sessionName, name.trim() || undefined, color);
    busy = false;
    if (!ok) error = share.guest.error ?? "Could not join";
  }

  function autofocus(el: HTMLInputElement) {
    el.focus();
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && !app.confirmDialog && close()} />

<div class="backdrop" transition:fade role="presentation" onclick={(e) => e.target === e.currentTarget && close()}>
  <form class="dialog" onsubmit={join} transition:dialog>
    <button type="button" class="icon-btn close" aria-label="Close" onclick={close}><Icon name="x" /></button>
    <h2>{found ? `Join ${form.sessionName}` : "Join with a code"}</h2>
    {#if found}
      <p class="muted mono">{form.host}:{form.port}</p>
    {:else}
      <div class="pair">
        <label class="grow">
          <span>Address or link</span>
          <input bind:value={host} placeholder="192.168.1.20 or homefi://join?…" onblur={readAddress} use:autofocus />
        </label>
        <label class="port">
          <span>Port</span>
          <input type="number" min="1" max="65535" bind:value={port} />
        </label>
      </div>
    {/if}
    <label>
      <span>Join code</span>
      {#if found}
        <input class="code" bind:value={code} inputmode="numeric" maxlength="8" placeholder="0000" use:autofocus />
      {:else}
        <input class="code" bind:value={code} inputmode="numeric" maxlength="8" placeholder="0000" />
      {/if}
    </label>
    <div class="pair">
      <label class="grow">
        <span>Your name</span>
        <input bind:value={name} maxlength="32" placeholder={share.name} />
      </label>
      <label>
        <span>Your colour</span>
        <span class="colors">
          {#each COLORS as c}
            <button type="button" class="swatch" class:on={color.toLowerCase() === c.toLowerCase()} style:background={c} aria-label="Colour {c}" onclick={() => (color = c)}></button>
          {/each}
        </span>
      </label>
    </div>
    {#if error}<p class="error">{error}</p>{/if}
    <div class="actions">
      <button type="button" class="btn tonal" onclick={close}>Cancel</button>
      <button type="submit" class="btn primary" disabled={busy}>{busy ? "Joining…" : "Join"}</button>
    </div>
  </form>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    background: rgb(4 10 18 / 0.6);
    backdrop-filter: blur(6px);
  }
  .dialog {
    position: relative;
    width: min(460px, calc(100vw - 32px));
    padding: 28px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: min(20px, var(--r-max));
    box-shadow: var(--shadow);
  }
  h2 {
    margin: 0;
    overflow-wrap: anywhere;
  }
  p {
    margin: 0;
  }
  .mono {
    font-family: var(--font-mono);
    font-size: 13px;
  }
  .close {
    position: absolute;
    top: 12px;
    right: 12px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--muted);
  }
  input {
    padding: 10px 12px;
    border-radius: min(10px, var(--r-max));
    font-size: 14px;
    font-weight: 400;
  }
  .code {
    font: 700 24px var(--font-mono);
    letter-spacing: 0.3em;
    text-align: center;
  }
  .pair {
    display: flex;
    gap: 12px;
    align-items: flex-end;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .port {
    width: 96px;
  }
  .colors {
    display: flex;
    gap: 6px;
    height: 40px;
    align-items: center;
  }
  .swatch {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    box-shadow: 0 0 0 2px var(--surface), 0 0 0 3px transparent;
  }
  .swatch.on {
    box-shadow: 0 0 0 2px var(--surface), 0 0 0 4px var(--text);
  }
  .error {
    color: var(--danger);
    font-size: 13px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  @media (max-width: 520px) {
    .pair {
      flex-direction: column;
      align-items: stretch;
    }
    .port {
      width: auto;
    }
  }
</style>
