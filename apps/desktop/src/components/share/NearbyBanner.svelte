<script lang="ts">
  // "José is sharing music nearby · Join": a session found on the network.
  import { toast } from "../../lib/motion";
  import { share } from "../../lib/share.svelte";
  import Icon from "../Icon.svelte";

  const hub = $derived(share.offers[0]);
</script>

{#if hub}
  <div class="banner" role="status" transition:toast|global={{ x: 0, y: -18 }}>
    <span class="mark"><Icon name="cast" size={16} /></span>
    <div class="grow">
      <b class="ellipsis">{hub.name}</b>
      <small class="muted">is sharing music nearby{share.offers.length > 1 ? ` · ${share.offers.length - 1} more` : ""}</small>
    </div>
    <button class="btn tonal small-btn" onclick={() => share.dismiss(hub)}>Not now</button>
    <button class="btn primary small-btn" onclick={() => share.askJoin(hub)}>Join</button>
  </div>
{/if}

<style>
  .banner {
    position: fixed;
    top: 40px;
    left: 50%;
    z-index: 30;
    transform: translateX(-50%);
    width: min(520px, calc(100vw - 120px));
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border-radius: min(16px, var(--r-max));
    background: color-mix(in srgb, var(--surface) 90%, transparent);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
    backdrop-filter: blur(18px) saturate(1.3);
    animation: drop 0.25s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .mark {
    display: grid;
    place-items: center;
    flex: none;
    width: 32px;
    height: 32px;
    border-radius: min(10px, var(--r-max));
    background: var(--gradient);
    color: #fff;
  }
  .grow {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  small {
    font-size: 12px;
  }
  .small-btn {
    height: 30px;
    padding: 0 12px;
    font-size: 13px;
  }
  @keyframes drop {
    from {
      opacity: 0;
      transform: translate(-50%, -10px);
    }
  }
</style>
