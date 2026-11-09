<script lang="ts">
  // Setup (docs/EXERCISE_UI.md §3.1): how long, what kind of ride, what to hear and where to watch;
  // the live drawing of the course on the right. Nothing is built until "Build my ride".
  import { EX_MAX_SENTENCE_CHARS, EX_MINUTES, STYLE_INFO, sketchOf } from "../../lib/exercise-api";
  import { exercise } from "../../lib/exercise.svelte";
  import { app } from "../../lib/state.svelte";
  import ExCard from "./ExCard.svelte";
  import ExSegmented from "./ExSegmented.svelte";
  import ExerciseChat from "./ExerciseChat.svelte";
  import GenreChips from "./GenreChips.svelte";
  import Mountain from "./Mountain.svelte";
  import SavedRides from "./SavedRides.svelte";
  import StylePicker from "./StylePicker.svelte";
  import WhereToWatch from "./WhereToWatch.svelte";
  import Icon from "../Icon.svelte";

  const NEEDS_AI = "Needs your AI key. Genres still work.";
  const req = $derived(exercise.request);
  const sketch = $derived(sketchOf(req.style, req.minutes));
  const style = $derived(STYLE_INFO[req.style]);
  const sliderKeyPage = (e: KeyboardEvent) => {
    if (e.key !== "PageUp" && e.key !== "PageDown") return;
    e.preventDefault();
    exercise.setMinutes(req.minutes + (e.key === "PageUp" ? EX_MINUTES.page : -EX_MINUTES.page));
  };
  const tabs = $derived([
    { id: "new" as const, label: "New ride" },
    { id: "rides" as const, label: exercise.rides.length ? `My rides · ${exercise.rides.length}` : "My rides" },
  ]);
  const goAi = () => (app.view = { name: "settings", page: "ai" });
</script>

