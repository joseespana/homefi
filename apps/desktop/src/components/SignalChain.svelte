<script lang="ts" module>
  import { api, type EqProfile, type OutputDevice } from "../lib/api";

  // what the tooltips fetch once and keep: a profile's curve, the outputs' rates
  const curves = new Map<string, Promise<number[]>>();
  let outputs: Promise<OutputDevice[]> | null = null;

  /** A crossfade going on (homefi-player; absent from engines without it). */
  export interface Fading {
    from?: string | null;
    to?: string | null;
    progress: number;
    seconds: number;
  }

  export type ChainLook = "room" | "glass" | "lcd" | "luna" | "alu" | "web";

  /** 48000/44100 → "160 : 147". */
  function ratio(to: number, from: number): string {
    const gcd = (a: number, b: number): number => (b ? gcd(b, a % b) : a);
    const g = gcd(to, from) || 1;
    return `${to / g} : ${from / g}`;
  }

  const CONTAINERS: Record<string, string> = {
    flac: "FLAC file",
    m4a: "MP4 audio (.m4a)",
    mp4: "MP4 audio",
    alac: "MP4 audio (.m4a)",
    mp3: "MP3 file",
    wav: "WAV file",
    aif: "AIFF file",
    aiff: "AIFF file",
    ogg: "Ogg file",
    opus: "Ogg Opus file",
    dsf: "DSD file (.dsf)",
    dff: "DSD file (.dff)",
    wv: "WavPack file",
  };
  const LOSSLESS = new Set(["flac", "alac", "wav", "aiff", "pcm", "wavpack", "ape", "dsd"]);

  /** What kind of output a name sounds like (the icon and the words). */
  export const OUTPUT_KINDS: Record<string, string> = {
    usb: "USB DAC",
    headphones: "Headphones",
    bluetooth: "Bluetooth",
    monitor: "HDMI / display",
    speaker: "Speakers",
  };

  function curveOf(profile: EqProfile, rate: number, tilt: { bassDb: number; trebleDb: number; clarityDb?: number | null } | null): Promise<number[]> {
    const key = JSON.stringify([profile, rate, tilt]);
    let c = curves.get(key);
    if (!c) {
      c = (async () => {
        const freqs = Array.from({ length: 48 }, (_, i) => 20 * Math.pow(1000, i / 47));
        const p = tilt ? await api.eqWithTaste(profile, tilt) : profile;
        return api.eqResponse(p, freqs, rate);
      })();
      c.catch(() => curves.delete(key));
      if (curves.size > 24) curves.delete(curves.keys().next().value!);
      curves.set(key, c);
    }
    return c;
  }
</script>

