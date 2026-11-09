<script lang="ts">
  // The middle column: EQ (High / Mid / Low with kill), filter and trim per deck, the channel
  // faders with their meters, the Mix button, the crossfader and its curve.
  import { dj } from "../../lib/dj.svelte";
  import { DECKS, deckIndex, DJ_EQ_KILL_DB, DJ_TRIM_DB, EQ_BAND_INDEX, type CrossfaderCurve, type DeckId, type DjEqBand } from "../../lib/dj-api";
  import DjFader from "./DjFader.svelte";
  import DjKnob from "./DjKnob.svelte";
  import DjMixButton from "./DjMixButton.svelte";
  import DjVu from "./DjVu.svelte";

  /** Filter knob values this close to 0 do nothing (homefi_core FILTER_DEADZONE). */
  const FILTER_DEADZONE = 0.03;
  const CROSSFADER_CENTER = 0.5;
  const CURVES: { id: CrossfaderCurve; label: string }[] = [
    { id: "smooth", label: "Smooth" },
    { id: "linear", label: "Linear" },
    { id: "cut", label: "Cut" },
  ];
  const BANDS: { band: DjEqBand; label: string; plain: string }[] = [
    { band: "high", label: "High", plain: "Treble" },
    { band: "mid", label: "Mid", plain: "Voice" },
    { band: "low", label: "Low", plain: "Bass" },
  ];

  const simple = $derived(dj.settings.simple_view);
  const boost = $derived(dj.settings.eq_max_boost_db);
  const deckColor = (d: DeckId) => (d === "a" ? "var(--dj-a)" : "var(--dj-b)");
  const deck = (d: DeckId) => dj.deck(d);

  // EQ knob: left half down to a kill (squared, so the useful range is wide), right half up to the boost
  const eqToKnob = (db: number) => (db < 0 ? -Math.sqrt(Math.min(1, db / DJ_EQ_KILL_DB)) : boost > 0 ? db / boost : 0);
  const knobToEq = (v: number) => (v < 0 ? DJ_EQ_KILL_DB * v * v : v * boost);
  const dbText = (db: number) => (db <= DJ_EQ_KILL_DB + 0.5 ? "Off" : `${db > 0 ? "+" : ""}${db.toFixed(1)} dB`);
  const filterText = (v: number) => (Math.abs(v) < FILTER_DEADZONE ? "Off" : v < 0 ? `Muffled ${Math.round(-v * 100)} %` : `Thin ${Math.round(v * 100)} %`);
  const pctText = (v: number) => `${Math.round(v * 100)} %`;
  const xfText = (v: number) => (Math.abs(v - CROSSFADER_CENTER) < 0.01 ? "Centre" : v < CROSSFADER_CENTER ? `Left ${Math.round((CROSSFADER_CENTER - v) * 200)} %` : `Right ${Math.round((v - CROSSFADER_CENTER) * 200)} %`);
  const peak = (d: DeckId) => Math.max(...(deck(d)?.peak ?? [0, 0]));
</script>

