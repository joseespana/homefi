<script lang="ts">
  // Creates a playlist on Tidal (with the tracks that were being added, if any).
  import { dialog, fade } from "../lib/motion";
  import { errorText } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import Icon from "./Icon.svelte";

  const trackIds = $derived(app.newPlaylist?.trackIds ?? []);
  let title = $state("");
  let description = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);

  const close = () => (app.newPlaylist = null);

  async function create(e: SubmitEvent) {
    e.preventDefault();
    if (!title.trim() || busy) return;
    busy = true;
    error = null;
    try {
      const hit = await app.createPlaylist(title, description, trackIds);
      close();
      app.view = { name: "tidal-item", kind: "playlist", id: hit.id, back: app.view };
    } catch (err) {
      error = errorText(err);
    } finally {
      busy = false;
    }
  }

  function autofocus(el: HTMLInputElement) {
    el.focus();
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

<div class="backdrop" transition:fade>
  <form class="dialog" onsubmit={create} transition:dialog>
    <button type="button" class="icon-btn close" aria-label="Close" onclick={close}><Icon name="x" /></button>
    <h2>New playlist</h2>
    {#if trackIds.length}<p class="muted">With {trackIds.length === 1 ? "1 track" : `${trackIds.length} tracks`}</p>{/if}
    <label>
      <span>Name</span>
      <input bind:value={title} maxlength="100" placeholder="My playlist" use:autofocus required />
    </label>
    <label>
      <span>Description <em class="muted">(optional)</em></span>
      <textarea bind:value={description} rows="3" maxlength="500"></textarea>
    </label>
    {#if error}<p class="error">{error}</p>{/if}
    <div class="actions">
      <button type="button" class="btn tonal" onclick={close}>Cancel</button>
      <button type="submit" class="btn primary" disabled={!title.trim() || busy}>{busy ? "Creating…" : "Create on Tidal"}</button>
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
    width: min(420px, calc(100vw - 32px));
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
  }
  p {
    margin: 0;
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
  input,
  textarea {
    padding: 10px 12px;
    border-radius: min(10px, var(--r-max));
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 14px;
    font-weight: 400;
    resize: vertical;
  }
  input:focus,
  textarea:focus {
    outline: 2px solid var(--primary);
    outline-offset: -1px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