<div class="ex">
  <header>
    <div class="head">
      <h1>Exercise</h1>
      <ExSegmented label="Exercise" options={tabs} value={exercise.setupTab} onchange={(id) => (exercise.setupTab = id)} />
    </div>
    {#if exercise.setupTab === "new"}
      <p class="sub">Choose how long, what kind of ride and what you want to hear. HomeFi builds the ride and the music.</p>
    {/if}
  </header>

  {#if exercise.setupTab === "rides"}
    <SavedRides />
  {:else}
  <div class="grid">
    <div class="col">
      <ExCard title="Length">
        <div class="len">
          <span class="big">{req.minutes}</span>
          <span class="unit">min</span>
          <span class="sp"></span>
          <div class="chips" role="group" aria-label="Quick lengths">
            {#each EX_MINUTES.chips as m}
              <button type="button" class="chip" aria-pressed={req.minutes === m} onclick={() => exercise.setMinutes(m)}>{m}</button>
            {/each}
          </div>
        </div>
        <input
          type="range"
          min={EX_MINUTES.min}
          max={EX_MINUTES.max}
          step={EX_MINUTES.step}
          value={req.minutes}
          aria-label="Ride length in minutes"
          aria-valuetext="{req.minutes} minutes"
          oninput={(e) => exercise.setMinutes(Number(e.currentTarget.value))}
          onkeydown={sliderKeyPage}
        />
        <div class="ends"><span>{EX_MINUTES.min}</span><span class="sp"></span><span>{EX_MINUTES.max} min</span></div>
      </ExCard>

      <ExCard title="Style">
        <StylePicker value={req.style} minutes={req.minutes} recommended={exercise.firstRideRecommended ? "first_ride" : null} onchange={(id) => (req.style = id)} />
      </ExCard>

      <ExCard title="Music">
        <span class="lbl">Genres for today (pick one or more)</span>
        <GenreChips picked={req.wish.genres} ontoggle={(g) => exercise.toggleGenre(g)} onclear={() => exercise.clearGenres()} />

        {#if req.wish.genres.length >= 2}
          <div class="mix">
            <button type="button" class="switch" role="switch" aria-checked={req.wish.mix} aria-labelledby="ex-mix-l" onclick={() => (req.wish.mix = !req.wish.mix)}><span class="knob"></span></button>
            <div>
              <b id="ex-mix-l">Mix the genres</b>
              <small>Each genre goes where its tempo fits: fast for sprints, heavy for climbs, calm for warm-up.</small>
            </div>
          </div>
        {/if}

        <label class="lbl sentence" for="ex-sentence">Or tell the AI in a sentence (optional)</label>
        <input
          id="ex-sentence"
          class="field"
          value={req.wish.sentence}
          placeholder="Classic rock for the climbs, something heavy for the sprints"
          maxlength={EX_MAX_SENTENCE_CHARS}
          disabled={!exercise.aiOn}
          title={exercise.aiOn ? undefined : NEEDS_AI}
          oninput={(e) => exercise.setSentence(e.currentTarget.value)}
        />
        <div class="ai">
          {#if exercise.aiOn}
            <span class="tag"><Icon name="sparkles" size={13} />AI on: {exercise.aiName}</span>
            {#if !exercise.chatOpen}<button type="button" class="link" onclick={() => exercise.openChat()}>Talk it through with the AI</button>{/if}
          {:else}
            <span class="tag off">No AI: genres only</span>
            <button type="button" class="link" onclick={goAi}>Set up AI</button>
          {/if}
        </div>
        {#if exercise.chatOpen}<ExerciseChat />{/if}
      </ExCard>
    </div>

    <div class="col">
      <ExCard title="Your ride">
        <Mountain parts={sketch.parts} profile={sketch.profile} mode="summary" height={120} label="Sketch of the {style.name} course at {req.minutes} minutes" />
        <p class="sum">{exercise.summary}</p>
        <p class="desc">
          {style.line}.
          {#if exercise.settings.beginner_mode}Beginner mode keeps the pace and effort gentle.{/if}
        </p>
      </ExCard>

      <WhereToWatch />

      {#if !app.account}
        <p class="note" role="status"><Icon name="info" size={15} />TIDAL is offline. Only your library will be used.</p>
      {/if}
      <div class="go">
        <button type="button" class="build" onclick={() => exercise.build()}>Build my ride</button>
        <small>Takes about 20 to 30 seconds.</small>
      </div>
    </div>
  </div>
  {/if}
</div>

<style>
  .ex {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 0 0 20px;
    min-width: 0;
  }
  h1 {
    font-size: 26px;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
  }
  .sub {
    margin: 4px 0 0;
    color: var(--muted);
  }
  .grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 380px;
    gap: 16px;
    align-items: start;
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-width: 0;
  }
  .len {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .len .chips {
    align-self: center;
  }
  .big {
    font-size: 56px;
    font-weight: 800;
    line-height: 1;
    font-variant-numeric: tabular-nums;
  }
  .unit {
    font-size: 20px;
    color: var(--muted);
    font-weight: 700;
  }
  .sp {
    flex: 1;
  }
  .chips {
    display: flex;
    gap: 8px;
  }
  .chip {
    min-height: 40px;
    min-width: 52px;
    padding: 0 16px;
    border-radius: var(--r-pill);
    border: 1px solid var(--border);
    background: var(--surface-2);
    font-weight: 600;
  }
  .chip:hover:not([aria-pressed="true"]) {
    background: var(--surface-3);
  }
  .chip[aria-pressed="true"] {
    background: var(--primary);
    color: var(--on-primary);
    border-color: transparent;
  }
  input[type="range"] {
    width: 100%;
    height: 44px;
    margin: 6px 0 0;
    padding: 0;
    border: 0;
    background: none;
    accent-color: var(--primary);
  }
  .ends {
    display: flex;
    color: var(--muted);
    font-size: 12px;
  }
  .lbl {
    display: block;
    margin: 0 0 8px;
    font-size: 13px;
    font-weight: 600;
    color: var(--muted);
  }
  .sentence {
    margin-top: 16px;
  }
  .mix {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: 14px;
  }
  .mix small {
    display: block;
    color: var(--muted);
    line-height: 1.35;
  }
  .switch {
    flex: none;
    width: 46px;
    height: 28px;
    border-radius: 99px;
    background: var(--track);
    position: relative;
    transition: background 0.2s;
  }
  .switch[aria-checked="true"] {
    background: var(--primary);
  }
  .knob {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: #fff;
    transition: left 0.2s;
  }
  .switch[aria-checked="true"] .knob {
    left: 21px;
  }
  .field {
    width: 100%;
    min-height: 48px;
    padding: 0 14px;
    user-select: text;
  }
  .field:disabled {
    opacity: 0.55;
  }
  .ai {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: 10px;
  }
  .tag {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    min-height: 24px;
    padding: 0 10px;
    border-radius: var(--r-pill);
    font-size: 12px;
    font-weight: 700;
    background: var(--ai-tag-bg);
    color: var(--ai-tag-text);
  }
  .tag.off {
    background: var(--surface-3);
    color: var(--muted);
  }
  .link {
    color: var(--primary);
    font-weight: 600;
    font-size: 13px;
    min-height: 28px;
  }
  .link:hover {
    text-decoration: underline;
  }
  .sum {
    margin: 10px 0 2px;
    font-weight: 700;
    font-size: 16px;
  }
  .desc {
    margin: 0;
    color: var(--muted);
    font-size: 13px;
    line-height: 1.4;
  }
  .note {
    margin: 0;
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 10px 12px;
    border-radius: var(--r-md);
    background: var(--warn-bg);
    border: 1px solid var(--warn-border);
    color: var(--warn);
    font-size: 13px;
  }
  .go {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
  }
  .go small {
    color: var(--muted);
  }
  .build {
    width: 100%;
    height: 56px;
    border-radius: var(--r-pill);
    background: var(--primary-strong);
    color: var(--on-primary-strong);
    font-size: 16px;
    font-weight: 700;
    transition: filter 0.15s;
  }
  .build:hover {
    filter: brightness(1.12);
  }
</style>