<script lang="ts">
  // The signal path in the room: the file, what works on it (resampler,
  // loudness, equalizer, a crossfade) and the output, joined by a signal that
  // flows while the music plays. Each stage tells everything on hover or
  // focus, and a click opens the screen that changes it.
  //   compact   small stages, a living link between them, a status pill
  //   detailed  the big boxes (SignalChainDetailed)
  // Settings → Listening room → Signal chain (and the room's Customize menu).
  import { khz, outputIcon, type PlayerState } from "../lib/api";
  import { flatProfile, formatDb, sourceLabel } from "../lib/eq.svelte";
  import { LOUDNESS_MODES, scopePhrase, sound, tiltWords } from "../lib/sound.svelte";
  import { useSource } from "../lib/source.svelte";
  import { app } from "../lib/state.svelte";
  import { visual } from "../lib/visual.svelte";
  import Icon from "./Icon.svelte";
  import OutputMenu from "./OutputMenu.svelte";
  import SignalChainDetailed from "./SignalChainDetailed.svelte";

  let { look = "room" }: { look?: ChainLook } = $props();

  const src = useSource();
  const p = $derived(src.player as (PlayerState & { fading?: Fading | null }) | null);
  const stream = $derived(p?.stream ?? null);
  const track = $derived(src.current);
  const room = $derived(app.settings?.room);
  const detailed = $derived(room?.signal_style === "detailed");
  const playing = $derived(p?.status === "playing");
  const flowing = $derived(playing && (room?.signal_animate ?? true));

  const eqName = $derived(p?.eq_active ? (p.eq_profile ?? p.taste_label ?? "On") : null);
  const tasteLabel = $derived(p?.eq_active && p.eq_profile ? (p.taste_label ?? null) : null);
  const gain = $derived(Math.abs(p?.loudness_db ?? 0) >= 0.05 ? (p?.loudness_db ?? 0) : 0);
  const fading = $derived(p?.fading && p.fading.seconds > 0 ? p.fading : null);
  const resampled = $derived(!!stream?.resampled);
  const processed = $derived(!!eqName || gain !== 0 || !!fading);
  const verdict = $derived(processed ? "processed" : resampled ? "resampled" : "clean");
  const workers = $derived([eqName ? "the equalizer" : "", gain ? "loudness normalization" : "", fading ? "the crossfade" : ""].filter(Boolean));
  const verdictText = { clean: "Bit-perfect", resampled: "Resampled", processed: "Processed" } as const;

  type StageId = "source" | "resample" | "loudness" | "eq" | "fade" | "output";
  interface Stage {
    id: StageId;
    cap: string;
    /** It changes the samples: the link after it turns warm. */
    works: boolean;
  }
  const stages = $derived.by((): Stage[] => {
    if (!stream) return [];
    const s: Stage[] = [{ id: "source", cap: "Source", works: false }];
    if (resampled) s.push({ id: "resample", cap: "Resample", works: true });
    if (gain) s.push({ id: "loudness", cap: "Loudness", works: true });
    if (eqName) s.push({ id: "eq", cap: "Equalizer", works: true });
    if (fading) s.push({ id: "fade", cap: "Crossfade", works: true });
    s.push({ id: "output", cap: "Output", works: false });
    return s;
  });
  /** The link before stage i: warm once anything worked on the signal, dense after the resampler. */
  function linkOf(i: number): { warm: boolean; dense: boolean } {
    const before = stages.slice(0, i);
    return { warm: before.some((s) => s.works), dense: before.some((s) => s.id === "resample") };
  }

  const codec = $derived((stream?.codec ?? "").toUpperCase());
  const ext = $derived((track?.path.split("#")[0].split(".").pop() ?? "").toLowerCase());
  const tidal = $derived(!src.remote && app.currentIsStream);
  const lossless = $derived(track ? track.lossless : LOSSLESS.has((stream?.codec ?? "").toLowerCase()));
  const resolution = $derived(stream ? (stream.bits_per_sample ? `${stream.bits_per_sample}/${khz(stream.source_rate)}` : `${khz(stream.source_rate)} kHz`) : "");
  const kind = $derived(stream ? outputIcon(stream.device) : "speaker");
  const deviceShort = $derived((stream?.device ?? "").replace(/\s*\((.*)\)\s*$/, "").replace(/^(External|Built-in)\s+/i, ""));
  const channels = (n: number) => (n === 1 ? "mono" : n === 2 ? "stereo" : `${n} ch`);

  // what the equalizer plays with on this output, and the taste rule on top
  const eqProfile = $derived.by((): EqProfile | null => {
    const s = app.settings?.eq;
    if (!s || !stream) return null;
    return s.perOutput[stream.device] ?? s.defaultProfile ?? null;
  });
  const rule = $derived(p?.taste_rule ? (sound.taste.rules.find((r) => r.id === p!.taste_rule) ?? null) : null);
  const loud = $derived(sound.loudness);

  // ------------------------------------------------------------------ the tooltip card
  let open = $state<StageId | "verdict" | null>(null);
  let pinned = $state(false);
  let at = $state<{ x: number; top?: number; bottom?: number }>({ x: 0 });
  let curve = $state<number[] | null>(null);
  let rates = $state<number[] | null>(null);
  let timer: ReturnType<typeof setTimeout> | null = null;
  const uid = `sc${Math.random().toString(36).slice(2, 8)}`;

  function place(el: HTMLElement) {
    const r = el.getBoundingClientRect();
    const w = 300;
    const x = Math.max(8, Math.min(window.innerWidth - w - 8, r.left + r.width / 2 - w / 2));
    at = r.bottom + 300 > window.innerHeight && r.top > 300 ? { x, bottom: window.innerHeight - r.top + 10 } : { x, top: r.bottom + 10 };
  }
  function show(id: StageId | "verdict", el: HTMLElement, now = false) {
    if (timer) clearTimeout(timer);
    const go = () => {
      place(el);
      open = id;
      if (id === "eq") loadCurve();
      if (id === "output" || id === "resample") loadRates();
    };
    if (now) go();
    else timer = setTimeout(go, 140);
  }
  function hide() {
    if (timer) clearTimeout(timer);
    timer = null;
    if (!pinned) open = null;
  }
  function close() {
    pinned = false;
    hide();
    open = null;
  }
  function loadCurve() {
    if (!stream) return;
    const profile = eqProfile && p?.eq_profile ? eqProfile : rule ? flatProfile() : null;
    if (!profile) return (curve = null);
    curveOf(profile, stream.output_rate, rule?.tilt ?? null)
      .then((c) => (curve = c))
      .catch(() => (curve = null));
  }
  function loadRates() {
    if (!stream || src.remote) return;
    outputs ??= api.outputDevices();
    outputs.catch(() => (outputs = null));
    const name = stream.device;
    outputs.then((d) => (rates = d.find((o) => o.name === name)?.sample_rates ?? null)).catch(() => {});
  }
  const curvePath = $derived.by(() => {
    if (!curve?.length) return "";
    const W = 268;
    const H = 46;
    const span = Math.max(6, ...curve.map((v) => Math.abs(v)));
    return curve.map((v, i) => `${i ? "L" : "M"}${((i / (curve!.length - 1)) * W).toFixed(1)} ${(H / 2 - (v / span) * (H / 2 - 3)).toFixed(1)}`).join(" ");
  });

  // ------------------------------------------------------------------ clicks
  let outputOpen = $state(false);
  let outputBox = $state<{ x: number; y: number; w: number; h: number; below: boolean } | null>(null);

  function settingsAt(page: string) {
    try {
      localStorage.setItem("homefi.settings.page", page);
    } catch {}
    app.view = { name: "settings", page };
  }
  function activate(id: StageId | "verdict", el: HTMLElement) {
    if (src.remote || id === "source" || id === "verdict") {
      // nothing to change from here: the card stays until the next click
      pinned = !(pinned && open === id);
      if (pinned) show(id, el, true);
      else open = null;
      return;
    }
    close();
    if (id === "eq") app.view = { name: "eq", output: stream?.device, tab: p?.eq_profile ? "device" : "taste" };
    else if (id === "loudness" || id === "fade") settingsAt("sound");
    else {
      const r = el.getBoundingClientRect();
      outputBox = { x: r.left, y: r.top, w: r.width, h: r.height, below: r.top < 420 };
      outputOpen = true;
    }
  }
  const hint: Partial<Record<StageId | "verdict", string>> = {
    resample: "Click: choose another output",
    loudness: "Click: loudness settings",
    eq: "Click: open the equalizer",
    fade: "Click: crossfade settings",
    output: "Click: change the output",
  };

  // the tooltip leaves the chain (panels clip, glass makes `fixed` local): it lives on <body>
  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }
  $effect(() => {
    if (!open && !outputOpen) return;
    const away = () => ((outputOpen = false), close());
    const key = (e: KeyboardEvent) => e.key === "Escape" && away();
    window.addEventListener("keydown", key);
    window.addEventListener("resize", away);
    window.addEventListener("scroll", close, true);
    return () => {
      window.removeEventListener("keydown", key);
      window.removeEventListener("resize", away);
      window.removeEventListener("scroll", close, true);
    };
  });

  // ------------------------------------------------------------------ the signal's swing
  // a few times a second the waves follow the music's energy (the visual loop
  // that already runs for the meters; nothing polls for this). A CSS variable,
  // no Svelte state: the links animate in CSS.
  let chain = $state<HTMLElement | null>(null);
  $effect(() => {
    const el = chain;
    if (!el || !flowing) return;
    const id = setInterval(() => {
      if (document.documentElement.dataset.motion !== "full") return el.style.removeProperty("--amp");
      const e = visual.energy;
      el.style.setProperty("--amp", e > 0.01 ? (0.55 + Math.min(1, e * 1.5) * 0.6).toFixed(2) : "1");
    }, 220);
    return () => {
      clearInterval(id);
      el.style.removeProperty("--amp");
    };
  });
  // sine across the link: two periods
  const WAVE = "M0 8 Q4.5 2.5 9 8 T18 8 T27 8 T36 8";
