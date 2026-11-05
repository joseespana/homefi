<script lang="ts">
  // Playing now: what you hear and why, the combined curve, the music this
  // sound is for (add / take out the song, album, artist or playlist), Hear
  // without, Edit curve and Ask the AI. Off, it says bit-perfect. Slim, it is
  // only the song and its sound (the By music tab has the room).
  import type { TasteScope } from "../../lib/api";
  import { usePreview } from "../../lib/preview.svelte";
  import { flatProfile } from "../../lib/eq.svelte";
  import { holds, kindLabel, ruleWords, sharesOutput, sound } from "../../lib/sound.svelte";
  import { app } from "../../lib/state.svelte";
  import Cover from "../Cover.svelte";
  import EqGraph from "../eq/EqGraph.svelte";
  import Icon from "../Icon.svelte";
  import AiTag from "./AiTag.svelte";
  import AskAi from "./AskAi.svelte";
  import HearWithout from "./HearWithout.svelte";
  import PlaysOn from "./PlaysOn.svelte";

  let { slim = false }: { slim?: boolean } = $props();

  const FLAT = flatProfile();
  const now = $derived(sound.now);
  const ctx = $derived(now?.context ?? null);
  const rule = $derived(now?.rule ?? null);
  const on = $derived(sound.on);
  const output = $derived(sound.output);
  const device = $derived(sound.profileFor(output));

  const preview = usePreview(
    () => rule,
    () => output,
  );
  const mainCurve = $derived(rule && on ? (preview.value?.combined ?? null) : null);

  const artists = $derived(ctx?.artists.map((a) => a.name).join(", ") ?? "");
  const subtitle = $derived(
    [artists, ctx?.album?.name, ctx?.source && ctx.source.kind !== "library" ? `from ${ctx.source.kind} “${ctx.source.name}”` : null].filter(Boolean).join(" · "),
  );

  /** "Rule for the artist Eagles, on your WH-1000XM5. Lifts…" */
  const why = $derived.by(() => {
    if (!rule) return "";
    if (now?.auditioning) return `Trying it on this song only, nothing saved. ${rule.explanation ?? ruleWords(rule)}`;
    const t = now?.target;
    const who = t ? `Rule for ${t.kind === "genre" ? `the genre ${t.name}` : `the ${kindLabel(t.kind).toLowerCase()} ${t.name}`}` : `“${rule.label}”`;
    const where = output ? `, on your ${output}` : "";
    return `${who}${where}. ${rule.explanation ?? ruleWords(rule)}`;
  });

  // "Add to": one toggle for each thing the song belongs to
  const scopes = $derived(now?.scopes ?? []);
  const checked = (s: TasteScope) => !!rule && holds(rule, s);
  /** The other sound that holds it where this one plays (the amber note), when adding would move it. */
  const takenBy = (s: TasteScope) => (rule ? (sound.rules.find((r) => r.id !== rule.id && holds(r, s) && sharesOutput(r, rule)) ?? null) : null);
  let hint = $state<TasteScope | null>(null);
  const moving = $derived(hint && !checked(hint) ? takenBy(hint) : null);
  function toggle(s: TasteScope) {
    if (!rule) return;
    if (checked(s)) sound.unassign(rule.id, s);
    else sound.assign(rule.id, s);
  }
  const chipText = (s: TasteScope) => (checked(s) ? `✓ ${s.name || kindLabel(s.kind)}` : `+ ${s.kind === "genre" ? s.name : `This ${kindLabel(s.kind).toLowerCase()}`}`);

  async function newFromSong() {
    const s = scopes[0];
    if (!s) return;
    const saved = await sound.newSoundFor(s, sound.onlyHere ? output : null);
    if (saved) app.view = { name: "sound", edit: saved.rule.id };
  }
  const playing = $derived(!!ctx);
  /** A Try is playing: it isn't saved, so it has nothing to add to or edit yet. */
  const trying = $derived(!!now?.auditioning);
