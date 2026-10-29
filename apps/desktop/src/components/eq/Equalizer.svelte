<script lang="ts">
  // The equalizer screen: a profile per output (and a default for the rest),
  // an editable curve, the band list, preamp, presets, AutoEq headphone
  // corrections, taste sliders and "Hear without EQ". Every change is applied
  // live a moment later (lib/eq.svelte.ts).
  import { onDestroy, untrack } from "svelte";
  import { api, EQ_LIMITS, errorText, outputIcon, type EqBandKind, type EqProfile, type EqUiSettings, type HeadphoneEntry, type Tilt } from "../../lib/api";
  import { OTHER_OUTPUTS, clamp, copyProfile, eq, flatProfile, formatDb, newBand, sourceLabel } from "../../lib/eq.svelte";
  import { pop, rise } from "../../lib/motion";
  import { app } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";
  import BandList from "./BandList.svelte";
  import EqGraph from "./EqGraph.svelte";
  import HeadphoneSearch from "./HeadphoneSearch.svelte";
  import TasteRules from "../sound/TasteRules.svelte";
  import { sound } from "../../lib/sound.svelte";

  const FLAT = flatProfile();
  const UI_DEFAULTS: EqUiSettings = { deck_button: true, ab_mode: "hold", band_labels: true, taste_sliders: true };
  const ui = $derived({ ...UI_DEFAULTS, ...(app.settings?.eq_ui ?? {}) });

  let tab = $state<"device" | "taste">("device");
  $effect(() => {
    const v = app.view;
    untrack(() => {
      eq.load(v.name === "eq" ? v.output : undefined);
      tab = v.name === "eq" && v.tab === "taste" ? "taste" : "device";
    });
  });
  // saved elsewhere (Settings → Sound, the assistant): shown here too
  $effect(() => {
    const s = app.settings?.eq;
    untrack(() => eq.sync(s));
  });

  const s = $derived(eq.settings);
  const own = $derived(s ? eq.own(eq.target) : null);
  const profile = $derived(s ? eq.effective(eq.target) : null);
  const targetLabel = $derived(eq.target === OTHER_OUTPUTS ? "other outputs" : eq.target);
  const playingOn = $derived(eq.playingOn);
  let selected = $state<number | null>(null);

  // the target changed: nothing is selected, the taste starts again
  $effect(() => {
    eq.target;
    untrack(() => {
      selected = null;
      resetTaste();
    });
  });

  // ------------------------------------------------------------ status
  const playingActive = $derived(!!app.player?.eq_active);
  // "HD 650 + Electronic": the output's profile and the taste layer playing
  const layers = $derived.by(() => {
    const p = app.player;
    const rule = p?.taste_rule ? sound.taste.rules.find((r) => r.id === p.taste_rule) : null;
    return [p?.eq_profile, rule?.scope.name ?? p?.taste_label].filter(Boolean).join(" + ") || profile?.name || null;
  });
  let bypass = $state(false);
  const status = $derived.by(() => {
    const dev = playingOn ?? "this computer";
    if (bypass) return { tone: "ok", text: `Hearing without EQ: bit-perfect on ${dev}. ${ui.ab_mode === "hold" ? "Let go" : "Click again"} to hear it with EQ.` };
    if (!s?.enabled) return { tone: "ok", text: `Off · bit-perfect: the music reaches ${dev} untouched.` };
    if (playingActive) return { tone: "warn", text: `On · “${layers ?? "EQ"}” on ${dev}. The sound is processed, so it isn't bit-perfect.` };
    return { tone: "ok", text: `On, but flat for ${dev}: nothing is changed, still bit-perfect.` };
  });

  function setBypass(on: boolean) {
    if (bypass === on) return;
    bypass = on;
    api.eqBypass(on).catch((e) => (eq.error = errorText(e)));
  }
  onDestroy(() => {
    if (bypass) api.eqBypass(false);
    eq.flush();
  });

  // ------------------------------------------------------------ edits
  function editBand(i: number, patch: Partial<{ freqHz: number; gainDb: number; q: number; kind: EqBandKind; enabled: boolean }>) {
    resetTaste();
    eq.edit((p) => {
      const b = p.bands[i];
      if (!b) return;
      if (patch.freqHz !== undefined) b.freqHz = Math.round(clamp(patch.freqHz, EQ_LIMITS.freqHz));
      if (patch.gainDb !== undefined) b.gainDb = Math.round(clamp(patch.gainDb, EQ_LIMITS.gainDb) * 10) / 10;
      if (patch.q !== undefined) b.q = Math.round(clamp(patch.q, EQ_LIMITS.q) * 100) / 100;
      if (patch.kind !== undefined) b.kind = patch.kind;
      if (patch.enabled !== undefined) b.enabled = patch.enabled;
    });
  }
  function addBand(freqHz?: number, gainDb = 0) {
    if ((profile?.bands.length ?? 0) >= EQ_LIMITS.maxBands) return;
    resetTaste();
    eq.edit((p) => {
      p.bands.push(newBand(p.bands, freqHz, gainDb));
      selected = p.bands.length - 1;
    });
  }
  function removeBand(i: number) {
    resetTaste();
    eq.edit((p) => p.bands.splice(i, 1));
    selected = null;
  }
  const full = $derived((profile?.bands.length ?? 0) >= EQ_LIMITS.maxBands);

  // ------------------------------------------------------------ preamp
  let autoValue = $state(0);
  $effect(() => {
    const p = profile ? copyProfile(profile) : null;
    if (!p) return void (autoValue = 0);
    api.eqAutoPreamp(p).then((v) => (autoValue = v)).catch(() => {});
  });
  const preamp = $derived(s?.autoPreamp ? autoValue : (profile?.preampDb ?? 0));

  // ------------------------------------------------------------ taste
  const ZERO: Required<Tilt> = { bassDb: 0, trebleDb: 0, clarityDb: 0 };
  let tilt = $state<Required<Tilt>>({ ...ZERO });
  let tasteBase: EqProfile | null = null;
  let tasteSeq = 0;
  function resetTaste() {
    tasteBase = null;
    tilt = { ...ZERO };
  }
  async function setTaste(key: keyof Tilt, value: number) {
    tasteBase ??= copyProfile(profile ?? flatProfile());
    tilt = { ...tilt, [key]: value };
    const my = ++tasteSeq;
    try {
      const next = await api.eqWithTaste(tasteBase, tilt);
      if (my === tasteSeq) eq.setProfile(next);
    } catch (e) {
      eq.error = errorText(e);
    }
  }
  const TASTE: { key: keyof Tilt; label: string; hint: string }[] = [
    { key: "bassDb", label: "Bass", hint: "Warmth and punch, below 150 Hz" },
    { key: "trebleDb", label: "Treble", hint: "Air and sparkle, above 8 kHz" },
    { key: "clarityDb", label: "Clarity", hint: "Voices and detail, around 3 kHz" },
  ];

  // ------------------------------------------------------------ presets
  const presets = $derived(app.settings?.eq_presets ?? []);
  let presetsOpen = $state(false);
  let saving = $state<string | null>(null);
  let renaming = $state<{ index: number; name: string } | null>(null);

  function usePreset(p: EqProfile) {
    resetTaste();
    selected = null;
    eq.setProfile(copyProfile(p));
    presetsOpen = false;
  }
  async function savePresets(list: EqProfile[]) {
    await eq.flush();
    await app.saveSettings({ eq: eq.settings ?? app.settings!.eq, eq_presets: list });
  }
  async function saveAsPreset() {
    const name = saving?.trim();
    if (!name || !profile) return;
    const p = { ...copyProfile(profile), name };
    const list = presets.filter((x) => x.name !== name);
    await savePresets([...list, p]);
    eq.edit((x) => (x.name = name));
    saving = null;
    app.note(`Saved the preset “${name}”`, "success");
  }
  async function renamePreset() {
    if (!renaming) return;
    const name = renaming.name.trim();
    const i = renaming.index;
    renaming = null;
    if (!name) return;
    await savePresets(presets.map((p, k) => (k === i ? { ...p, name } : p)));
  }
  async function deletePreset(i: number) {
    presetsOpen = false;
    const ok = await app.confirm({ title: `Delete the preset “${presets[i].name}”?`, text: "Profiles already using it keep their curve.", confirmLabel: "Delete", danger: true });
    if (ok) await savePresets(presets.filter((_, k) => k !== i));
  }

  // ------------------------------------------------------------ AutoEq
  function applyHeadphones(p: EqProfile, entry: HeadphoneEntry) {
    resetTaste();
    selected = null;
    eq.setProfile(p);
    if (!s?.enabled) eq.update((x) => (x.enabled = true));
    app.note(`${entry.name} corrected for ${targetLabel}`, "success");
  }

  const targets = $derived([...eq.outputs.map((o) => ({ ...o, id: o.name, label: o.name })), { id: OTHER_OUTPUTS, name: "", label: "Other outputs", connected: true }]);