<section class="mixer" aria-label="Mixer">
  <div class="mx">
  <h2>Mixer</h2>
  <div class="eqs">
    {#each BANDS as b (b.band)}
      {@const k = EQ_BAND_INDEX[b.band]}
      {#each DECKS as d, n (d)}
        {#if n === 1}<span class="lab" title="{b.plain}. Turn down what you don't want.">{b.label}</span>{/if}
        {@const s = deck(d)}
        {@const killed = !!s?.eq_kill[k]}
        <div class="kn" class:b={d === "b"}>
          <DjKnob
            value={eqToKnob(s?.eq_db[k] ?? 0)}
            min={-1}
            max={1}
            detent={0}
            dim={killed}
            color={deckColor(d)}
            label="{b.label}, deck {d.toUpperCase()}"
            tip="{b.plain}. Turn down what you don't want."
            valueText={(v) => (killed ? "Killed" : dbText(knobToEq(v)))}
            onchange={(v) => dj.send({ type: "eq", deck: d, band: b.band, db: knobToEq(v) })}
          />
          <button class="kill" class:on={killed} type="button" aria-pressed={killed} aria-label="Kill {b.plain.toLowerCase()}, deck {d.toUpperCase()}" title="Silence this band completely" onclick={() => dj.send({ type: "eq_kill", deck: d, band: b.band, on: !killed })}>KILL</button>
        </div>
      {/each}
    {/each}
    {#each DECKS as d, n (d)}
      {#if n === 1}<span class="lab" title="Left cuts the high end (muffled), right cuts the bass (thin). Centre is off.">Filter</span>{/if}
      <div class="kn">
        <DjKnob
          value={deck(d)?.filter ?? 0}
          min={-1}
          max={1}
          detent={0}
          color={deckColor(d)}
          label="Filter, deck {d.toUpperCase()}"
          tip="Left cuts the high end (muffled), right cuts the bass (thin). Centre is off."
          valueText={filterText}
          onchange={(value) => dj.send({ type: "filter", deck: d, value })}
        />
      </div>
    {/each}
    {#if !simple}
      {#each DECKS as d, n (d)}
        {#if n === 1}<span class="lab quiet" title="Input volume of this song. Use it if one is louder than the other.">Trim</span>{/if}
        <div class="kn">
          <DjKnob
            value={deck(d)?.trim_db ?? 0}
            min={DJ_TRIM_DB.min}
            max={DJ_TRIM_DB.max}
            detent={0}
            size={36}
            color="var(--muted)"
            label="Trim, deck {d.toUpperCase()}"
            tip="Input volume of this song. Use it if one is louder than the other."
            valueText={(db) => `${db > 0 ? "+" : ""}${db.toFixed(1)} dB`}
            onchange={(db) => dj.send({ type: "trim", deck: d, db })}
          />
        </div>
      {/each}
    {/if}
  </div>

  <div class="chs">
    {#each DECKS as d (d)}
      <div class="ch" class:b={d === "b"} style:--c={deckColor(d)}>
        <DjFader value={deck(d)?.volume ?? 1} reset={1} label="Volume, deck {d.toUpperCase()}" tip="Volume of this deck" valueText={pctText} onchange={(value) => dj.send({ type: "volume", deck: d, value })} />
        <DjVu level={peak(d)} label="Level, deck {d.toUpperCase()}" />
      </div>
    {/each}
  </div>

  <div class="mixslot"><DjMixButton /></div>

  <div class="xf">
    <DjFader
      horizontal
      fill={false}
      value={dj.snap?.crossfader ?? CROSSFADER_CENTER}
      reset={CROSSFADER_CENTER}
      label="Crossfader"
      tip="Blends between deck A (left) and deck B (right)"
      valueText={xfText}
      onchange={(value) => dj.send({ type: "crossfader", value })}
    />
    <div class="curve">
      <span class="cap-t" title="How the crossfader blends: Smooth, Linear or Cut (sharp, for scratching)">Curve</span>
      <div class="seg" role="radiogroup" aria-label="Crossfader curve">
        {#each CURVES as c (c.id)}
          {@const on = (dj.snap?.crossfader_curve ?? dj.settings.crossfader_curve) === c.id}
          <button type="button" role="radio" aria-checked={on} class:on title="How the crossfader blends: {c.label}" onclick={() => dj.saveSettings({ crossfader_curve: c.id })}>{c.label}</button>
        {/each}
      </div>
    </div>
  </div>
  </div>
</section>

<style>
  .mixer {
    display: flex;
    flex-direction: column;
    container: mixer / inline-size;
    min-width: 0;
    min-height: 0;
    padding: 12px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
  }
  .mx {
    display: flex;
    flex-direction: column;
    gap: 10px;
    flex: 1;
    min-height: 0;
  }
  /* On a wide band (the board is narrower than three columns): EQ | faders | Mix and crossfader, side by side */
  @container mixer (min-width: 540px) {
    .mx {
      display: grid;
      grid-template-columns: minmax(0, 1.25fr) minmax(0, 0.7fr) minmax(0, 1.15fr);
      grid-template-rows: auto 1fr auto;
      grid-template-areas:
        "title title title"
        "eqs chs mix"
        "eqs chs xf";
      gap: 8px 18px;
    }
    .mx > h2 {
      grid-area: title;
    }
    .eqs {
      grid-area: eqs;
      align-content: center;
    }
    .chs {
      grid-area: chs;
    }
    .mixslot {
      grid-area: mix;
      align-self: end;
    }
    .xf {
      grid-area: xf;
    }
  }
  h2 {
    margin: 0;
    font-size: 11px;
    color: var(--faint);
    text-transform: uppercase;
    letter-spacing: 0.1em;
    text-align: center;
  }
  .eqs {
    display: grid;
    grid-template-columns: 1fr 44px 1fr;
    gap: 6px 4px;
    align-items: center;
  }
  .lab {
    text-align: center;
    font-size: 11px;
    color: var(--muted);
  }
  .lab.quiet {
    color: var(--faint);
  }
  .kn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
  }
  .kn.b {
    flex-direction: row-reverse;
  }
  .kill {
    min-height: 28px;
    padding: 0 8px;
    border-radius: var(--r-pill);
    border: 1px solid var(--border);
    background: var(--surface-2);
    font-size: 10px;
    font-weight: 700;
  }
  .kill.on {
    background: var(--danger);
    color: #2b0505;
    border-color: transparent;
  }
  .chs {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
    flex: 1;
    min-height: 110px;
  }
  .ch {
    display: flex;
    gap: 6px;
    justify-content: center;
  }
  .ch.b {
    flex-direction: row-reverse;
  }
  .xf {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding-top: 8px;
    border-top: 1px solid var(--border);
  }
  .curve {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .cap-t {
    font-size: 10px;
    color: var(--faint);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .seg {
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: var(--r-pill);
    overflow: hidden;
  }
  .seg button {
    min-height: 32px;
    padding: 0 10px;
    font-size: 12px;
    color: var(--muted);
  }
  .seg button.on {
    background: var(--surface-3);
    color: var(--text);
  }
</style>
