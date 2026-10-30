<script lang="ts" generics="T extends string">
  import type { GalleryOption } from "./types";
  // Cards for choices that have a picture: a theme, a layout, a progress bar,
  // an AI provider. Columns come from the width (`min` = the narrowest a card gets).
  import type { Snippet } from "svelte";
  import ChoiceCard from "./ChoiceCard.svelte";
  import { roving } from "./roving";

  let {
    options,
    value,
    onchange,
    labelledby,
    min = 168,
    preview,
    extra,
  }: {
    options: readonly GalleryOption<T>[];
    value: T;
    onchange: (id: T) => void;
    labelledby?: string;
    /** Narrowest card in px. */
    min?: number;
    preview?: Snippet<[GalleryOption<T>]>;
    /** More under the label and hint (a badge, a status). */
    extra?: Snippet<[GalleryOption<T>]>;
  } = $props();
</script>

<div class="gallery" role="radiogroup" aria-labelledby={labelledby} style:--card-min="{min}px" use:roving={{ selector: "[role=radio]:not(:disabled)", vertical: true }}>
  {#each options as o (o.id)}
    {#snippet picture()}{@render preview?.(o)}{/snippet}
    {#snippet more()}{@render extra?.(o)}{/snippet}
    <ChoiceCard label={o.label} hint={o.hint} selected={value === o.id} disabled={o.disabled} onclick={() => onchange(o.id)} preview={picture} extra={more} />
  {/each}
</div>

<style>
  .gallery {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(var(--card-min, 168px), 100%), 1fr));
    gap: var(--s-3);
  }
</style>
