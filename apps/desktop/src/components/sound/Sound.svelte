<script lang="ts">
  // The Sound screen, organised around "what is playing now": a master switch
  // (off = bit-perfect), the Playing now card with its curve, the sounds you
  // made (or every song with its sound), the output and the sound settings.
  // Edit sound and the output's own curve open as side panels. Drawing only:
  // state and IPC live in lib/sound.svelte.ts and lib/assistant.svelte.ts.
  import { onMount, untrack } from "svelte";
  import { api, errorText } from "../../lib/api";
  import { rise } from "../../lib/motion";
  import { correctionName, sound } from "../../lib/sound.svelte";
  import { app } from "../../lib/state.svelte";
  import OutputCurveEditor from "../eq/OutputCurveEditor.svelte";
  import Segmented from "../settings/Segmented.svelte";
  import Icon from "../Icon.svelte";
  import ByMusicTable from "./ByMusicTable.svelte";
  import EditSound from "./EditSound.svelte";
  import OutputCard from "./OutputCard.svelte";
  import PlayingNow from "./PlayingNow.svelte";
  import SoundList from "./SoundList.svelte";
  import SoundSettingsRow from "./SoundSettingsRow.svelte";
  import SuggestionCard from "./SuggestionCard.svelte";
  import TryBar from "./TryBar.svelte";

  const TABS = [
    { id: "sounds", label: "Sounds" },
    { id: "music", label: "By music" },
  ] as const;

  const view = $derived(app.view.name === "sound" ? app.view : null);
  const tab = $derived(view?.tab ?? "sounds");
  const editId = $derived(view?.edit ?? null);
  // the output's own curve: opened by "Everything else", the output card, or a link with `output`
  let curve = $state<{ output: string | null } | null>(null);
  $effect(() => {
    const o = view?.output;
    if (o) untrack(() => (curve = { output: o }));
  });
  // dev hooks for screenshots: ?soundtab=music, ?soundedit=<id>, ?soundsearch=Steely (the search open and typed)
  const hooks = import.meta.env.DEV ? new URLSearchParams(location.search) : null;
  const search = hooks?.get("soundsearch") ?? "";
  onMount(() => {
    const t = hooks?.get("soundtab");
    const e = hooks?.get("soundedit");
    if (t === "music" || t === "sounds" || e) app.view = { name: "sound", tab: t === "music" ? "music" : "sounds", edit: e ?? undefined };
  });

  const setTab = (id: "sounds" | "music") => (app.view = { name: "sound", tab: id });
  const closeEdit = () => (app.view = { name: "sound", tab });

  // what plays follows the song, the sounds and the player's own report of the sound
  const nowKey = $derived.by(() => {
    const p = app.player;
    return `${p?.current}|${p?.taste_rule}|${p?.taste_target?.kind}${p?.taste_target?.name}|${p?.auditioning}|${p?.stream?.device}`;
  });
  $effect(() => {
    nowKey;
    app.settings?.taste;
    app.settings?.eq?.enabled;
    untrack(() => sound.refreshNow());
  });
  // Esc closes a Try's AI panel handled in AskAi; nothing else to clean here
  const output = $derived(sound.output);
  const profile = $derived(sound.profileFor(output));
  const on = $derived(sound.on);

  async function setMaster(enabled: boolean) {
    const eq = app.settings?.eq;
    if (!eq) return;
    try {
      await api.eqSet({ ...eq, enabled });
    } catch (e) {
      app.note(errorText(e), "error", "sound-master");
    }
  }
</script>

<div class="sound" class:off={!on}>
  <header class="head">
    <h1>Sound</h1>
    {#if output}
      <span class="chip" title={profile ? "Headphone correction in use" : "No headphone correction for this output"}>
        {#if profile}<span class="ok">✓</span> {output} · corrected{:else}{output}{/if}
      </span>
    {/if}
    <span class="grow"></span>
    <label class="master" title={on ? "Turn Sound off: bit-perfect" : "Turn Sound on"}>
      <b>{on ? "Sound on" : "Sound off"}</b>
      <input type="checkbox" role="switch" checked={on} onchange={(e) => setMaster(e.currentTarget.checked)} />
      <span class="track" aria-hidden="true"><span class="thumb"></span></span>
    </label>
  </header>

  <TryBar />

  <div class="grid" class:wide={tab === "music"}>
    <div class="main">
      <PlayingNow slim={tab === "music"} />
      {#if tab !== "music"}<SoundSettingsRow />{/if}
      <SuggestionCard />
    </div>
    <div class="side">
      <Segmented options={TABS} value={tab} onchange={setTab} label="Sounds or by music" />
      {#key tab}
        <div in:rise={{ y: 6, duration: 180 }}>
          {#if tab === "music"}
            <ByMusicTable />
          {:else}
            <SoundList onoutput={() => (curve = { output })} />
          {/if}
        </div>
      {/key}
      <OutputCard onedit={(o) => (curve = { output: o })} />
      {#if tab === "music"}<SoundSettingsRow />{/if}
    </div>
  </div>
</div>

{#if editId}<EditSound id={editId} {search} onclose={closeEdit} />{/if}
{#if curve}<OutputCurveEditor output={curve.output} onclose={() => (curve = null)} />{/if}

<style>
  .sound {
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 1280px;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 14px;
    flex-wrap: wrap;
  }
  .grow {
    flex: 1;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 12px;
    border-radius: min(999px, var(--r-max));
    font-size: 12.5px;
    color: var(--muted);
    background: var(--glass-strong);
    border: 1px solid var(--glass-border);
  }
  .ok {
    color: var(--ok);
    font-weight: 700;
  }
  .master {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 10px;
    padding: 6px 8px 6px 16px;
    border-radius: min(999px, var(--r-max));
    background: var(--glass);
    border: 1px solid var(--glass-border);
    cursor: pointer;
    font-size: 13px;
  }
  .master input {
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
    transition: transform calc(0.2s * var(--motion-speed, 1)) var(--ease-pop, ease);
  }
  .master input:checked + .track {
    background: var(--primary-strong);
  }
  .master input:checked + .track .thumb {
    transform: translateX(20px);
  }
  .master input:focus-visible + .track {
    outline: 2px solid var(--primary);
    outline-offset: 2px;
  }
  .grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 16px;
  }
  .main,
  .side {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }
  /* off: the lists stay readable (and editable), just quieter */
  .sound.off .side > div {
    opacity: 0.7;
  }
  .side :global(.seg) {
    align-self: flex-start;
    width: min(100%, 300px);
  }
  .side :global(.seg button) {
    flex: 1;
  }
  @container (min-width: 980px) {
    .grid:not(.wide) {
      grid-template-columns: minmax(0, 1fr) minmax(320px, 420px);
      align-items: start;
    }
  }
</style>
