<script lang="ts">
  // Shared control & party: start sharing this computer's music (code, QR, guests,
  // approvals, stop) or join someone else's session found on the network.
  import { drawer, fade } from "../../lib/motion";
  import { formatTime } from "../../lib/api";
  import { share } from "../../lib/share.svelte";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";
  import QrCode from "./QrCode.svelte";

  const hub = $derived(share.hub);
  const close = () => (share.panelOpen = false);
  let party = $state(app.settings?.share.party_default ?? false);
  const now = $derived(hub.state?.now_playing ?? null);

  /** Settings on the "Shared control & party" page. */
  function openRules() {
    try {
      localStorage.setItem("homefi.settings.page", "share");
    } catch {}
    app.view = { name: "settings" };
    close();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape" && !app.confirmDialog) close();
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="backdrop" transition:fade role="presentation" onclick={(e) => e.target === e.currentTarget && close()}>
  <aside class="sheet" transition:drawer aria-label="Shared control and party">
    <header>
      <span class="mark"><Icon name="cast" size={18} /></span>
      <div class="grow">
        <h2>{hub.running ? `Sharing “${hub.info?.session_name}”` : share.inRoom ? "In a session" : "Share this music"}</h2>
        <p class="muted small">
          {#if hub.running}{hub.party ? "Party: guests can play it on their own speakers" : "Shared control: guests pick songs, the music plays here"}{:else}Others on this network join from HomeFi on their phone or computer.{/if}
        </p>
      </div>
      <button class="icon-btn" aria-label="Close" onclick={close}><Icon name="x" /></button>
    </header>

    {#if hub.running && hub.info}
      <section class="join">
        <QrCode text={hub.info.join_uri} size={176} />
        <div class="code-box">
          <span class="muted small">Join code</span>
          <span class="code" aria-label="Join code {hub.info.code.split('').join(' ')}">{hub.info.code}</span>
          <span class="muted small mono">{hub.info.host}:{hub.info.port}</span>
          <p class="muted small hint">Scan it with the phone’s camera, or open HomeFi: the session shows up by itself.</p>
        </div>
      </section>

      {#if now}
        <div class="now">
          <Icon name="music" size={15} />
          <span class="ellipsis grow">{now.title} · {now.artist}</span>
          {#if now.duration_ms}<span class="muted small">{formatTime(now.duration_ms)}</span>{/if}
        </div>
      {/if}

      <section>
        <label class="toggle">
          <input type="checkbox" checked={hub.party} onchange={(e) => share.setParty(e.currentTarget.checked)} />
          <span><b>Party: stream to speakers</b><small class="muted">Everyone plays it on their own speaker, in sync</small></span>
        </label>
        {#if hub.party}
          <label class="toggle">
            <input type="checkbox" checked={hub.play_here} onchange={(e) => share.setPlayHere(e.currentTarget.checked)} />
            <span><b>Play on this computer too</b><small class="muted">Off: this computer is only the DJ, the guests’ speakers play</small></span>
          </label>
        {/if}
      </section>

      {#if hub.pending.length}
        <section>
          <h3>Waiting for your OK ({hub.pending.length})</h3>
          {#each hub.pending as item (item.queue_id)}
            <div class="row">
              <span class="dot" style:background={item.added_by.color}></span>
              <div class="grow">
                <div class="ellipsis">{item.title}</div>
                <div class="muted small ellipsis">{item.artist} · by {item.added_by.name}</div>
              </div>
              <button class="btn tonal small-btn" onclick={() => share.reject(item)}>Reject</button>
              <button class="btn primary small-btn" onclick={() => share.approve(item)}>Approve</button>
            </div>
          {/each}
        </section>
      {/if}

      <section>
        <h3>Guests ({hub.guests.length})</h3>
        {#each hub.guests as g (g.guest_id)}
          <div class="row">
            <span class="dot" style:background={g.color}></span>
            <span class="grow ellipsis">{g.name}</span>
            <button class="icon-btn" title="Remove from the session" aria-label="Remove {g.name} from the session" onclick={() => share.kick(g)}>
              <Icon name="x" size={16} />
            </button>
          </div>
        {:else}
          <p class="muted small">Nobody yet. Show them the QR code or tell them the code.</p>
        {/each}
      </section>

      <footer>
        <button class="btn tonal" onclick={openRules}><Icon name="settings" size={15} /> Rules</button>
        <button class="btn danger" onclick={() => share.stop()}>Stop sharing</button>
      </footer>
    {:else if hub.starting}
      <p class="muted starting"><span class="spin"></span> Starting…</p>
    {:else if share.inRoom}
      <section>
        <p>You are in “{share.guest.session_name}”. Leave it to share your own music.</p>
        <div class="actions">
          <button class="btn tonal" onclick={() => share.leave()}>Leave</button>
          <button class="btn primary" onclick={() => { share.open(); close(); }}>Open</button>
        </div>
      </section>
    {:else}
      <section>
        <div class="modes">
          <button class="mode" class:selected={!party} onclick={() => (party = false)}>
            <Icon name="users" size={20} />
            <b>Share control</b>
            <span>Guests pick songs from their devices; the music plays here</span>
          </button>
          <button class="mode" class:selected={party} onclick={() => (party = true)}>
            <Icon name="cast" size={20} />
            <b>Party: stream to speakers</b>
            <span>Everyone plays it on their own speaker, in sync</span>
          </button>
        </div>
        <p class="muted small">As “{share.name}” · over this computer’s network (Wi-Fi or Ethernet)</p>
        {#if hub.error}<p class="error">{hub.error}</p>{/if}
        <div class="actions">
          <button class="btn primary" onclick={() => share.start(party)}><Icon name="cast" size={16} /> Start sharing</button>
        </div>
      </section>

      <section>
        <h3>Join someone’s music</h3>
        {#each share.nearby as n (n.host + n.port)}
          <div class="row">
            <span class="mark small-mark"><Icon name="wifi" size={15} /></span>
            <div class="grow">
              <div class="ellipsis">{n.name}</div>
              <div class="muted small mono">{n.host}:{n.port}</div>
            </div>
            <button class="btn primary small-btn" onclick={() => share.askJoin(n)}>Join</button>
          </div>
        {:else}
          <p class="muted small">
            {app.settings?.share.discover === false ? "Looking for sessions nearby is off (Settings → Shared control & party)." : "Looking for sessions on this network…"}
          </p>
        {/each}
        <button class="btn tonal" onclick={() => share.askJoin()}><Icon name="plus" size={15} /> Join with a code</button>
      </section>
    {/if}
  </aside>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 45;
    background: rgb(4 10 18 / 0.35);
    animation: fade 0.15s ease;
  }
  .sheet {
    position: absolute;
    top: 12px;
    right: 12px;
    bottom: 112px;
    width: min(440px, calc(100vw - 24px));
    overflow-y: auto;
    padding: 20px 22px;
    display: flex;
    flex-direction: column;
    gap: 18px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: min(20px, var(--r-max));
    box-shadow: var(--shadow);
    animation: slide 0.2s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  header {
    display: flex;
    align-items: flex-start;
    gap: 12px;
  }
  h2 {
    margin: 0 0 2px;
    overflow-wrap: anywhere;
  }
  h3 {
    margin: 0 0 8px;
    font-size: 14px;
  }
  p {
    margin: 0;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .small {
    font-size: 12.5px;
  }
  .mono {
    font-family: var(--font-mono);
  }
  .mark {
    display: grid;
    place-items: center;
    flex: none;
    width: 36px;
    height: 36px;
    border-radius: min(11px, var(--r-max));
    background: var(--gradient);
    color: #fff;
  }
  .small-mark {
    width: 30px;
    height: 30px;
    border-radius: min(9px, var(--r-max));
  }
  section.join {
    flex-direction: row;
    gap: 18px;
    align-items: center;
  }
  .code-box {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .code {
    font: 800 46px/1 var(--font-mono);
    letter-spacing: 0.18em;
    color: var(--primary);
  }
  .hint {
    margin-top: 6px;
  }
  .now {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-radius: min(10px, var(--r-max));
    background: var(--surface-2);
    font-size: 13px;
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .toggle {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 4px 0;
  }
  .toggle input {
    width: 16px;
    height: 16px;
    margin-top: 3px;
    accent-color: var(--primary-strong);
  }
  .toggle span {
    display: flex;
    flex-direction: column;
  }
  .toggle small {
    font-size: 12px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 8px;
    border-radius: min(10px, var(--r-max));
    background: var(--surface-2);
  }
  .dot {
    flex: none;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    box-shadow: 0 0 0 2px var(--surface);
  }
  .small-btn {
    height: 30px;
    padding: 0 12px;
    font-size: 13px;
  }
  .modes {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .mode {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    padding: 12px;
    border-radius: min(12px, var(--r-max));
    text-align: left;
    background: var(--surface-2);
    border: 1px solid var(--border);
    color: var(--muted);
  }
  .mode b {
    color: var(--text);
  }
  .mode span {
    font-size: 12px;
  }
  .mode.selected {
    border-color: var(--primary);
    box-shadow: inset 0 0 0 1px var(--primary);
    background: color-mix(in srgb, var(--primary) 14%, var(--surface));
    color: var(--primary);
  }
  .actions,
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  footer {
    justify-content: space-between;
    margin-top: auto;
  }
  .btn.danger {
    background: color-mix(in srgb, var(--danger) 78%, black);
    color: #fff;
  }
  .error {
    color: var(--danger);
    font-size: 13px;
  }
  .starting {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .spin {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid color-mix(in srgb, currentColor 30%, transparent);
    border-top-color: currentColor;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  @keyframes slide {
    from {
      opacity: 0;
      transform: translateX(20px);
    }
  }
  @media (max-width: 520px) {
    section.join {
      flex-direction: column;
      align-items: flex-start;
    }
    .modes {
      grid-template-columns: 1fr;
    }
  }
</style>