</script>

{#if stream && detailed}
  <SignalChainDetailed {stream} eq={eqName} taste={tasteLabel} loudness={gain} />
{:else if stream}
  <div class="chain sc-{look}" class:flowing class:still={!(room?.signal_animate ?? true)} bind:this={chain} role="group" aria-label="Signal path: {verdictText[verdict]}">
    {#each stages as st, i (st.id)}
      {#if i > 0}
        {@const l = linkOf(i)}
        <svg class="link" class:warm={l.warm} class:dense={l.dense} viewBox="0 0 36 16" aria-hidden="true">
          <g class="swing">
            <path class="sc-rail" d={WAVE} />
            <path class="dots" d={WAVE} />
          </g>
        </svg>
      {/if}
      <button
        class="node {st.id}"
        class:works={st.works}
        class:open={open === st.id}
        aria-label="{st.cap}"
        aria-describedby={open === st.id ? `${uid}-${st.id}` : undefined}
        aria-expanded={st.id === "output" ? outputOpen : undefined}
        onmouseenter={(e) => show(st.id, e.currentTarget)}
        onmouseleave={hide}
        onfocus={(e) => show(st.id, e.currentTarget, true)}
        onblur={hide}
        onclick={(e) => activate(st.id, e.currentTarget)}
      >
        {#if st.id === "source"}
          <span class="ico"><Icon name={tidal ? "soundwave" : "music"} size={13} /></span>
          <span class="chip">{tidal ? "TIDAL" : codec}</span>
          <span class="txt">{tidal ? `${codec} ${resolution}` : resolution}{stream.source_channels !== 2 ? ` · ${channels(stream.source_channels)}` : ""}</span>
        {:else if st.id === "resample"}
          <span class="ico">
            <!-- a wave redrawn on a finer grid -->
            <svg viewBox="0 0 24 24" width="13" height="13" aria-hidden="true"><path d="M2 12c2.5-7 5-7 7.5 0s5 7 7.5 0" /><path d="M14 6h2v3h2v3h2v3h2" /></svg>
          </span>
          <span class="txt">{khz(stream.source_rate)}→{khz(stream.output_rate)}<small>kHz</small></span>
        {:else if st.id === "loudness"}
          <span class="ico"><Icon name="gauge" size={13} /></span>
          <span class="txt">{gain > 0 ? "+" : "−"}{Math.abs(gain).toFixed(1)}<small>dB</small></span>
        {:else if st.id === "eq"}
          <span class="ico"><Icon name="eq" size={13} /></span>
          <span class="txt name">{eqName}</span>
          {#if tasteLabel}<span class="chip taste" title={tasteLabel}><Icon name="layers" size={10} />{tasteLabel.split(" · ")[0]}</span>{/if}
        {:else if st.id === "fade"}
          <span class="ico">
            <svg viewBox="0 0 24 24" width="13" height="13" aria-hidden="true"><path d="M3 6c7 0 11 12 18 12" /><path d="M3 18c7 0 11-12 18-12" /></svg>
          </span>
          <span class="txt">{Math.round(fading!.progress * 100)}<small>%</small></span>
          <span class="meter" style:--f={fading!.progress}></span>
        {:else}
          <span class="ico"><Icon name={kind} size={13} /></span>
          <span class="txt name">{deviceShort}</span>
          {#if resampled}<span class="txt rate">{khz(stream.output_rate)}<small>kHz</small></span>{:else}<i class="native" title="At the file's own rate">✓</i>{/if}
        {/if}
      </button>
    {/each}
    <button
      class="pill {verdict}"
      aria-label="{verdictText[verdict]}"
      aria-describedby={open === "verdict" ? `${uid}-verdict` : undefined}
      onmouseenter={(e) => show("verdict", e.currentTarget)}
      onmouseleave={hide}
      onfocus={(e) => show("verdict", e.currentTarget, true)}
      onblur={hide}
      onclick={(e) => activate("verdict", e.currentTarget)}
    >
      <span class="led"></span><span class="pill-txt">{verdictText[verdict]}</span>
    </button>
  </div>

  {#if open}
    <div class="tip sc-card sc-{look}" id="{uid}-{open}" role="tooltip" use:portal style:left="{at.x}px" style:top={at.top != null ? `${at.top}px` : undefined} style:bottom={at.bottom != null ? `${at.bottom}px` : undefined}>
      {#if open === "source"}
        <div class="sc-head"><span class="chip">{codec}</span><b>{lossless ? "Lossless" : "Lossy"} source</b></div>
        <dl>
          <dt>Codec</dt><dd>{codec}{lossless ? " (lossless)" : " (compressed)"}</dd>
          <dt>Container</dt><dd>{tidal ? "Tidal stream" : (CONTAINERS[ext] ?? (ext ? `.${ext} file` : "—"))}</dd>
          <dt>Bit depth</dt><dd>{stream.bits_per_sample ? `${stream.bits_per_sample}-bit` : lossless ? "—" : "decoded to float"}</dd>
          <dt>Sample rate</dt><dd>{khz(stream.source_rate)} kHz</dd>
          <dt>Channels</dt><dd>{stream.source_channels} ({channels(stream.source_channels)})</dd>
          {#if track?.quality}<dt>Quality</dt><dd>{track.quality}</dd>{/if}
          {#if src.remote}
            <dt>From</dt><dd>{src.label}</dd>
          {:else if tidal}
            <dt>Tidal</dt><dd>{app.currentTidalId ? `track ${app.currentTidalId}` : "streaming"}</dd>
          {:else if track}
            <dt>File</dt><dd class="path" title={track.path}>{track.path.split("#")[0]}</dd>
          {/if}
        </dl>
      {:else if open === "resample"}
        <div class="sc-head"><b>Resampled</b><span class="sub">the output can't run at {khz(stream.source_rate)} kHz</span></div>
        <dl>
          <dt>From</dt><dd>{khz(stream.source_rate)} kHz</dd>
          <dt>To</dt><dd>{khz(stream.output_rate)} kHz</dd>
          <dt>Ratio</dt><dd>{ratio(stream.output_rate, stream.source_rate)}</dd>
          <dt>Algorithm</dt><dd>Band-limited sinc (rubato), 64-bit float</dd>
          {#if rates?.length}<dt>Output runs at</dt><dd>{rates.map(khz).join(" / ")} kHz</dd>{/if}
        </dl>
        <p class="sc-note">Every sample is recalculated: very close to the original, but not bit-perfect. An output that supports {khz(stream.source_rate)} kHz plays it as it is.</p>
      {:else if open === "loudness"}
        <div class="sc-head"><b>Loudness normalization</b><span class="sub">{gain > 0 ? "+" : "−"}{Math.abs(gain).toFixed(1)} dB on this song</span></div>
        <dl>
          <dt>Mode</dt><dd>{LOUDNESS_MODES.find((m) => m.id === loud.mode)?.label ?? loud.mode}</dd>
          <dt>Target</dt><dd>{loud.targetLufs} LUFS</dd>
          <dt>Gain now</dt><dd>{formatDb(gain)}</dd>
          <dt>From</dt><dd>ReplayGain tags (or a measure of the song)</dd>
          <dt>Clipping</dt><dd>{loud.preventClipping ? "Protected: never boosted past its peak" : "Not limited"}</dd>
        </dl>
      {:else if open === "eq"}
        <div class="sc-head"><b class="ellipsis">{p?.eq_profile ?? "Taste layer"}</b>{#if p?.eq_profile && eqProfile}<span class="sub">{sourceLabel(eqProfile)}</span>{/if}</div>
        {#if curvePath}
          <svg class="curve" viewBox="0 0 268 46" preserveAspectRatio="none" aria-hidden="true">
            <line x1="0" y1="23" x2="268" y2="23" />
            <path d={curvePath} />
          </svg>
        {/if}
        <dl>
          {#if p?.eq_profile && eqProfile}
            <dt>Preamp</dt><dd>{app.settings?.eq.autoPreamp ? "Automatic, from the curve" : formatDb(eqProfile.preampDb)}</dd>
            <dt>Bands</dt><dd>{eqProfile.bands.filter((b) => b.enabled).length} of {eqProfile.bands.length} on</dd>
          {/if}
          {#if rule}
            <dt>Taste layer</dt><dd>{rule.label}</dd>
            <dt>For</dt><dd>{scopePhrase(rule.scope)}</dd>
            <dt>Tilt</dt><dd>{tiltWords(rule.tilt)}</dd>
          {:else if tasteLabel}
            <dt>Taste layer</dt><dd>{tasteLabel}</dd>
          {/if}
          <dt>Processing</dt><dd>64-bit float DSP</dd>
        </dl>
      {:else if open === "fade" && fading}
        {@const from = fading.from ? src.trackAt(fading.from) : null}
        {@const to = fading.to ? src.trackAt(fading.to) : null}
        <div class="sc-head"><b>Crossfade</b><span class="sub">{fading.seconds} s, both songs mixed</span></div>
        <div class="sc-bar"><i style:width="{fading.progress * 100}%"></i></div>
        <dl>
          {#if from}<dt>Out</dt><dd class="ellipsis">{from.title}</dd>{/if}
          {#if to}<dt>In</dt><dd class="ellipsis">{to.title}</dd>{/if}
          <dt>Length</dt><dd>{fading.seconds} s</dd>
          <dt>Progress</dt><dd>{Math.round(fading.progress * 100)}%</dd>
        </dl>
      {:else if open === "output"}
        <div class="sc-head"><Icon name={kind} size={16} /><b class="ellipsis">{stream.device}</b></div>
        <dl>
          <dt>Type</dt><dd>{OUTPUT_KINDS[kind]}</dd>
          <dt>Running at</dt><dd>{khz(stream.output_rate)} kHz{resampled ? "" : " (the file's own rate)"}</dd>
          <dt>Channels</dt><dd>{stream.output_channels} ({channels(stream.output_channels)})</dd>
          {#if rates?.length}<dt>Supports</dt><dd>{rates.map(khz).join(" / ")} kHz</dd>{/if}
          {#if src.remote}<dt>Where</dt><dd>{src.label}</dd>{/if}
        </dl>
      {:else if open === "verdict"}
        <div class="sc-head"><span class="led {verdict}"></span><b>{verdictText[verdict]}</b></div>
        <p class="sc-note">
          {#if processed}
            {workers.join(", ").replace(/, ([^,]*)$/, " and $1").replace(/^./, (c) => c.toUpperCase())}
            {workers.length > 1 ? "change" : "changes"} the samples on their way to the device: the sound is processed, not bit-perfect.{resampled ? ` It is also resampled to ${khz(stream.output_rate)} kHz.` : ""}
          {:else if resampled}
            This output can't run at {khz(stream.source_rate)} kHz, so the music is resampled to {khz(stream.output_rate)} kHz. Nothing else touches it.
          {:else}
            The device runs at the file's own rate ({khz(stream.source_rate)} kHz) and nothing changes the samples: what reaches it is exactly what is in the file.
          {/if}
        </p>
      {/if}
      {#if hint[open] && !src.remote}<div class="foot">{hint[open]}</div>{/if}
    </div>
  {/if}

  {#if outputOpen && outputBox}
    <div class="out-box" class:below={outputBox.below} use:portal style:left="{outputBox.x}px" style:top="{outputBox.y}px" style:width="{outputBox.w}px" style:height="{outputBox.h}px">
      <button class="scrim" aria-label="Close" onclick={() => (outputOpen = false)}></button>
      <OutputMenu onclose={() => (outputOpen = false)} />
    </div>
  {/if}
{/if}

<style>
  /* ------------------------------------------------------------ tokens */
  .chain,
  .sc-card {
    --sc-clean: color-mix(in srgb, var(--ok) 55%, var(--brand-teal));
    --sc-warm: #fbbf24;
    --sc-node: var(--glass-strong);
    --sc-edge: var(--glass-border);
    --sc-text: var(--text);
    --sc-muted: var(--muted);
    --sc-chip: color-mix(in srgb, var(--sc-clean) 22%, transparent);
    --sc-chip-text: var(--text);
    --sc-radius: min(999px, var(--r-max));
    --sc-font: var(--font-mono);
  }
  :global(:root[data-mode="light"]) :is(.chain, .sc-card):is(.sc-room, .sc-glass) {
    --sc-clean: color-mix(in srgb, var(--ok) 70%, var(--brand-teal));
    --sc-warm: #b45309;
  }

  /* ------------------------------------------------------------ the row */
  .chain {
    /* sized by its place, never by its stages (it's a size container) */
    width: 100%;
    container-type: inline-size;
    display: flex;
    align-items: center;
    min-width: 0;
    font-family: var(--sc-font);
    --amp: 1;
  }
  .node,
  .pill {
    flex: none;
    height: 30px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 0 10px 0 4px;
    border-radius: var(--sc-radius);
    background: var(--sc-node);
    border: 1px solid var(--sc-edge);
    color: var(--sc-text);
    font: inherit;
    font-size: 12px;
    white-space: nowrap;
    cursor: pointer;
    transition:
      border-color 0.15s,
      background 0.15s,
      transform 0.15s;
  }
  /* only the names give way (ellipsis); the numbers stay whole */
  .node:is(.eq, .output) {
    flex: 0 1 auto;
    min-width: 0;
    overflow: hidden;
  }
  /* the profile's name gives way before the device's */
  .node.eq {
    flex-shrink: 4;
  }
  .node:hover,
  .node.open,
  .pill:hover {
    border-color: color-mix(in srgb, var(--sc-clean) 60%, var(--sc-edge));
  }
  .node.works:hover,
  .node.works.open {
    border-color: color-mix(in srgb, var(--sc-warm) 60%, var(--sc-edge));
  }
  .node:focus-visible,
  .pill:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 2px;
  }
  .node:active {
    transform: scale(0.97);
  }
  .ico {
    flex: none;
    width: 22px;
    height: 22px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    color: var(--sc-clean);
    background: color-mix(in srgb, var(--sc-clean) 16%, transparent);
  }
  .works .ico {
    color: var(--sc-warm);
    background: color-mix(in srgb, var(--sc-warm) 16%, transparent);
  }
  .ico svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2.2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .txt {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .txt small {
    margin-left: 2px;
    font-size: 9.5px;
    color: var(--sc-muted);
  }
  .works .txt {
    color: color-mix(in srgb, var(--sc-warm) 75%, var(--sc-text));
  }
  .txt.name {
    min-width: 54px;
    max-width: 150px;
    font-family: var(--font);
    font-weight: 600;
  }
  .node.output .txt.name {
    max-width: 130px;
  }
  .rate {
    flex: none;
    color: var(--sc-muted);
  }
  .native {
    flex: none;
    font-style: normal;
    font-weight: 700;
    color: var(--sc-clean);
  }
  .chip {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 2px 5px;
    border-radius: min(5px, var(--r-max));
    background: var(--sc-chip);
    color: var(--sc-chip-text);
    font-size: 10px;
    font-weight: 800;
    letter-spacing: 0.06em;
  }
  .chip.taste {
    max-width: 110px;
    overflow: hidden;
    text-overflow: ellipsis;
    font-family: var(--font);
    font-weight: 600;
    letter-spacing: 0;
    background: color-mix(in srgb, var(--sc-warm) 18%, transparent);
    color: var(--sc-text);
  }
  .meter {
    width: 26px;
    height: 4px;
    border-radius: 2px;
    background: linear-gradient(90deg, var(--sc-warm) calc(var(--f) * 100%), color-mix(in srgb, var(--sc-warm) 22%, transparent) 0);
  }

  /* ------------------------------------------------------------ the living link */
  .link {
    flex: 0 1 34px;
    min-width: 16px;
    height: 18px;
    overflow: visible;
  }
  .swing {
    transform-box: view-box;
    transform-origin: 50% 50%;
    transform: scaleY(var(--amp));
    transition: transform 0.25s ease-out;
  }
  .sc-rail,
  .dots {
    fill: none;
    stroke: var(--sc-clean);
    stroke-linecap: round;
    vector-effect: non-scaling-stroke;
  }
  .sc-rail {
    stroke-width: 1.2;
    opacity: 0.3;
  }
  .dots {
    stroke-width: 2.6;
    stroke-dasharray: 0 6;
    animation: flow calc(0.5s / var(--motion-speed, 1)) linear infinite;
    animation-play-state: paused;
  }
  .link.warm :is(.sc-rail, .dots) {
    stroke: var(--sc-warm);
  }
  /* after the resampler: more, smaller points (a finer grid of samples) */
  .link.dense .dots {
    stroke-width: 1.7;
    stroke-dasharray: 0 3;
  }
  .flowing .dots {
    animation-play-state: running;
  }
  .still .dots,
  :global(:root:is([data-motion="subtle"], [data-motion="off"])) .dots {
    animation: none;
  }
  .still .swing,
  :global(:root:is([data-motion="subtle"], [data-motion="off"])) .swing {
    transform: none;
  }
  @keyframes flow {
    to {
      stroke-dashoffset: -12;
    }
  }

  /* ------------------------------------------------------------ status pill */
  .pill {
    flex: none;
    margin-left: 10px;
    padding: 0 10px;
    gap: 7px;
    font-family: var(--font);
    font-weight: 700;
    font-size: 11.5px;
    color: var(--sc-clean);
    background: color-mix(in srgb, var(--sc-clean) 12%, transparent);
    border-color: color-mix(in srgb, var(--sc-clean) 35%, transparent);
  }
  .pill:not(.clean) {
    color: var(--sc-warm);
    background: color-mix(in srgb, var(--sc-warm) 12%, transparent);
    border-color: color-mix(in srgb, var(--sc-warm) 35%, transparent);
  }
  .led {
    flex: none;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: currentColor;
    box-shadow: 0 0 6px currentColor;
  }

  /* narrow: the extra words go first, then the names; the card has it all */
  @container (max-width: 760px) {
    .chip.taste {
      display: none;
    }
  }
  @container (max-width: 640px) {
    .node.output .rate {
      display: none;
    }
    .txt.name {
      max-width: 96px;
    }
  }
  @container (max-width: 580px) {
    .pill-txt,
    .node:is(.loudness, .resample, .fade) .txt,
    .meter {
      display: none;
    }
    .pill {
      padding: 0 9px;
    }
    .node:is(.loudness, .resample, .fade) {
      padding-right: 4px;
    }
  }
  @container (max-width: 470px) {
    .node:is(.eq, .output) .txt,
    .node.source .txt {
      display: none;
    }
    .node {
      padding-right: 4px;
    }
  }

  /* ------------------------------------------------------------ the card */
  .sc-card {
    position: fixed;
    z-index: 60;
    width: 300px;
    padding: 12px 14px;
    border-radius: min(12px, var(--r-max));
    background: var(--window-surface, var(--surface));
    color: var(--window-text, var(--text));
    border: 1px solid var(--window-border, var(--border));
    box-shadow: var(--shadow);
    font-size: 12px;
    pointer-events: none;
    animation: card-in calc(0.14s / var(--motion-speed, 1)) var(--ease-out, ease-out);
  }
  @keyframes card-in {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }
  :global(:root[data-motion="subtle"]) .sc-card {
    animation-name: card-fade;
  }
  @keyframes card-fade {
    from {
      opacity: 0;
    }
  }
  .sc-head {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 4px 8px;
    margin-bottom: 8px;
    min-width: 0;
  }
  .sc-head b {
    font-size: 13px;
    min-width: 0;
  }
  .sc-head .sub {
    flex-basis: 100%;
    color: var(--window-muted, var(--muted));
    font-size: 11.5px;
  }
  .sc-card .chip {
    font-family: var(--font-mono);
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 4px 12px;
    margin: 0;
  }
  dt {
    color: var(--window-muted, var(--muted));
  }
  dd {
    margin: 0;
    min-width: 0;
    font-family: var(--font-mono);
    font-size: 11.5px;
    overflow-wrap: anywhere;
  }
  dd.path {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .sc-note {
    margin: 8px 0 0;
    color: var(--window-muted, var(--muted));
    line-height: 1.45;
  }
  .sc-head + .sc-note {
    margin-top: 0;
  }
  .foot {
    margin-top: 10px;
    padding-top: 8px;
    border-top: 1px solid var(--window-border, var(--border));
    color: var(--primary);
    font-size: 11.5px;
    font-weight: 600;
  }
  .curve {
    display: block;
    width: 100%;
    height: 46px;
    margin: 2px 0 10px;
    border-radius: 6px;
    background: color-mix(in srgb, var(--sc-warm) 7%, transparent);
  }
  .curve line {
    stroke: var(--window-border, var(--border));
    stroke-dasharray: 3 3;
  }
  .curve path {
    fill: none;
    stroke: var(--sc-warm);
    stroke-width: 2;
    vector-effect: non-scaling-stroke;
  }
  .sc-bar {
    height: 4px;
    border-radius: 2px;
    margin: 0 0 10px;
    background: color-mix(in srgb, var(--sc-warm) 20%, transparent);
    overflow: hidden;
  }
  .sc-bar i {
    display: block;
    height: 100%;
    background: var(--sc-warm);
  }
  .sc-card .led {
    color: var(--sc-clean);
  }
  .sc-card .led:not(.clean) {
    color: var(--sc-warm);
  }

  .out-box {
    position: fixed;
    z-index: 55;
  }
  .out-box.below :global(.menu) {
    top: calc(100% + 10px);
    bottom: auto;
  }
  .scrim {
    position: fixed;
    inset: 0;
    cursor: default;
  }

  /* ------------------------------------------------------------ the skins' own chains */
  /* Retro Amp: an LCD strip, green on black, square */
  .chain.sc-lcd,
  .sc-card.sc-lcd {
    --sc-clean: #00e000;
    --sc-warm: #e0d020;
    --sc-node: #000;
    --sc-edge: #12121a;
    --sc-text: #00e000;
    --sc-muted: #0a8a0a;
    --sc-chip: #0a3a0a;
    --sc-chip-text: #00e000;
    --sc-radius: 0;
  }
  .chain.sc-lcd .node,
  .chain.sc-lcd .pill {
    height: 24px;
    box-shadow:
      inset 1px 1px 0 #12121a,
      inset -1px -1px 0 #5c5c78;
  }
  .chain.sc-lcd .ico {
    background: none;
    border-radius: 0;
  }
  .chain.sc-lcd .txt.name {
    font-family: var(--font-mono);
  }
  .chain.sc-lcd .led {
    border-radius: 0;
  }
  .sc-card.sc-lcd {
    background: linear-gradient(180deg, #34344a, #1e1e2c);
    color: #cfcfe0;
    border: 0;
    border-radius: 0;
    box-shadow:
      inset 1px 1px 0 #5c5c78,
      inset -1px -1px 0 #12121a,
      4px 4px 0 rgb(0 0 0 / 0.6);
    --window-muted: #9a9ab4;
    --window-border: #12121a;
    --primary: #00e000;
  }
  .sc-card.sc-lcd dd {
    color: #00e000;
  }
  .sc-card.sc-lcd .curve {
    background: #000;
    border-radius: 0;
  }

  /* Luna: XP buttons on blue, a yellow tooltip */
  .chain.sc-luna {
    --sc-clean: #7fff7f;
    --sc-warm: #ffd24a;
    --sc-node: linear-gradient(180deg, rgb(255 255 255 / 0.28), rgb(255 255 255 / 0.08));
    --sc-edge: rgb(255 255 255 / 0.45);
    --sc-text: #fff;
    --sc-muted: rgb(255 255 255 / 0.75);
    --sc-chip: #3c8d2f;
    --sc-chip-text: #fff;
    --sc-radius: 4px;
    font-family: Tahoma, "Segoe UI", var(--font);
  }
  .chain.sc-luna .node,
  .chain.sc-luna .pill {
    height: 26px;
  }
  .sc-card.sc-luna {
    --sc-clean: #2e8b2e;
    --sc-warm: #b36b00;
    --window-muted: #444;
    --window-border: #000;
    --primary: #0046d5;
    background: #ffffe1;
    color: #000;
    border-color: #000;
    border-radius: 0;
    box-shadow: 2px 2px 4px rgb(0 0 0 / 0.35);
    font-family: Tahoma, "Segoe UI", var(--font);
  }

  /* Aluminum: little Aqua capsules on brushed metal */
  .chain.sc-alu {
    --sc-clean: #2f7d32;
    --sc-warm: #b35c00;
    --sc-node: linear-gradient(180deg, #fdfdfd, #d9d9d9);
    --sc-edge: #8e8e8e;
    --sc-text: #1a1a1a;
    --sc-muted: #555;
    --sc-chip: linear-gradient(180deg, #9fd0ff, #3a8ee6);
    --sc-chip-text: #fff;
    font-family: "Lucida Grande", var(--font);
  }
  .chain.sc-alu .node,
  .chain.sc-alu .pill {
    height: 24px;
    box-shadow: inset 0 1px 0 #fff;
  }
  .sc-card.sc-alu {
    --sc-clean: #2f7d32;
    --sc-warm: #b35c00;
    --window-muted: #555;
    --window-border: #9a9a9a;
    background: #ffffc6;
    color: #000;
    border-color: #9a9a9a;
    border-radius: 4px;
  }

  /* Web 2000: glossy, orange and lime */
  .chain.sc-web {
    --sc-clean: #5fa800;
    --sc-warm: #ff7a00;
    --sc-node: linear-gradient(180deg, #fff 50%, #eef2f5 50%);
    --sc-edge: #c9d3dc;
    --sc-text: #333;
    --sc-muted: #777;
    --sc-chip: linear-gradient(180deg, #9be33a, #5fa800);
    --sc-chip-text: #fff;
  }
  .chain.sc-web .node,
  .chain.sc-web .pill {
    box-shadow: 0 1px 2px rgb(0 0 0 / 0.12);
  }

  /* glass layouts (immersive, vinyl, studio) over the album: a frosted capsule */
  .chain.sc-glass .node {
    backdrop-filter: blur(14px) saturate(1.3);
  }
</style>
