<script lang="ts">
  // The notes of the version just installed (or the running one, from About). One dialog, one
  // button; the body scrolls. Esc / backdrop / Got it close it; the focus returns where it was.
  import { untrack } from "svelte";
  import { dialog, fade } from "../lib/motion";
  import { whatsNew } from "../lib/whatsNew.svelte";
  import NoteText from "./NoteText.svelte";

  const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

  const notes = $derived(whatsNew.notes);
  const single = $derived(notes.length === 1 ? notes[0]! : null);
  const title = $derived(single?.version ? `What's new in HomeFi ${single.version}` : "What's new");

  /** "2026-11-03" -> "Nov 3, 2026"; anything else -> "". */
  function prettyDate(iso: string): string {
    const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
    const month = m ? MONTHS[Number(m[2]) - 1] : undefined;
    return m && month ? `${month} ${Number(m[3])}, ${m[1]}` : "";
  }

  let card = $state<HTMLElement>();
  let body = $state<HTMLElement>();
  let okBtn = $state<HTMLButtonElement>();
  let scrolledTop = $state(false);
  let scrolledBottom = $state(false);

  $effect(() => {
    const before = document.activeElement as HTMLElement | null;
    untrack(() => okBtn)?.focus();
    return () => before?.focus?.();
  });

  // the borders show only while there is more to scroll to; set only when the boolean changes
  function measure() {
    if (!body) return;
    const top = body.scrollTop > 0;
    const bottom = body.scrollHeight - body.clientHeight - body.scrollTop > 1;
    if (top !== scrolledTop) scrolledTop = top;
    if (bottom !== scrolledBottom) scrolledBottom = bottom;
  }
  $effect(() => {
    notes;
    body;
    measure();
  });

  function focusables(): HTMLElement[] {
    return card ? [...card.querySelectorAll<HTMLElement>("a[href], [tabindex='0'], button:not(:disabled)")] : [];
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape" || (e.key === "Enter" && !(document.activeElement instanceof HTMLAnchorElement))) {
      e.preventDefault();
      e.stopPropagation();
      whatsNew.close();
    } else if (e.key === "Tab") {
      const items = focusables();
      if (!items.length) return;
      const i = items.indexOf(document.activeElement as HTMLElement);
      e.preventDefault();
      items[(i + (e.shiftKey ? items.length - 1 : 1) + items.length) % items.length]?.focus();
    }
  }
</script>

<svelte:window onkeydowncapture={onKey} />

<div class="backdrop" role="presentation" transition:fade onclick={(e) => e.target === e.currentTarget && whatsNew.close()}>
  <div class="dialog" transition:dialog bind:this={card} role="dialog" aria-modal="true" aria-labelledby="whatsnew-title">
    <header class:edge={scrolledTop}>
      <h2 id="whatsnew-title">{title}</h2>
      {#if single?.version && prettyDate(single.date)}<p class="date">{prettyDate(single.date)}</p>{/if}
    </header>
    <!-- svelte-ignore a11y_no_noninteractive_tabindex (keyboard users scroll the notes) -->
    <div class="body" bind:this={body} onscroll={measure} tabindex="0" role="region" aria-label="Release notes">
      {#each notes as n, i (n.version + i)}
        <section class:divided={i > 0}>
          {#if !single}
            <h3>HomeFi {n.version}{prettyDate(n.date) ? ` · ${prettyDate(n.date)}` : ""}</h3>
          {/if}
          {#if n.summary}<p class="summary"><NoteText line={n.summary} /></p>{/if}
          {#each n.sections as sec (sec.title)}
            <h4>{sec.title}</h4>
            <ul>
              {#each sec.items as item, j (j)}<li><NoteText line={item} /></li>{/each}
            </ul>
          {/each}
          {#if !n.summary && !n.sections.length}<p class="none">No notes for this version.</p>{/if}
        </section>
      {/each}
    </div>
    <footer class:edge={scrolledBottom}>
      <button type="button" class="btn primary" bind:this={okBtn} onclick={() => whatsNew.close()}>Got it</button>
    </footer>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 56;
    display: grid;
    place-items: center;
    background: rgb(5 4 12 / 0.6);
    backdrop-filter: blur(6px);
  }
  .dialog {
    width: min(520px, calc(100vw - 32px));
    max-height: min(640px, calc(100vh - 48px));
    display: flex;
    flex-direction: column;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: min(20px, var(--r-max));
    box-shadow: var(--shadow);
    overflow: hidden;
  }
  header,
  footer {
    flex: none;
    padding: 0 28px;
    border: 0 solid transparent;
  }
  header {
    padding-top: 24px;
    padding-bottom: 12px;
    border-bottom-width: 1px;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    padding-top: 12px;
    padding-bottom: 20px;
    border-top-width: 1px;
  }
  header.edge,
  footer.edge {
    border-color: var(--border);
  }
  h2 {
    margin: 0;
    font-size: 18px;
    overflow-wrap: anywhere;
  }
  .date {
    margin: 2px 0 0;
    font-size: 12.5px;
    color: var(--muted);
  }
  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
    padding: 4px 28px;
    font-size: 13.5px;
    line-height: 1.5;
    overflow-wrap: anywhere;
    user-select: text;
  }
  section.divided {
    margin-top: 16px;
    padding-top: 16px;
    border-top: 1px solid var(--border);
  }
  h3 {
    margin: 0 0 6px;
    font-size: 15px;
    font-weight: 600;
  }
  .summary {
    margin: 4px 0 12px;
  }
  h4 {
    margin: 14px 0 6px;
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--muted);
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    position: relative;
    padding-left: 14px;
  }
  li + li {
    margin-top: 6px;
  }
  li::before {
    content: "•";
    position: absolute;
    left: 2px;
    color: var(--primary);
  }
  .none {
    margin: 4px 0;
    color: var(--muted);
  }
</style>
