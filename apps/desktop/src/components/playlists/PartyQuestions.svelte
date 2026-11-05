<script lang="ts">
  // The party questions. Three are always there (who's coming, the vibe, how long); the rest
  // (ages, energy, explicit lyrics, anything else) fold under "More details", which opens by
  // itself when one of them isn't at its default. Every answer starts as the user's setting.
  import { ENERGY_CURVES, PARTY_MINUTES_RANGE, partyLength, type AgeGroup, type PartyBrief, type PartyGuests, type PartyVibe } from "../../lib/api";
  import { MAX_NOTES_CHARS } from "../../lib/party.svelte";
  import Icon from "../Icon.svelte";
  import { roving } from "../settings/roving";
  import Segmented from "../settings/Segmented.svelte";

  let { brief, disabled = false, defaults, onchange }: { brief: PartyBrief; disabled?: boolean; defaults: Pick<PartyBrief, "curve" | "allow_explicit">; onchange: (patch: Partial<PartyBrief>) => void } = $props();

  const GUESTS: { id: PartyGuests; label: string }[] = [
    { id: "friends", label: "Friends" },
    { id: "family", label: "Family" },
    { id: "coworkers", label: "Coworkers" },
    { id: "mixed", label: "A mix" },
  ];
  const VIBES: { id: PartyVibe; label: string }[] = [
    { id: "background", label: "In the background" },
    { id: "chill", label: "Chill" },
    { id: "dance", label: "Dance" },
    { id: "singalong", label: "Sing-along" },
    { id: "throwback", label: "Throwback" },
  ];
  const AGES: { id: AgeGroup; label: string }[] = [
    { id: "kids", label: "Kids" },
    { id: "teens", label: "Teens" },
    { id: "young_adults", label: "18–29" },
    { id: "adults", label: "30–49" },
    { id: "seniors", label: "50+" },
  ];
  const EXPLICIT = [
    { id: "allow", label: "Allow" },
    { id: "avoid", label: "Avoid" },
  ] as const;

  // opens once, at the start, when an answer in there differs from the user's defaults
  // svelte-ignore state_referenced_locally
  let more = $state(brief.ages.length > 0 || brief.notes.trim() !== "" || brief.curve !== defaults.curve || brief.allow_explicit !== defaults.allow_explicit);

  const curveLabel = $derived(ENERGY_CURVES.find((c) => c.id === brief.curve)?.label ?? "");
  const summary = $derived(
    [brief.ages.length ? AGES.filter((a) => brief.ages.includes(a.id)).map((a) => a.label).join(", ") : "Any age", curveLabel, brief.allow_explicit ? "Explicit allowed" : "No explicit"].join(" · "),
  );
  const toggleAge = (id: AgeGroup) => onchange({ ages: brief.ages.includes(id) ? brief.ages.filter((a) => a !== id) : [...brief.ages, id] });
</script>

