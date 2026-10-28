<script lang="ts">
  import SeekBar from "./SeekBar.svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { api, errorText, type Quality, type Settings, type ShareSettings, type Theme, type ProgressStyle, type Motion } from "../lib/api";
  import { share } from "../lib/share.svelte";
  import MascotOptions from "./MascotOptions.svelte";
  import { LAYOUTS, fixedTheme, layoutOf, skinOf } from "../lib/layouts";
  import { app } from "../lib/state.svelte";
  import Icon from "./Icon.svelte";
  import { item, rise } from "../lib/motion";
  import { TERMS_TEXT, TERMS_VERSION } from "../lib/terms";

  let settings = $state<Settings | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    api.getSettings().then((s) => (settings = s));
  });
  // options saved elsewhere (the mascot ones, the room menu) show up here too
  $effect(() => {
    if (app.settings) settings = app.settings;
  });

  async function save(patch: Partial<Settings>) {
    if (!settings) return;
    try {
      settings = await api.setSettings({ ...settings, ...patch });
      app.settings = settings;
      error = null;
    } catch (e) {
      error = errorText(e);
    }
  }

  async function pickDownloadDir() {
    const dir = await open({ directory: true, defaultPath: settings?.download_dir });
    if (typeof dir === "string") save({ download_dir: dir });
  }

  async function addLibraryDir() {
    const dir = await open({ directory: true });
    if (typeof dir === "string" && settings && !settings.library_dirs.includes(dir)) {
      save({ library_dirs: [...settings.library_dirs, dir] });
    }
  }

  const qualities: { id: Quality; label: string; hint: string }[] = [
    { id: "low", label: "Low", hint: "96 kbps AAC" },
    { id: "normal", label: "Normal", hint: "320 kbps AAC" },
    { id: "high", label: "Lossless", hint: "16-bit / 44.1 kHz FLAC" },
    { id: "max", label: "Max", hint: "up to 24-bit / 192 kHz FLAC" },
  ];

  const progressStyles: { id: ProgressStyle; label: string }[] = [
    { id: "classic", label: "Classic" },
    { id: "wavy", label: "Wavy" },
    { id: "waveform", label: "Waveform" },
    { id: "neon", label: "Neon" },
    { id: "led", label: "LED" },
    { id: "minimal", label: "Minimal" },
  ];

  const motions: { id: Motion; label: string; hint: string }[] = [
    { id: "auto", label: "Automatic", hint: "Full, or subtle when the system asks for less motion" },
    { id: "full", label: "Full", hint: "Views slide in, menus pop, lists cascade" },
    { id: "subtle", label: "Subtle", hint: "Short fades, nothing moves around" },
    { id: "off", label: "Off", hint: "Everything appears at once" },
  ];

  const themes: { id: Theme; label: string; hint: string }[] = [
    { id: "system", label: "System", hint: "Follows macOS / Windows / Linux" },
    { id: "light", label: "Light", hint: "Always light" },
    { id: "dark", label: "Dark", hint: "Always dark" },
  ];

  async function logout() {
    const ok = await app.confirm({
      title: "Log out of Tidal?",
      text: "Your library and downloaded files stay. You'll need to connect again to search, stream and download.",
      confirmLabel: "Log out",
      danger: true,
    });
    if (!ok) return;
    await api.logout();
    app.account = null;
  }

  async function removeLibraryDir(dir: string) {
    const ok = await app.confirm({
      title: "Remove this music folder?",
      text: `${dir} will stop showing in your library. The files stay on disk.`,
      confirmLabel: "Remove folder",
      danger: true,
    });
    if (ok && settings) save({ library_dirs: settings.library_dirs.filter((d) => d !== dir) });
  }

  type PageId = "look" | "room" | "lyrics" | "mascot" | "share" | "downloads" | "library" | "tidal" | "about";
  // words are what the search box matches, besides the title
  const pages: { id: PageId; title: string; icon: string; words: string }[] = [
    { id: "look", title: "Look", icon: "palette", words: "appearance theme light dark system skin layout everywhere whole app progress bar seek wavy neon led thickness animate motion animation animations transitions effects movement reduce reduced still calm" },
    { id: "room", title: "Listening room", icon: "disc", words: "layout player skin theme everywhere whole app look classic immersive studio aluminum cover flow retro amp winamp luna xp web 2000 vinyl signal chain vu meters visualizer halo album colors shade shadow gradient readable title" },
    { id: "lyrics", title: "Lyrics", icon: "lyrics", words: "lyrics karaoke words sing singing song text lines synced lrc musixmatch size timing offset delay bluetooth late early open automatically mascot homefito microphone" },
    { id: "mascot", title: "Mascot", icon: "smile", words: "mascot dance style mood turns arms legs angry temper click right-click" },
    {
      id: "share",
      title: "Shared control & party",
      icon: "cast",
      words: "share shared control party hub guest join code qr port rules permissions add approve limit skip vote turns queue order explicit remove move notices speaker delay bluetooth sync play along nearby discover announce network mdns bonjour visualizer levels name colour color",
    },
    { id: "downloads", title: "Downloads", icon: "download", words: "quality lossless max hi-res flac aac cover lyrics skip existing folder" },
    { id: "library", title: "Music folders", icon: "folder", words: "library folders music add remove scan" },
    { id: "tidal", title: "Tidal account", icon: "user", words: "tidal account login log out connect lists scroll lazy load pages playlists favorites infinite" },
    { id: "about", title: "About", icon: "info", words: "about version ffmpeg data personal use terms notice disclaimer copyright accepted" },
  ];

  const STORE = "homefi.settings.page";
  let page = $state<PageId>(readPage());
  let query = $state("");

  function readPage(): PageId {
    try {
      const saved = localStorage.getItem(STORE);
      if (pages.some((p) => p.id === saved)) return saved as PageId;
    } catch {}
    return "look";
  }

  function go(id: PageId) {
    page = id;
    try {
      localStorage.setItem(STORE, id);
    } catch {}
  }

  const shown = $derived.by(() => {
    const words = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
    if (!words.length) return pages;
    return pages.filter((p) => words.every((w) => `${p.title} ${p.words}`.toLowerCase().includes(w)));
  });
  // typing jumps to the first match
  $effect(() => {
    if (query.trim() && shown.length && !shown.some((p) => p.id === page)) page = shown[0].id;
  });

  function summary(id: PageId, s: Settings): string {
    switch (id) {
      case "look": {
        const fixed = fixedTheme(skinOf(s.room));
        const theme = fixed ? themes.find((t) => t.id === fixed)?.label : themes.find((t) => t.id === (s.theme ?? "system"))?.label;
        const motion = s.room.motion && s.room.motion !== "auto" ? ` · ${motions.find((m) => m.id === s.room.motion)?.label.toLowerCase()} motion` : "";
        return `${theme} · ${progressStyles.find((p) => p.id === (s.progress_style ?? "wavy"))?.label} bar${motion}`;
      }
      case "room":
        return `${LAYOUTS.find((l) => l.id === layoutOf(s.room.layout))?.label ?? ""}${s.room.layout_everywhere === false ? " · room only" : " · everywhere"}`;
      case "lyrics":
        return `${s.room.lyrics_mode === "karaoke" ? "Karaoke" : "Normal"} · ${sizeLabels[s.room.lyrics_size] ?? "Medium"} text${s.room.lyrics_offset_ms ? ` · ${offsetLabel(s.room.lyrics_offset_ms)}` : ""}`;
      case "mascot":
        return s.room.mascot ? "On" : "Off";
      case "share":
        return share.sharing ? `Sharing · ${share.hub.guests.length} guests` : share.inRoom ? `In ${share.guest.session_name}` : `As “${s.share.name || share.name}”`;
      case "downloads":
        return qualities.find((q) => q.id === s.quality)?.label ?? "";
      case "library":
        return s.library_dirs.length === 1 ? "1 folder" : `${s.library_dirs.length} folders`;
      case "tidal":
        return `${app.account ? "Connected" : "Not connected"}${s.lazy_lists === false ? " · whole lists" : ""}`;
      case "about":
        return `HomeFi ${app.info?.version ?? ""}`;
    }
  }

  const current = $derived(pages.find((p) => p.id === page)!);
  // the layout dresses the whole app; some keep their era's light / dark
  const skin = $derived(settings ? skinOf(settings.room) : "classic");
  const skinLabel = $derived(LAYOUTS.find((l) => l.id === skin)?.label ?? "");
  const lockedTheme = $derived(fixedTheme(skin));

  const sizeLabels: Record<string, string> = { small: "Small", medium: "Medium", large: "Large" };
  const offsetLabel = (ms: number) => (ms === 0 ? "In time" : `${ms > 0 ? "+" : "−"}${(Math.abs(ms) / 1000).toFixed(1)} s ${ms > 0 ? "later" : "earlier"}`);
  function saveRoom(patch: Partial<Settings["room"]>) {
    if (settings) save({ room: { ...settings.room, ...patch } });
  }

  function saveShare(patch: Partial<ShareSettings>) {
    if (settings) save({ share: { ...settings.share, ...patch } });
  }
  const COLORS = ["#0891B2", "#1D4ED8", "#10B981", "#F59E0B", "#EF4444", "#EC4899", "#8B5CF6", "#64748B"];
  const addModes = [
    { id: "free", label: "Freely" },
    { id: "approve", label: "You approve" },
    { id: "limit", label: "Up to a limit" },
  ] as const;
  const orders = [
    { id: "first_come", label: "First come", hint: "Songs play in the order they were added" },
    { id: "turns", label: "Turns", hint: "One song from each person in turn; you count as one more" },
  ] as const;
  const skips = [
    { id: "nobody", label: "Nobody" },
    { id: "everyone", label: "Everyone" },
    { id: "vote", label: "By vote" },
  ] as const;
  const edits = [
    { id: "own", label: "Their own" },
    { id: "all", label: "Any" },
    { id: "none", label: "None" },
  ] as const;