</script>

<section class="hero" class:slim class:off={!on} aria-label="Playing now">
  {#if !slim}<span class="cap">Playing now</span>{/if}

  {#if !playing}
    <div class="idle">
      <Icon name="soundwave" size={22} />
      <div>
        <h2>Play something to shape its sound</h2>
        <p class="muted">The curve shows the output's default. You can still ask the AI for a sound for a named artist or genre.</p>
      </div>
    </div>
  {:else}
    <div class="song">
      <Cover path={ctx?.path} size={slim ? 40 : 76} radius={10} />
      <div class="meta">
        <h2 class="ellipsis">{ctx?.title ?? "This song"}</h2>
        <p class="muted ellipsis">{subtitle}</p>
      </div>
      {#if slim}
        {#if on && rule}<span class="hear ellipsis"><Icon name="soundwave" size={14} /> {rule.label} <AiTag by={rule.createdBy} /></span>{/if}
        <button class="btn tonal" disabled={!rule} onclick={() => rule && (app.view = { name: "sound", tab: "music", edit: rule.id })}>Edit curve</button>
      {/if}
    </div>
  {/if}

  {#if !slim}
    {#if !on}
      <div class="why bit"><Icon name="check" size={16} /> <span><b>Bit-perfect:</b> HomeFi does not touch the audio.</span></div>
    {:else if rule && playing}
      <div class="why" class:trying={now?.auditioning}>
        <Icon name="music" size={16} />
        <div>
          <b>You hear: {rule.label}</b> <AiTag by={rule.createdBy} />
          {#if now?.auditioning}<span class="tag-try">Trying</span>{/if}
          <p>{why}</p>
        </div>
      </div>
    {:else if playing}
      <div class="why">
        <Icon name="music" size={16} />
        <div>
          <b>Flat{device ? " + headphone correction" : ""}</b>
          <p>No sound is set for this song. Try one with the AI or pick from Your sounds.</p>
        </div>
      </div>
    {/if}

    <div class="curve" class:dim={!on}>
      <EqGraph profile={FLAT} curve={mainCurve ?? (on ? device : null)} compare={rule && on ? (device ?? FLAT) : null} height={150} range={12} dim={!on} />
      {#if rule && on}
        <div class="legend"><span><i class="sw now"></i> What you hear (headphone correction + this sound)</span> <span><i class="sw was"></i> Original</span></div>
      {/if}
    </div>

    {#if playing && !trying}
      {#if rule}
        <div class="add" role="group" aria-label="Add to “{rule.label}”">
          <span class="label">Add to “{rule.label}”</span>
          {#each scopes as s (s.kind + s.name)}
            <button
              class="tog"
              class:on={checked(s)}
              aria-pressed={checked(s)}
              onclick={() => toggle(s)}
              onmouseenter={() => (hint = s)}
              onfocus={() => (hint = s)}
              onmouseleave={() => (hint = null)}
              onblur={() => (hint = null)}
            >{chipText(s)}</button>
          {/each}
        </div>
        {#if moving}<p class="warn" role="status">{hint!.name || kindLabel(hint!.kind)} moves from “{moving.label}” to “{rule.label}”. Undo shows for 8 s.</p>{/if}
      {:else}
        <div class="actions">
          <button class="btn primary" disabled={!on} title={on ? undefined : "Turn Sound on first"} onclick={newFromSong}>New sound from this song</button>
          <button class="btn tonal" onclick={() => sound.openAsk()}><Icon name="ai" size={16} /> Ask the AI for a sound</button>
          {#if output}<label class="only"><input type="checkbox" checked={sound.onlyHere} onchange={(e) => (sound.onlyHere = e.currentTarget.checked)} /> Only on {output}</label>{/if}
        </div>
      {/if}
    {/if}

    {#if rule && playing && !trying}<PlaysOn {rule} />{/if}

    <div class="buttons">
      <HearWithout disabled={!on || !playing} />
      <button class="btn tonal" disabled={!rule || trying} title={trying ? "Keep it first, then edit its curve" : undefined} onclick={() => rule && (app.view = { name: "sound", edit: rule.id })}>Edit curve</button>
    </div>

    <AskAi />
  {/if}
</section>

<style>
  .hero {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px 18px;
    border-radius: var(--r-lg);
    background: var(--glass);
    border: 1px solid var(--glass-border);
    backdrop-filter: blur(18px) saturate(1.2);
  }
  .hero.slim {
    flex-direction: row;
    align-items: center;
    padding: 10px 14px;
  }
  .cap {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--faint);
  }
  .song {
    display: flex;
    align-items: center;
    gap: 14px;
    min-width: 0;
    flex: 1;
  }
  .meta {
    min-width: 0;
    flex: 1;
  }
  h2 {
    margin: 0;
    font: 700 22px var(--font-display);
  }
  .slim h2 {
    font-size: 16px;
  }
  .meta p,
  .idle p {
    margin: 4px 0 0;
    font-size: 12.5px;
  }
  .hear {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    color: var(--accent-text);
  }
  .idle {
    display: flex;
    align-items: center;
    gap: 14px;
    color: var(--muted);
  }
  .idle h2 {
    color: var(--text);
    font-size: 18px;
  }
  .why {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 12px 14px;
    border-radius: var(--r-sm);
    background: var(--surface-2);
    border-left: 3px solid var(--primary);
    font-size: 14px;
  }
  .why.bit {
    align-items: center;
    border-left-color: var(--ok);
  }
  .why.trying {
    border-left-color: var(--accent-text);
  }
  .why > :global(svg) {
    flex: none;
    margin-top: 2px;
    color: var(--accent-text);
  }
  .why p {
    margin: 4px 0 0;
    font-size: 12.5px;
    line-height: 1.45;
    color: var(--muted);
  }
  .tag-try {
    margin-left: 6px;
    padding: 1px 8px;
    border-radius: var(--r-pill);
    font-size: 10.5px;
    font-weight: 700;
    color: var(--on-primary-strong);
    background: var(--primary-strong);
  }
  .curve {
    border-radius: var(--r-sm);
    background: color-mix(in srgb, var(--bg) 70%, transparent);
    border: 1px solid var(--border);
    overflow: hidden;
    transition: opacity 0.2s;
  }
  .curve.dim {
    opacity: 0.55;
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 18px;
    padding: 6px 12px 8px;
    font-size: 11.5px;
    color: var(--muted);
  }
  .legend span {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .sw {
    display: inline-block;
    width: 16px;
    border-top: 2.5px solid var(--accent-text);
  }
  .sw.was {
    border-top: 1.6px dashed var(--muted);
  }
  .add {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .label {
    margin-right: 4px;
    font-size: 12.5px;
    color: var(--muted);
  }
  .tog {
    max-width: 220px;
    height: 32px;
    padding: 0 14px;
    border-radius: var(--r-sm);
    font-size: 12.5px;
    color: var(--text);
    background: var(--surface-2);
    border: 1px solid var(--border);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tog:hover {
    border-color: color-mix(in srgb, var(--primary) 60%, var(--border));
  }
  .tog.on {
    font-weight: 700;
    color: var(--on-primary-strong);
    background: var(--primary-strong);
    border-color: var(--primary-strong);
  }
  .warn {
    margin: 0;
    padding: 8px 10px;
    border-radius: var(--r-sm);
    font-size: 12px;
    color: var(--warn);
    background: var(--warn-bg);
    border: 1px solid var(--warn-border);
  }
  .actions,
  .buttons {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .only {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    color: var(--muted);
  }
  .only input {
    accent-color: var(--primary-strong);
  }
  .off .song,
  .off .add,
  .off .actions,
  .off .why:not(.bit),
  .off :global(.ask) {
    opacity: 0.6;
  }
</style>