{#snippet check(on: boolean)}
  {#if on}<Icon name="check" size={14} />{/if}
{/snippet}

<div class="questions">
  <div class="q">
    <span class="label" id="q-guests">Who's coming?</span>
    <div class="chips" role="radiogroup" aria-labelledby="q-guests" use:roving={{ selector: "[role=radio]:not(:disabled)", vertical: false }}>
      {#each GUESTS as g (g.id)}
        {@const on = brief.guests === g.id}
        <button type="button" role="radio" class:selected={on} aria-checked={on} tabindex={on ? 0 : -1} {disabled} onclick={() => onchange({ guests: g.id })}>{@render check(on)}{g.label}</button>
      {/each}
    </div>
  </div>

  <div class="q">
    <span class="label" id="q-vibe">What's the vibe?</span>
    <div class="chips" role="radiogroup" aria-labelledby="q-vibe" use:roving={{ selector: "[role=radio]:not(:disabled)", vertical: false }}>
      {#each VIBES as v (v.id)}
        {@const on = brief.vibe === v.id}
        <button type="button" role="radio" class:selected={on} aria-checked={on} tabindex={on ? 0 : -1} {disabled} onclick={() => onchange({ vibe: v.id })}>{@render check(on)}{v.label}</button>
      {/each}
    </div>
  </div>

  <label class="q">
    <span class="label row">How long? <b>{partyLength(brief.minutes)}</b></span>
    <input
      type="range"
      min={PARTY_MINUTES_RANGE.min}
      max={PARTY_MINUTES_RANGE.max}
      step={PARTY_MINUTES_RANGE.step}
      value={brief.minutes}
      {disabled}
      aria-valuetext={partyLength(brief.minutes, true)}
      oninput={(e) => onchange({ minutes: Number(e.currentTarget.value) })}
    />
  </label>

  <div class="more">
    <button type="button" class="toggle" aria-expanded={more} aria-controls="party-more" onclick={() => (more = !more)}>
      <span class="chev" class:open={more}><Icon name="chevron" size={16} /></span>
      <span class="label">More details</span>
      {#if !more}<span class="muted summary">{summary}</span>{/if}
    </button>
    {#if more}
      <div class="details" id="party-more">
        <div class="q">
          <span class="label" id="q-ages">Ages <span class="muted hint">Pick all that apply</span></span>
          <div class="chips" role="group" aria-labelledby="q-ages">
            {#each AGES as a (a.id)}
              {@const on = brief.ages.includes(a.id)}
              <button type="button" class:selected={on} aria-pressed={on} {disabled} onclick={() => toggleAge(a.id)}>{@render check(on)}{a.label}</button>
            {/each}
          </div>
        </div>
        <div class="q">
          <span class="label" id="q-energy">Energy</span>
          <Segmented options={ENERGY_CURVES} value={brief.curve} labelledby="q-energy" onchange={(curve) => onchange({ curve })} />
        </div>
        <div class="q">
          <span class="label" id="q-explicit">Explicit lyrics</span>
          <Segmented options={EXPLICIT} value={brief.allow_explicit ? "allow" : "avoid"} labelledby="q-explicit" onchange={(id) => onchange({ allow_explicit: id === "allow" })} />
        </div>
        <label class="q">
          <span class="label">Anything else? <span class="muted hint">(optional)</span></span>
          <input
            type="text"
            maxlength={MAX_NOTES_CHARS}
            placeholder="Artists everyone loves, a language, songs to avoid…"
            aria-label="Anything else"
            value={brief.notes}
            {disabled}
            autocomplete="off"
            oninput={(e) => onchange({ notes: e.currentTarget.value })}
          />
        </label>
      </div>
    {/if}
  </div>
</div>

<style>
  .questions {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .q {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .label {
    font-weight: 600;
  }
  .label.row {
    display: flex;
    justify-content: space-between;
  }
  .label b {
    font-weight: 500;
    color: var(--muted);
  }
  .hint {
    font-size: 12.5px;
    font-weight: 400;
    margin-left: 6px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .chips > button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 36px;
    padding: 0 14px;
    border: 1px solid var(--set-ctl-border);
    border-radius: var(--r-pill);
    background: var(--set-ctl-bg);
    box-shadow: var(--set-ctl-shadow);
    font: var(--set-label-font);
    transition: background 0.2s, border-color 0.2s, box-shadow 0.2s;
  }
  .chips > button:hover:not(.selected):not(:disabled) {
    border-color: var(--set-ctl-hover);
  }
  .chips > button.selected {
    background: var(--set-sel-bg);
    border-color: var(--set-sel-border);
    box-shadow: var(--set-sel-shadow);
    color: var(--set-sel-fg);
    font-weight: 600;
  }
  .chips > button:disabled {
    opacity: 0.55;
  }
  .q :global(.seg) {
    justify-content: flex-start;
  }
  input[type="range"] {
    accent-color: var(--primary-strong);
    width: 100%;
  }
  input[type="text"] {
    height: 40px;
    padding: 0 12px;
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    background: var(--surface-2);
    font-size: 15px;
  }
  .more {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 36px;
    text-align: left;
  }
  .chev {
    display: inline-grid;
    transition: transform 0.15s;
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .summary {
    font-size: 12.5px;
    min-width: 0;
  }
  .details {
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding-left: 22px;
  }
  @media (prefers-reduced-motion: reduce) {
    .chev {
      transition: none;
    }
  }
</style>