</script>

{#if s}
<div class="eq">
  <header class="head">
    <div class="titles">
      <h1>Equalizer</h1>
      <p class="status {status.tone}" aria-live="polite"><span class="led"></span>{status.text}</p>
    </div>
    <div class="actions">
      <button class="btn tonal" onclick={() => (app.view = { name: "assistant" })}><Icon name="ai" size={16} /> Set up my sound</button>
      <label class="switch" title={s.enabled ? "Turn the equalizer off" : "Turn the equalizer on"}>
        <input type="checkbox" role="switch" checked={s.enabled} onchange={(e) => eq.update((x) => (x.enabled = e.currentTarget.checked))} />
        <span class="track"><span class="thumb"></span></span>
        <b>{s.enabled ? "On" : "Off"}</b>
      </label>
    </div>
  </header>

  <div class="tabs" role="tablist" aria-label="Equalizer">
    <button role="tab" aria-selected={tab === "device"} class:on={tab === "device"} onclick={() => (tab = "device")}><Icon name="headphones" size={15} /> Your outputs</button>
    <button role="tab" aria-selected={tab === "taste"} class:on={tab === "taste"} onclick={() => (tab = "taste")}>
      <Icon name="layers" size={15} /> Taste rules
      {#if sound.taste.rules.length}<span class="count">{sound.taste.rules.length}</span>{/if}
    </button>
  </div>

  {#if tab === "taste"}
    <TasteRules />
  {:else}
  <section class="outputs" aria-label="Output">
    <span class="cap">Profile for</span>
    <div class="chips" role="radiogroup">
      {#each targets as t (t.id)}
        {@const has = t.id === OTHER_OUTPUTS ? !!s.defaultProfile : !!s.perOutput[t.id]}
        <button role="radio" aria-checked={eq.target === t.id} class:on={eq.target === t.id} class:away={!t.connected} onclick={() => (eq.target = t.id)} title={t.connected ? t.label : `${t.label} (not connected)`}>
          <Icon name={t.id === OTHER_OUTPUTS ? "speaker" : outputIcon(t.id)} size={14} />
          <span class="ellipsis">{t.label}</span>
          {#if t.id === playingOn}<span class="now" title="Playing here now"></span>{/if}
          {#if has}<span class="own" title="Has its own profile">●</span>{/if}
        </button>
      {/each}
    </div>
  </section>

  <div class="layout">
    <div class="main">
      <section class="card">
        <div class="profile-bar">
          <div class="pname">
            {#if profile}
              <input class="name" value={profile.name} aria-label="Profile name" onchange={(e) => eq.edit((p) => (p.name = e.currentTarget.value.trim() || p.name))} />
              <span class="badge">{sourceLabel(profile)}</span>
            {:else}
              <b>Flat</b>
            {/if}
          </div>
          <div class="presets">
            <button class="btn tonal" aria-expanded={presetsOpen} onclick={() => (presetsOpen = !presetsOpen)}>Presets <Icon name="down" size={14} /></button>
            {#if presetsOpen}
              <button class="scrim" aria-label="Close" onclick={() => ((presetsOpen = false), (saving = null), (renaming = null))}></button>
              <div class="menu" role="menu" transition:pop={{ origin: "top right" }}>
                <button class="row" role="menuitem" onclick={() => usePreset(flatProfile())}><Icon name="minus" size={16} /><span class="grow">Flat</span><small class="muted">no change</small></button>
                {#each presets as p, i (p.name + i)}
                  <div class="row preset">
                    {#if renaming?.index === i}
                      <!-- svelte-ignore a11y_autofocus -->
                      <input class="grow" bind:value={renaming.name} autofocus onkeydown={(e) => (e.key === "Enter" ? renamePreset() : e.key === "Escape" && (renaming = null))} onblur={renamePreset} aria-label="Preset name" />
                    {:else}
                      <button class="grow apply" role="menuitem" onclick={() => usePreset(p)}><Icon name="eq" size={16} /> <span class="ellipsis">{p.name}</span></button>
                      <button class="icon-btn mini" title="Rename" aria-label="Rename {p.name}" onclick={() => (renaming = { index: i, name: p.name })}><Icon name="edit" size={14} /></button>
                      <button class="icon-btn mini" title="Delete" aria-label="Delete {p.name}" onclick={() => deletePreset(i)}><Icon name="trash" size={14} /></button>
                    {/if}
                  </div>
                {/each}
                <div class="sep"></div>
                {#if saving !== null}
                  <form class="row save" onsubmit={(e) => (e.preventDefault(), saveAsPreset())}>
                    <!-- svelte-ignore a11y_autofocus -->
                    <input class="grow" bind:value={saving} placeholder="Preset name" maxlength="60" autofocus aria-label="Preset name" />
                    <button class="btn primary small" disabled={!saving.trim()}>Save</button>
                  </form>
                {:else}
                  <button class="row" role="menuitem" disabled={!profile} onclick={() => (saving = profile?.name ?? "")}><Icon name="save" size={16} /><span class="grow">Save as preset…</span></button>
                {/if}
              </div>
            {/if}
          </div>
        </div>

        {#if eq.target !== OTHER_OUTPUTS && !own}
          <p class="hint"><Icon name="info" size={14} /> {eq.target} uses the profile for other outputs ({s.defaultProfile?.name ?? "flat"}). Any change here gives it its own.</p>
        {:else if eq.target !== OTHER_OUTPUTS && own}
          <p class="hint"><Icon name="check" size={14} /> {eq.target} has its own profile. <button class="link" onclick={() => eq.setProfile(null)}>Use the one for other outputs instead</button></p>
        {:else}
          <p class="hint"><Icon name="info" size={14} /> Used by every output that doesn't have a profile of its own.</p>
        {/if}

        <EqGraph
          profile={profile ?? FLAT}
          editable
          bind:selected
          labels={ui.band_labels}
          dim={!s.enabled || bypass}
          onedit={(i, patch) => editBand(i, patch)}
          onadd={(f, g) => addBand(f, g)}
          onremove={removeBand}
        />
        <p class="tiny muted">Drag a point for frequency and gain · scroll or pinch over it for width (Q) · double-click to add a band · arrows and Delete on a selected point</p>

        <div class="controls">
          <div class="preamp">
            <span class="cap">Preamp</span>
            <label class="check"><input type="checkbox" checked={s.autoPreamp} onchange={(e) => eq.update((x) => (x.autoPreamp = e.currentTarget.checked))} /> Automatic</label>
            {#if s.autoPreamp}
              <b class="mono">{formatDb(preamp)}</b>
              <small class="muted">leaves room for the boosts, so nothing clips</small>
            {:else}
              <input type="range" min="-24" max="6" step="0.1" value={profile?.preampDb ?? 0} aria-label="Preamp" oninput={(e) => eq.edit((p) => (p.preampDb = Math.round(Number(e.currentTarget.value) * 10) / 10))} />
              <input class="num" type="number" min={EQ_LIMITS.preampDb[0]} max={EQ_LIMITS.preampDb[1]} step="0.1" value={profile?.preampDb ?? 0} aria-label="Preamp in dB" onchange={(e) => eq.edit((p) => (p.preampDb = clamp(Number(e.currentTarget.value), EQ_LIMITS.preampDb)))} />
              {#if (profile?.preampDb ?? 0) > autoValue + 0.05}<small class="warn-text">may clip: automatic would be {formatDb(autoValue)}</small>{/if}
            {/if}
          </div>
          <div class="buttons">
            {#if ui.ab_mode === "hold"}
              <button
                class="btn tonal ab"
                class:pressed={bypass}
                disabled={!s.enabled}
                title="Hold to hear the music without the equalizer"
                onpointerdown={() => setBypass(true)}
                onpointerup={() => setBypass(false)}
                onpointerleave={() => setBypass(false)}
                onkeydown={(e) => (e.key === " " || e.key === "Enter") && (e.preventDefault(), e.stopPropagation(), setBypass(true))}
                onkeyup={() => setBypass(false)}
                onblur={() => setBypass(false)}
              ><Icon name="ear" size={16} /> Hear without EQ</button>
            {:else}
              <button class="btn tonal ab" class:pressed={bypass} aria-pressed={bypass} disabled={!s.enabled} title="Click to compare with and without the equalizer" onclick={() => setBypass(!bypass)}><Icon name="ear" size={16} /> {bypass ? "Back to EQ" : "Hear without EQ"}</button>
            {/if}
            <button class="btn tonal" disabled={full} title={full ? `At most ${EQ_LIMITS.maxBands} bands` : "Add a band"} onclick={() => addBand()}><Icon name="plus" size={16} /> Band</button>
            <button class="btn tonal" disabled={!profile?.bands.length} onclick={() => usePreset({ ...flatProfile(profile?.name ?? "Flat") })}>Reset</button>
          </div>
        </div>
      </section>

      <section class="card">
        <h3>Bands <small class="muted">{profile?.bands.length ?? 0} of {EQ_LIMITS.maxBands}</small></h3>
        {#if !profile?.bands.length}
          <p class="muted small">No bands: the sound goes through unchanged. Add one, pick a preset, or find your headphones.</p>
        {:else}
          <BandList bands={profile.bands} bind:selected onedit={(i, patch) => editBand(i, patch)} onremove={removeBand} />
        {/if}
      </section>
    </div>

    <aside class="side">
      <section class="card">
        <h3><Icon name="headphones" size={16} /> Find my headphones</h3>
        <HeadphoneSearch target={targetLabel} onapply={applyHeadphones} />
      </section>

      {#if ui.taste_sliders}
        <section class="card" in:rise>
          <h3>Your taste</h3>
          <p class="muted small">On top of the profile; the preamp follows so it doesn't clip.</p>
          {#each TASTE as t}
            <label class="taste">
              <span><b>{t.label}</b> <span class="mono">{formatDb(tilt[t.key] ?? 0)}</span></span>
              <input type="range" min="-6" max="6" step="0.5" value={tilt[t.key] ?? 0} oninput={(e) => setTaste(t.key, Number(e.currentTarget.value))} />
              <small class="muted">{t.hint}</small>
            </label>
          {/each}
          <button class="link" onclick={resetTaste} disabled={!tilt.bassDb && !tilt.trebleDb && !tilt.clarityDb}>Set the sliders back to 0 (keeps the sound)</button>
        </section>
      {/if}

      <section class="card soft">
        <h3><Icon name="ai" size={16} /> Not sure where to start?</h3>
        <p class="muted small">Answer a few questions (or chat with your AI) and HomeFi builds the profile for you.</p>
        <button class="btn primary" onclick={() => (app.view = { name: "assistant" })}>Set up my sound</button>
        <button class="link" onclick={() => (app.view = { name: "settings", page: "sound" })}>Equalizer settings</button>
      </section>
    </aside>
  </div>
  {/if}
  {#if eq.error}<p class="error"><Icon name="alert" size={14} /> {eq.error}</p>{/if}
</div>
{/if}

<style>
  .eq {
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 1280px;
  }
  .head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
  }
  .status {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 6px 0 0;
    font-size: 13px;
    color: var(--muted);
  }
  .led {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--ok);
    box-shadow: 0 0 8px var(--ok);
  }
  .status.warn .led {
    background: #fbbf24;
    box-shadow: 0 0 8px #fbbf24;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  /* a big on / off switch */
  .switch {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
  }
  .switch input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }
  .track {
    position: relative;
    width: 46px;
    height: 26px;
    border-radius: min(999px, var(--r-max));
    background: var(--surface-3);
    border: 1px solid var(--border);
    transition: background 0.2s;
  }
  .thumb {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 20px;
    height: 20px;
    border-radius: min(999px, var(--r-max));
    background: var(--surface);
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.35);
    transition: transform calc(0.2s * var(--motion-speed)) var(--ease-pop);
  }
  .switch input:checked + .track {
    background: var(--primary-strong);
  }
  .switch input:checked + .track .thumb {
    transform: translateX(20px);
  }
  .switch input:focus-visible + .track {
    outline: 2px solid var(--primary);
    outline-offset: 2px;
  }
  .cap {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--faint);
  }
  .tabs {
    display: flex;
    gap: 4px;
    padding: 4px;
    width: fit-content;
    border-radius: min(14px, var(--r-max));
    background: var(--glass);
    border: 1px solid var(--glass-border);
  }
  .tabs button {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 34px;
    padding: 0 14px;
    border-radius: min(10px, var(--r-max));
    font-size: 13px;
    font-weight: 600;
    color: var(--muted);
    transition: background 0.15s, color 0.15s;
  }
  .tabs button:hover {
    color: var(--text);
  }
  .tabs button.on {
    color: var(--text);
    background: color-mix(in srgb, var(--primary) 16%, var(--surface));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--primary) 55%, transparent);
  }
  .tabs .count {
    min-width: 18px;
    height: 18px;
    padding: 0 5px;
    display: grid;
    place-items: center;
    border-radius: min(999px, var(--r-max));
    font-size: 10.5px;
    background: var(--primary-strong);
    color: var(--on-primary-strong);
  }
  .outputs {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chips button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    max-width: 240px;
    height: 32px;
    padding: 0 12px;
    border-radius: min(999px, var(--r-max));
    background: var(--glass-strong);
    border: 1px solid var(--glass-border);
    font-size: 12.5px;
    color: var(--muted);
  }
  .chips button.on {
    color: var(--text);
    border-color: var(--primary);
    box-shadow: inset 0 0 0 1px var(--primary);
    background: color-mix(in srgb, var(--primary) 14%, var(--surface));
  }
  .chips button.away {
    opacity: 0.6;
  }
  .chips .now {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--ok);
    box-shadow: 0 0 6px var(--ok);
  }
  .chips .own {
    font-size: 8px;
    color: var(--accent-text);
  }
  .layout {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 16px;
  }
  @container (min-width: 980px) {
    .layout {
      grid-template-columns: minmax(0, 1fr) 320px;
      align-items: start;
    }
  }
  .main,
  .side {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }
  .card {
    padding: 14px 16px;
    border-radius: min(16px, var(--r-max));
    background: var(--glass);
    border: 1px solid var(--glass-border);
    backdrop-filter: blur(18px) saturate(1.2);
  }
  .card.soft {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
  }
  .card.soft p {
    margin: 0;
  }
  h3 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 0 10px;
    font-size: 14px;
  }
  h3 small {
    font-weight: 400;
  }
  .profile-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .pname {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .name {
    min-width: 0;
    width: min(320px, 40vw);
    font: 700 17px var(--font-display);
    background: transparent;
    border-color: transparent;
    padding: 4px 6px;
    margin-left: -6px;
  }
  .name:hover,
  .name:focus {
    border-color: var(--border);
    background: var(--surface-2);
  }
  .presets {
    position: relative;
    flex: none;
  }
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 10;
    cursor: default;
  }
  .menu {
    position: absolute;
    right: 0;
    top: calc(100% + 6px);
    z-index: 11;
    width: 290px;
    max-height: 60vh;
    overflow-y: auto;
    padding: 6px;
    border-radius: min(14px, var(--r-max));
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
  }
  .menu .row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: 36px;
    padding: 0 10px;
    border-radius: min(9px, var(--r-max));
    font-size: 13px;
    text-align: left;
  }
  .menu button.row:hover:not(:disabled),
  .menu .preset:hover {
    background: var(--surface-2);
  }
  .menu .preset {
    padding: 0 4px 0 0;
  }
  .menu .apply {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    height: 36px;
    padding: 0 10px;
    text-align: left;
  }
  .menu .save {
    padding: 4px;
  }
  .menu input {
    min-width: 0;
    padding: 6px 8px;
  }
  .sep {
    height: 1px;
    margin: 4px 6px;
    background: var(--border);
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .icon-btn.mini {
    width: 28px;
    height: 28px;
  }
  .btn.small {
    height: 30px;
    padding: 0 12px;
  }
  .hint {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    margin: 8px 0 10px;
    font-size: 12.5px;
    color: var(--muted);
  }
  .link {
    padding: 2px 0;
    font-size: 12.5px;
    color: var(--primary);
  }
  .link:hover:not(:disabled) {
    text-decoration: underline;
  }
  .tiny {
    margin: 4px 0 0;
    font-size: 11.5px;
  }
  .small {
    font-size: 12.5px;
  }
  .controls {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 12px;
    margin-top: 12px;
  }
  .preamp {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px;
    font-size: 13px;
  }
  .preamp input[type="range"] {
    width: 160px;
    accent-color: var(--primary-strong);
  }
  .check {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .check input {
    accent-color: var(--primary-strong);
  }
  .num {
    width: 76px;
  }
  .mono {
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }
  .warn-text {
    color: #d97706;
  }
  .buttons {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .ab.pressed {
    background: var(--primary-strong);
    color: var(--on-primary-strong);
    transform: translateY(1px);
  }
  .taste {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-bottom: 10px;
    font-size: 13px;
  }
  .taste > span {
    display: flex;
    justify-content: space-between;
  }
  .taste input {
    accent-color: var(--primary-strong);
  }
  .error {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--danger);
  }
</style>