</script>

{#if settings}
<div class="settings">
  <nav>
    <h1>Settings</h1>
    <label class="search">
      <Icon name="search" size={16} />
      <input type="search" placeholder="Search settings" bind:value={query} />
    </label>
    {#each shown as p (p.id)}
      <button class="page" in:item|global class:selected={p.id === page} onclick={() => go(p.id)}>
        <span class="badge"><Icon name={p.icon} size={18} /></span>
        <span class="grow"><b>{p.title}</b><small class="muted ellipsis">{summary(p.id, settings)}</small></span>
      </button>
    {:else}
      <p class="muted small none">Nothing matches “{query}”.</p>
    {/each}
  </nav>

  <div class="pane">
  <!-- each section slides in when it is picked -->
  {#key page}
  <div in:rise={{ y: 12 }}>
  <h2 class="title">{current.title}</h2>
  {#if page === "look"}
  <section>
    <h3>Theme</h3>
    <div class="segmented themes" class:locked={!!lockedTheme}>
      {#each themes as t}
        <button class:selected={(settings.theme ?? "system") === t.id} onclick={() => save({ theme: t.id })}>
          <b>{t.label}</b><span>{t.hint}</span>
        </button>
      {/each}
    </div>
    {#if lockedTheme}
      <p class="note-line"><Icon name="info" size={14} /> {skinLabel} is always {lockedTheme}, the look of its era. Turn off “Use the layout everywhere” to pick the theme yourself.</p>
    {/if}
    <h3>Skin</h3>
    <label class="toggle top">
      <input type="checkbox" checked={settings.room.layout_everywhere ?? true} onchange={(e) => save({ room: { ...settings!.room, layout_everywhere: e.currentTarget.checked } })} />
      <span>Use the layout everywhere<small class="muted">The room layout ({LAYOUTS.find((l) => l.id === layoutOf(settings!.room.layout))?.label}) also dresses the sidebar, the player bar, lists, menus and Settings</small></span>
    </label>
    <button class="link-btn" onclick={() => go("room")}>Choose the layout in Listening room</button>
  </section>

  <section>
    <h3>Progress bar</h3>
    <p class="muted small">How the seek bar under the player looks. Click or drag it to seek.</p>
    <div class="bars">
      {#each progressStyles as s}
        <button class="bar" class:selected={(settings.progress_style ?? "wavy") === s.id} onclick={() => save({ progress_style: s.id })}>
          <b>{s.label}</b>
          <SeekBar fraction={0.62} style={s.id} thickness={settings.progress_thickness} animated={settings.progress_animated} playing seed={7} />
        </button>
      {/each}
    </div>
    <div class="segmented thickness">
      {#each [["thin", "Thin"], ["medium", "Medium"], ["thick", "Thick"]] as [id, label]}
        <button class:selected={settings.progress_thickness === id} onclick={() => save({ progress_thickness: id as "thin" })}><b>{label}</b></button>
      {/each}
    </div>
    <label class="toggle"><input type="checkbox" checked={settings.progress_animated} onchange={(e) => save({ progress_animated: e.currentTarget.checked })} /> Animate while playing</label>
  </section>

  <section>
    <h3>Motion</h3>
    <p class="muted small">How the app moves: views sliding in, menus and dialogs opening, lists arriving one row after another, the song changing. The skin gives it its character; the retro ones move in a few quick frames.</p>
    <div class="segmented">
      {#each motions as m}
        <button class:selected={(settings.room.motion ?? "auto") === m.id} onclick={() => saveRoom({ motion: m.id })}>
          <b>{m.label}</b><span>{m.hint}</span>
        </button>
      {/each}
    </div>
  </section>
  {/if}

  {#if page === "downloads"}
  <section>
    <h3>Quality</h3>
    <div class="segmented">
      {#each qualities as q}
        <button class:selected={settings.quality === q.id} onclick={() => save({ quality: q.id })}>
          <b>{q.label}</b><span>{q.hint}</span>
        </button>
      {/each}
    </div>
    <label class="toggle"><input type="checkbox" checked={settings.embed_cover} onchange={(e) => save({ embed_cover: e.currentTarget.checked })} /> Embed cover art</label>
    <label class="toggle"><input type="checkbox" checked={settings.embed_lyrics} onchange={(e) => save({ embed_lyrics: e.currentTarget.checked })} /> Embed lyrics</label>
    <label class="toggle"><input type="checkbox" checked={settings.skip_existing} onchange={(e) => save({ skip_existing: e.currentTarget.checked })} /> Skip files that already exist</label>
  </section>

  <section>
    <h3>Save downloads in</h3>
    <div class="folder">
      <Icon name="download" />
      <div class="grow">
        <div class="ellipsis">{settings.download_dir}</div>
      </div>
      <button class="icon-btn" title="Show in Finder / Explorer" onclick={() => revealItemInDir(settings!.download_dir)}><Icon name="external" /></button>
      <button class="btn tonal" onclick={pickDownloadDir}>Change</button>
    </div>
  </section>
  {/if}

  {#if page === "room"}
  <section>
    <p class="muted small">Choose what the now-playing room shows. Everything can be turned off.</p>
    <h3>Layout</h3>
    <div class="segmented layouts">
      {#each LAYOUTS as l}
        <button class:selected={layoutOf(settings.room.layout) === l.id} onclick={() => save({ room: { ...settings!.room, layout: l.id } })}>
          <b>{l.label}</b><span>{l.hint}</span>
        </button>
      {/each}
    </div>
    <label class="toggle top">
      <input type="checkbox" checked={settings.room.layout_everywhere ?? true} onchange={(e) => save({ room: { ...settings!.room, layout_everywhere: e.currentTarget.checked } })} />
      <span>Use the layout everywhere<small class="muted">The whole app takes the layout's look, not only the now-playing room. Studio and Retro Amp are always dark; Luna, Aluminum and Web 2000 always light</small></span>
    </label>
    {#if layoutOf(settings.room.layout) === "immersive"}
      <h3>Shade under the title</h3>
      <p class="muted small">So the title reads on any album. Auto makes it darker on light covers.</p>
      <div class="segmented">
        {#each [["auto", "Auto"], ["strong", "Strong"], ["soft", "Soft"], ["off", "Off"]] as [id, label]}
          <button class:selected={(settings.room.immersive_shade ?? "auto") === id} onclick={() => save({ room: { ...settings!.room, immersive_shade: id as "auto" } })}><b>{label}</b></button>
        {/each}
      </div>
    {/if}
    <h3>Show</h3>
    <div class="room-grid">
      {#each [["signal_chain", "Signal chain"], ["vu_meters", "VU meters"], ["visualizer", "Visualizer"], ["halo", "Halo around the cover"], ["album_colors", "Colors from the album art"]] as [key, label]}
        <label class="toggle">
          <input
            type="checkbox"
            checked={settings.room[key as keyof typeof settings.room] as boolean}
            onchange={(e) => save({ room: { ...settings!.room, [key]: e.currentTarget.checked } })}
          />
          {label}
        </label>
      {/each}
    </div>
  </section>
  {/if}

  {#if page === "lyrics"}
  <section>
    <p class="muted small">The song's words in the listening room: open them with the Lyrics button in the room or the player bar. They come from the file (a .lrc next to it, or the lyrics tag) or from Tidal.</p>
    <h3>Mode</h3>
    <div class="segmented">
      <button class:selected={settings.room.lyrics_mode !== "karaoke"} onclick={() => saveRoom({ lyrics_mode: "normal" })}>
        <b>Normal</b><span>The whole text, the line being sung lit and followed</span>
      </button>
      <button class:selected={settings.room.lyrics_mode === "karaoke"} onclick={() => saveRoom({ lyrics_mode: "karaoke" })}>
        <b>Karaoke</b><span>The line being sung, big, filling with color as it goes</span>
      </button>
    </div>
    <h3>Text size</h3>
    <div class="segmented thickness">
      {#each ["small", "medium", "large"] as const as id}
        <button class:selected={settings.room.lyrics_size === id} onclick={() => saveRoom({ lyrics_size: id })}><b>{sizeLabels[id]}</b></button>
      {/each}
    </div>
    <label class="field range">
      <span>Lyrics timing: {offsetLabel(settings.room.lyrics_offset_ms)}</span>
      <input type="range" min="-2000" max="2000" step="100" value={settings.room.lyrics_offset_ms} onchange={(e) => saveRoom({ lyrics_offset_ms: Number(e.currentTarget.value) })} />
      <small class="muted">Bluetooth speakers and headphones play a little late: move the lyrics later until they match the voice</small>
    </label>
    <label class="toggle top">
      <input type="checkbox" checked={settings.room.lyrics_auto_open} onchange={(e) => saveRoom({ lyrics_auto_open: e.currentTarget.checked })} />
      <span>Open the lyrics on their own<small class="muted">When a song that has lyrics starts, the room shows them</small></span>
    </label>
    <label class="toggle top">
      <input type="checkbox" checked={settings.room.lyrics_mascot} onchange={(e) => saveRoom({ lyrics_mascot: e.currentTarget.checked })} />
      <span>HomeFito sings in karaoke<small class="muted">The mascot stands beside the words, singing into a microphone while the song plays</small></span>
    </label>
  </section>
  {/if}

  {#if page === "mascot"}
  <section>
    <p class="muted small">A little character that dances to the music in the listening room.</p>
    <label class="toggle"><input type="checkbox" checked={settings.room.mascot} onchange={(e) => save({ room: { ...settings!.room, mascot: e.currentTarget.checked } })} /> Show the mascot</label>
    {#if settings.room.mascot}
      <div class="mascot-opts"><MascotOptions /></div>
    {/if}
  </section>
  {/if}

  {#if page === "share"}
  {@const sh = settings.share}
  <section>
    <p class="muted small">One computer or phone plays the music (the hub); others join from theirs to add songs, or play along on their own speakers. On a computer it works over the network it is on (Wi-Fi or Ethernet).</p>
    <h3>You</h3>
    <div class="fields">
      <label class="field">
        <span>Your name</span>
        <input value={sh.name} placeholder={share.name} maxlength="32" onchange={(e) => saveShare({ name: e.currentTarget.value.trim() })} />
      </label>
      <div class="field">
        <span>Your colour</span>
        <div class="colors">
          {#each COLORS as c}
            <button class="swatch" class:on={sh.color.toLowerCase() === c.toLowerCase()} style:background={c} aria-label="Colour {c}" onclick={() => saveShare({ color: c })}></button>
          {/each}
          <input type="color" value={sh.color} aria-label="Another colour" onchange={(e) => saveShare({ color: e.currentTarget.value })} />
        </div>
      </div>
    </div>
    <label class="field range">
      <span>Speaker delay: {sh.speaker_delay_ms} ms</span>
      <input type="range" min="-500" max="1000" step="10" value={sh.speaker_delay_ms} onchange={(e) => saveShare({ speaker_delay_ms: Number(e.currentTarget.value) })} />
      <small class="muted">This computer’s speaker latency (Bluetooth speakers lag 100-300 ms); used as a guest and as the hub</small>
    </label>

    <h3>This computer as the hub</h3>
    <div class="fields">
      <label class="field">
        <span>Session name</span>
        <input value={sh.session_name} placeholder={share.sessionName} maxlength="60" onchange={(e) => saveShare({ session_name: e.currentTarget.value.trim() })} />
      </label>
      <label class="field port">
        <span>Port</span>
        <input type="number" min="0" max="65535" value={sh.port} onchange={(e) => saveShare({ port: Math.max(0, Math.min(65535, Number(e.currentTarget.value) || 0)) })} />
      </label>
    </div>
    <small class="muted">Port 47800 by default; 0 picks a free one. Changes apply the next time you start sharing.</small>
    <label class="toggle"><input type="checkbox" checked={sh.party_default} onchange={(e) => saveShare({ party_default: e.currentTarget.checked })} /> Start in party mode</label>
    <div class="row-setting">
      <span class="label">Join code</span>
      <div class="segmented compact">
        <button class:selected={!sh.fixed_code} onclick={() => saveShare({ fixed_code: false })}><b>New one each time</b></button>
        <button class:selected={sh.fixed_code} onclick={() => saveShare({ fixed_code: true, code: sh.code || String(Math.floor(1000 + Math.random() * 9000)) })}><b>Always the same</b></button>
      </div>
      {#if sh.fixed_code}
        <input class="code" value={sh.code} inputmode="numeric" maxlength="8" aria-label="Join code" onchange={(e) => saveShare({ code: e.currentTarget.value.replace(/\D/g, "") })} />
      {/if}
    </div>
    <div class="row-setting">
      <span class="label">Guests add songs</span>
      <div class="segmented compact">
        {#each addModes as m}<button class:selected={sh.add === m.id} onclick={() => saveShare({ add: m.id })}><b>{m.label}</b></button>{/each}
      </div>
      {#if sh.add === "limit"}
        <label class="inline">At most <input type="number" min="1" max="50" value={sh.max_per_guest} onchange={(e) => saveShare({ max_per_guest: Math.max(1, Number(e.currentTarget.value) || 1) })} /> waiting songs per person</label>
      {/if}
    </div>
    <div class="row-setting">
      <span class="label">Queue order</span>
      <div class="segmented compact">
        {#each orders as o}<button class:selected={sh.order === o.id} onclick={() => saveShare({ order: o.id })}><b>{o.label}</b></button>{/each}
      </div>
      <small class="muted">{orders.find((o) => o.id === sh.order)?.hint}</small>
    </div>
    <div class="row-setting">
      <span class="label">Who can skip</span>
      <div class="segmented compact">
        {#each skips as k}<button class:selected={sh.skip === k.id} onclick={() => saveShare({ skip: k.id })}><b>{k.label}</b></button>{/each}
      </div>
    </div>
    <div class="row-setting">
      <span class="label">Remove and move songs</span>
      <div class="segmented compact">
        {#each edits as k}<button class:selected={sh.edit === k.id} onclick={() => saveShare({ edit: k.id })}><b>{k.label}</b></button>{/each}
      </div>
    </div>
    <label class="toggle"><input type="checkbox" checked={sh.playback} onchange={(e) => saveShare({ playback: e.currentTarget.checked })} /> Guests can pause and change the volume</label>
    <label class="toggle"><input type="checkbox" checked={sh.allow_explicit} onchange={(e) => saveShare({ allow_explicit: e.currentTarget.checked })} /> Allow explicit songs</label>
    <label class="toggle"><input type="checkbox" checked={sh.drop_songs_on_leave} onchange={(e) => saveShare({ drop_songs_on_leave: e.currentTarget.checked })} /> Remove a guest’s songs when they leave</label>
    <label class="field range">
      <span>{sh.party_start_wait_sec ? `New song: wait up to ${sh.party_start_wait_sec} s for everyone` : "New song: start at once"}</span>
      <input type="range" min="0" max="10" step="1" value={sh.party_start_wait_sec} onchange={(e) => saveShare({ party_start_wait_sec: Number(e.currentTarget.value) })} />
      <small class="muted">In a party, each new song waits until every guest has it loaded, then everyone starts at the same moment. Songs are downloaded ahead, so it is usually under a second.</small>
    </label>
    <label class="toggle top">
      <input type="checkbox" checked={sh.send_levels} onchange={(e) => saveShare({ send_levels: e.currentTarget.checked })} />
      <span>Send the visualizer to guests<small class="muted">Guests in remote control see the meters, spectrum and mascot move with your music. A few hundred bytes a second.</small></span>
    </label>
    <h4>Notices on the hub <small class="muted">Small and silent, they go away by themselves</small></h4>
    <div class="room-grid">
      <label class="toggle"><input type="checkbox" checked={sh.notice_joined} onchange={(e) => saveShare({ notice_joined: e.currentTarget.checked })} /> Someone joins</label>
      <label class="toggle"><input type="checkbox" checked={sh.notice_left} onchange={(e) => saveShare({ notice_left: e.currentTarget.checked })} /> Someone leaves</label>
      <label class="toggle"><input type="checkbox" checked={sh.notice_added} onchange={(e) => saveShare({ notice_added: e.currentTarget.checked })} /> Someone adds a song</label>
      <label class="toggle"><input type="checkbox" checked={sh.notice_approval} onchange={(e) => saveShare({ notice_approval: e.currentTarget.checked })} /> A song waits for your OK</label>
    </div>
    <label class="field range">
      <span>Notices stay {sh.notice_seconds} s</span>
      <input type="range" min="1" max="30" step="1" value={sh.notice_seconds} onchange={(e) => saveShare({ notice_seconds: Number(e.currentTarget.value) })} />
    </label>

    <h3>This computer as a guest</h3>
    <label class="toggle"><input type="checkbox" checked={sh.play_along} onchange={(e) => saveShare({ play_along: e.currentTarget.checked })} /> Play the party on this computer by default</label>
    <label class="toggle top">
      <input type="checkbox" checked={sh.guest_opens_player} onchange={(e) => saveShare({ guest_opens_player: e.currentTarget.checked })} />
      <span>Open on the player<small class="muted">Joining a session shows the hub’s song in your player layout; off, it opens on the room (queue, votes and volume)</small></span>
    </label>
    <label class="toggle top">
      <input type="checkbox" checked={sh.sync_by_speed} onchange={(e) => saveShare({ sync_by_speed: e.currentTarget.checked })} />
      <span>Smooth sync<small class="muted">Closes small gaps by playing a touch faster or slower. This computer’s player can’t change speed yet, so here it always jumps when it is more than 80 ms off; phones use it.</small></span>
    </label>
    <label class="toggle top">
      <input type="checkbox" checked={sh.preload_songs} onchange={(e) => saveShare({ preload_songs: e.currentTarget.checked })} />
      <span>Download the songs ahead<small class="muted">Keeps the next and the previous song on this computer and plays them from here, so the party doesn’t cut out when the network is busy. Deleted when you leave.</small></span>
    </label>

    <h3>Finding each other</h3>
    <label class="toggle"><input type="checkbox" checked={sh.discover} onchange={(e) => saveShare({ discover: e.currentTarget.checked })} /> Look for music nearby</label>
    <label class="toggle"><input type="checkbox" checked={sh.discover_everywhere} disabled={!sh.discover} onchange={(e) => saveShare({ discover_everywhere: e.currentTarget.checked })} /> Offer it on any screen, not only in the listening room</label>
    <label class="toggle top">
      <input type="checkbox" checked={sh.announce_lan} onchange={(e) => saveShare({ announce_lan: e.currentTarget.checked })} />
      <span>Announce the session on the network<small class="muted">Phones and computers on this network find it without typing an address (mDNS / Bonjour). They still need the join code.</small></span>
    </label>
  </section>
  {/if}

  {#if page === "library"}
  <section>
    <p class="muted small">HomeFi plays and lists the music in these folders. The downloads folder is always included.</p>
    {#each settings.library_dirs as dir}
      <div class="folder">
        <Icon name="folder" />
        <div class="grow"><div class="ellipsis">{dir}</div></div>
        <button class="icon-btn" title="Remove" aria-label="Remove {dir}" onclick={() => removeLibraryDir(dir)}>
          <Icon name="x" />
        </button>
      </div>
    {/each}
    <button class="btn tonal add" onclick={addLibraryDir}><Icon name="folder" size={16} /> Add music folder</button>
  </section>
  {/if}

  {#if page === "tidal"}
  <section>
    {#if app.account}
      <p>Connected · user {app.account.userId} · {app.account.countryCode}</p>
      <button class="btn tonal danger" onclick={logout}>Log out</button>
    {:else}
      <p class="muted">Not connected. The player works without an account.</p>
      <button class="btn primary" onclick={() => (app.loginOpen = true)}>Connect Tidal</button>
    {/if}
    <h3>Lists</h3>
    <label class="toggle top">
      <input type="checkbox" checked={settings.lazy_lists ?? true} onchange={(e) => save({ lazy_lists: e.currentTarget.checked })} />
      <span>Load lists as you scroll<small class="muted">Playlists, albums and favorites show their first tracks right away and bring more as you scroll. Off, each list loads whole before it shows. Play, Shuffle and Download always use the whole list.</small></span>
    </label>
  </section>
  {/if}

  {#if page === "about"}
  <section>
    <p class="muted small">
      HomeFi {app.info?.version} · {app.info?.platform} · ffmpeg {app.info?.ffmpeg ? "found" : "not found (not needed for FLAC / Hi-Res)"}<br />
      Data: {app.info?.appDir}
    </p>
    <h3>Personal use</h3>
    <p class="muted small terms">{TERMS_TEXT}</p>
    <p class="muted small">
      {#if settings && settings.tidal_terms_accepted >= TERMS_VERSION}
        Accepted{settings.tidal_terms_accepted_at ? ` on ${new Date(settings.tidal_terms_accepted_at).toLocaleDateString()}` : ""}.
      {:else}
        Not accepted yet. You'll be asked when you connect Tidal.
      {/if}
    </p>
  </section>
  {/if}
  </div>
  {/key}
  {#if error}<p class="error">{error}</p>{/if}
  </div>
</div>
{/if}

<style>
  .settings {
    display: grid;
    grid-template-columns: 260px minmax(0, 1fr);
    gap: 28px;
    align-items: start;
    container-type: inline-size;
  }
  nav {
    position: sticky;
    top: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  nav h1 {
    margin-bottom: 10px;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    margin-bottom: 8px;
    border-radius: min(10px, var(--r-max));
    background: var(--surface-2);
    border: 1px solid var(--border);
    color: var(--muted);
  }
  .search input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    padding: 0;
    box-shadow: none;
  }
  .page {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 10px;
    border-radius: min(10px, var(--r-max));
    text-align: left;
    transition: background 0.2s;
  }
  .page:hover:not(.selected) {
    background: var(--surface-2);
  }
  .page.selected {
    background: color-mix(in srgb, var(--primary) 14%, var(--surface));
  }
  .page.selected b {
    color: var(--primary);
  }
  .page .grow {
    display: flex;
    flex-direction: column;
  }
  .page small {
    font-size: 12px;
  }
  .badge {
    display: grid;
    place-items: center;
    flex: none;
    width: 32px;
    height: 32px;
    border-radius: min(9px, var(--r-max));
    background: var(--surface-2);
    color: var(--primary);
    transition: background 0.2s, color 0.2s;
  }
  .page.selected .badge {
    background: var(--primary);
    color: var(--on-primary, #fff);
    animation: hf-pick calc(0.36s * var(--motion-speed)) var(--ease-pop);
  }
  .none {
    padding: 8px 10px;
  }
  .pane {
    min-width: 0;
    padding-top: 6px;
  }
  .title {
    margin: 0 0 12px;
    font-size: 22px;
  }
  section {
    max-width: 760px;
    margin-bottom: 20px;
  }
  /* narrow window: the sections become a strip of chips above the page */
  @media (max-width: 820px) {
    .settings {
      grid-template-columns: minmax(0, 1fr);
      gap: 12px;
    }
    nav {
      position: static;
      flex-direction: row;
      flex-wrap: wrap;
      gap: 6px;
    }
    nav h1,
    .search {
      flex-basis: 100%;
    }
    .page {
      padding: 6px 12px 6px 6px;
      border: 1px solid var(--border);
      border-radius: min(999px, var(--r-max));
      gap: 8px;
    }
    .page small {
      display: none;
    }
    .badge {
      width: 26px;
      height: 26px;
      border-radius: min(999px, var(--r-max));
    }
  }
  .segmented {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
    gap: 8px;
    margin-bottom: 12px;
  }
  .segmented button {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 12px;
    border-radius: min(12px, var(--r-max));
    background: var(--surface-2);
    border: 1px solid var(--border);
    text-align: left;
    transition: background 0.2s, border-color 0.2s, box-shadow 0.2s;
  }
  .segmented button span {
    font-size: 12px;
    color: var(--muted);
  }
  .segmented button:hover:not(.selected) {
    border-color: color-mix(in srgb, var(--primary) 45%, var(--border));
  }
  .segmented button.selected {
    border-color: var(--primary);
    box-shadow: inset 0 0 0 1px var(--primary);
    background: color-mix(in srgb, var(--primary) 14%, var(--surface));
  }
  .segmented button.selected b {
    color: var(--primary);
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 0;
  }
  .toggle input {
    flex: none;
    width: 16px;
    height: 16px;
    accent-color: var(--primary-strong);
  }
  h3 {
    margin: 0 0 8px;
    font-size: 14px;
  }
  h3:not(:first-child) {
    margin-top: 20px;
  }
  .mascot-opts {
    max-width: 560px;
  }
  .fields {
    display: flex;
    flex-wrap: wrap;
    gap: 12px 16px;
    margin-bottom: 8px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 220px;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--muted);
  }
  .field input:not([type="range"]):not([type="color"]) {
    font-weight: 400;
    font-size: 14px;
  }
  .field.port {
    min-width: 0;
    width: 110px;
  }
  .field.range {
    margin: 10px 0 6px;
    max-width: 560px;
  }
  .field.range input {
    accent-color: var(--primary-strong);
  }
  .field small,
  .toggle small,
  h4 small {
    font-weight: 400;
    font-size: 12px;
  }
  .colors {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 38px;
  }
  .swatch {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    box-shadow: 0 0 0 2px var(--bg), 0 0 0 3px transparent;
  }
  .swatch.on {
    box-shadow: 0 0 0 2px var(--bg), 0 0 0 4px var(--text);
  }
  .colors input[type="color"] {
    width: 30px;
    height: 26px;
    padding: 0 2px;
  }
  .row-setting {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 12px 0 6px;
  }
  .row-setting .label {
    font-size: 13px;
    font-weight: 600;
  }
  .segmented.compact {
    grid-template-columns: repeat(auto-fit, minmax(120px, 160px));
    margin-bottom: 0;
  }
  .segmented.compact button {
    padding: 8px 12px;
  }
  .code {
    width: 140px;
    font: 700 18px var(--font-mono);
    letter-spacing: 0.25em;
  }
  .inline {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
  }
  .inline input {
    width: 64px;
  }
  .toggle.top {
    align-items: flex-start;
  }
  .toggle.top input {
    margin-top: 3px;
  }
  .toggle span {
    display: flex;
    flex-direction: column;
  }
  h4 {
    display: flex;
    flex-direction: column;
    margin: 18px 0 4px;
    font-size: 13px;
  }
  /* the skin fixes light / dark: the choice is kept for when it doesn't */
  .themes.locked button {
    opacity: 0.55;
  }
  .note-line {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: -4px 0 12px;
    font-size: 12.5px;
    color: var(--muted);
  }
  .link-btn {
    padding: 2px 0;
    font-size: 12.5px;
    color: var(--primary);
  }
  .link-btn:hover {
    text-decoration: underline;
  }
  .room-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 0 24px;
  }
  .folder {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    margin-bottom: 6px;
    border-radius: min(10px, var(--r-max));
    background: var(--surface-2);
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .small {
    font-size: 12.5px;
  }
  .terms {
    max-width: 620px;
    line-height: 1.55;
  }
  .add {
    margin-top: 4px;
  }
  .danger {
    color: var(--danger);
  }
  .error {
    color: var(--danger);
  }
  .bars {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 10px;
    margin: 10px 0;
  }
  .bar {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 12px;
    text-align: left;
    border-radius: min(12px, var(--r-max));
    border: 1px solid var(--border);
    background: var(--surface-2);
  }
  .bar {
    transition: border-color 0.2s, box-shadow 0.2s;
  }
  .bar.selected {
    animation: hf-pick calc(0.32s * var(--motion-speed)) var(--ease-pop);
    border-color: var(--accent-text);
    box-shadow: inset 0 0 0 1px var(--accent-text);
  }
  .bar :global(canvas) {
    flex: none;
  }
  .thickness {
    max-width: 560px;
    grid-template-columns: repeat(3, 1fr);
  }
</style>
