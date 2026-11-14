<script lang="ts">
  // The loading mark of a button whose work takes time (an API, the network, the AI): the rule is that such a
  // button turns into this until it answers. Decorative: the button says what is loading (aria-busy, its label).
  let { size = 16 }: { size?: number } = $props();
</script>

<span class="spinner" style:--s="{size}px" aria-hidden="true"></span>

<style>
  .spinner {
    display: inline-block;
    flex: none;
    width: var(--s);
    height: var(--s);
    border: 2px solid var(--surface-3);
    border-top-color: var(--primary);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .spinner {
      animation-duration: 2.4s;
    }
  }
</style>
