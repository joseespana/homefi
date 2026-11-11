<script lang="ts">
  // What a chat's screen shows when the chat was closed from the AI chats menu while it was open.
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";
</script>

<div class="closed" role="status">
  <p>This chat was closed.</p>
  <!-- svelte-ignore a11y_autofocus -->
  <button class="btn primary" autofocus onclick={() => (app.view = { name: "playlists" })}><Icon name="back" size={16} /> Back</button>
</div>

<style>
  .closed {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    margin: 12vh auto 0;
    color: var(--muted);
  }
  .closed p {
    margin: 0;
    font-size: 15px;
  }
</style>
