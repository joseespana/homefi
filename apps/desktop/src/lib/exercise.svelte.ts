// Exercise mode's state on the screen: where the ride is (setup, building, preview, ride, done), the
// request being edited, the newest ride view (10 a second from the `ride` event), the cues and the TV.
// It lives here, not in a component, so leaving the screen does not stop a build or a ride. Components
// draw from here and send intents; every rule (the course, the tempo fit, the clock) is homefi-core's.
import { api, errorText } from "./api";
import { assistant } from "./assistant.svelte";
import { CuePlayer } from "./exerciseCues";
import {
  EX_LANGUAGE,
  EX_MAX_GENRES,
  EX_MAX_GENRE_CHARS,
  EX_MAX_ARTIST_CHARS,
  EX_MAX_SENTENCE_CHARS,
  EX_MAX_TASTE_ARTISTS,
  EX_MINUTES,
  PRESET_IDS,
  RIDE_FILE,
  exerciseApi,
  exerciseSettingsOf,
  onExercise,
  positionLabel,
  zoneName,
  type ExerciseSettings,
  type ExerciseState,
  type ExerciseTurn,
  type MonitorView,
  type PresetView,
  type RideBuilt,
  type RidePlan,
  type RideProgress,
  type RideRequest,
  type RideTaste,
  type RideView,
  type SavedRide,
} from "./exercise-api";
import { app } from "./state.svelte";
import { pickOpenPath, pickSavePath } from "./transfer";

/** The two halves of the setup screen. */
export type SetupTab = "new" | "rides";
export type ExerciseStep = "setup" | "building" | "preview" | "ride" | "done";
/** Where the ride is shown besides this screen. */
export type TvMode = "none" | "window" | "app";

const MAX_CHAT_MESSAGES = 40;

export interface ChatMessage {
  who: "ai" | "me";
  text: string;
}

const emptyRequest = (): RideRequest => ({ minutes: EX_MINUTES.min * 3, style: "first_ride", wish: { genres: [], mix: true, sentence: "" }, workout: null });

class ExerciseStore {
  /** What the backend said last (the stage, the plan, the TV). */
  state = $state.raw<ExerciseState | null>(null);
  /** The newest ride view; replaced whole on every tick (raw: one signal). */
  view = $state.raw<RideView | null>(null);
  playing = $state(false);
  /** The ride being set up; the setup screen edits it, the AI chat may change it. */
  request = $state<RideRequest>(emptyRequest());
  presets = $state.raw<PresetView[]>([]);
  /** "Change setup" from the preview. */
  editing = $state(false);
  /** A build started here is running (the stage event confirms it a moment later). */
  building = $state(false);
  /** The running build is "new songs" for the plan on the screen: the preview stays, with the progress in it. */
  rebuilding = $state(false);
  /** Why the last "new songs" failed (the previous plan stays). */
  rebuildError = $state<string | null>(null);
  /** The plan on the screen while "new songs" runs (the backend reports none while it builds). */
  private keptPlan = $state.raw<RidePlan | null>(null);
  /** Artists the library has the most songs of in the genres picked (for "artists you like"). */
  suggestions = $state.raw<string[]>([]);
  progress = $state.raw<RideProgress | null>(null);
  buildError = $state<string | null>(null);
  /** The plan the last build answered (until the stage event carries it). */
  built = $state.raw<RideBuilt | null>(null);
  /** The health notice is open; `after` is what waits for "I understand". */
  noticeOpen = $state(false);
  noticeText = $state("");
  tvMode = $state<TvMode>("none");
  monitors = $state.raw<MonitorView[]>([]);
  /** A polite live region: part changes, the next part. */
  live = $state("");
  // the AI conversation
  chatOpen = $state(false);
  chat = $state<ChatMessage[]>([]);
  chatQuick = $state<string[]>([]);
  chatBusy = $state(false);
  chatError = $state<string | null>(null);
  /** The saved rides, favorites first then the most recent (the backend's order). */
  rides = $state.raw<SavedRide[]>([]);
  setupTab = $state<SetupTab>("new");

  private started = false;
  private after: (() => void) | null = null;
  private cancelled = false;
  private cues = new CuePlayer();
  private lastSegment = -1;
  private announcedNotice = -1;

  settings = $derived<ExerciseSettings>(exerciseSettingsOf(app.settings));
  /** The stage as the backend holds it. */
  stage = $derived(this.state?.stage ?? "idle");
  /** The screen to draw. */
  plan = $derived<RidePlan | null>(this.state?.plan ?? this.keptPlan ?? this.built?.plan ?? null);
  step = $derived<ExerciseStep>(
    this.stage === "riding"
      ? "ride"
      : this.stage === "finished"
        ? "done"
        : this.rebuilding && this.plan
          ? "preview"
          : this.building || this.stage === "building" || !!this.buildError
            ? "building"
            : this.stage === "ready" && !this.editing
              ? "preview"
              : "setup",
  );
  riding = $derived(this.stage === "riding");
  /** The rail's dot: a ride or a build is going on. */
  busy = $derived(this.stage === "riding" || this.step === "building");
  tv = $derived(this.state?.tv ?? { server: false, url: null, window: false, hub: false, audio: "phone" as const });
  /** The ride screen is dark in any theme (a setting). */
  dark = $derived(this.settings.ride_always_dark);
  aiOn = $derived(assistant.ready);
  aiName = $derived(assistant.provider?.name ?? "");
  /** The saved ride the plan on the screen belongs to (every build is saved by itself). */
  currentSaved = $derived<SavedRide | null>(
    (this.state?.saved_id ? this.rides.find((r) => r.id === this.state?.saved_id) : null) ?? this.built?.saved ?? null,
  );
  /** The 3-2-1 digit over the ring, 0 when none is due. */
  countdown = $derived.by(() => {
    const v = this.view;
    const secs = this.plan?.cues.countdown_seconds ?? this.settings.countdown_seconds;
    if (!v || v.finished || !secs) return 0;
    const left = v.interval ? v.interval.phase_remaining_s : v.remaining_s;
    return left >= 1 && left <= secs ? left : 0;
  });

  async init() {
    if (this.started) return;
    this.started = true;
    onExercise.state((s) => this.apply(s));
    onExercise.ride((t) => this.onTick(t));
    onExercise.rides(() => this.refreshRides());
    assistant.loadStatus();
    try {
      this.apply(await exerciseApi.state());
      this.presets = await exerciseApi.presets(EX_LANGUAGE);
      this.request = await exerciseApi.defaultRequest();
    } catch {}
    this.refreshRides();
    if (import.meta.env.DEV && new URLSearchParams(location.search).has("exrides")) this.setupTab = "rides";
    if (import.meta.env.DEV && new URLSearchParams(location.search).has("exrebuild") && this.plan) this.rebuild();
    this.tvMode = this.settings.tv_enabled ? "app" : "none";
  }

  private apply(s: ExerciseState) {
    const before = this.state?.stage;
    this.state = s;
    if (s.view && (!this.view || s.view.at_ms >= this.view.at_ms)) this.view = s.view;
    this.playing = s.playing;
    if (s.stage !== "ready") this.editing = false;
    if (s.stage === "riding" && before !== "riding") this.rideBegan();
    if (s.stage === "idle" || s.stage === "ready") {
      this.view = null;
      this.cues.reset();
    }
  }

  private rideBegan() {
    this.cues.reset();
    this.lastSegment = -1;
    this.announcedNotice = -1;
  }

  private onTick(t: { stage: ExerciseState["stage"]; playing: boolean; view: RideView }) {
    if (t.stage !== "riding" && t.stage !== "finished") return;
    this.view = t.view;
    this.playing = t.playing;
    if (t.stage === "riding") {
      const s = this.settings;
      this.cues.onView(t.view, t.playing, { audible: s.audible_cues, volume: s.cue_volume, click: s.metronome_click });
    }
    this.announce(t.view);
  }

  /** Part changes and the next part, for a screen reader (the 3-2-1 is not read: the cues sound). */
  private announce(v: RideView) {
    if (v.finished) return;
    if (v.segment_index !== this.lastSegment) {
      this.lastSegment = v.segment_index;
      this.live = `${zoneName(v.kind)}, ${v.target_rpm} RPM, ${positionLabel(v.position).toLowerCase()}`;
    } else if (v.notice && v.next && this.announcedNotice !== v.segment_index) {
      this.announcedNotice = v.segment_index;
      this.live = `Next: ${zoneName(v.next.kind)}, ${v.next.target_rpm} RPM, ${positionLabel(v.next.position).toLowerCase()}`;
    }
  }

  async saveSettings(patch: Partial<ExerciseSettings>) {
    try {
      await app.saveSettings({ exercise: { ...this.settings, ...patch } });
    } catch (e) {
      app.note(errorText(e), "error", "exercise-settings");
    }
  }

  // ------------------------------------------------------------ the request

  setMinutes(minutes: number) {
    this.request.minutes = Math.min(EX_MINUTES.max, Math.max(EX_MINUTES.min, Math.round(minutes / EX_MINUTES.step) * EX_MINUTES.step));
  }
  /** Genres picked (an empty list is "Any"). */
  toggleGenre(genre: string) {
    const has = this.request.wish.genres.some((g) => g.toLowerCase() === genre.toLowerCase());
    if (has) this.request.wish.genres = this.request.wish.genres.filter((g) => g.toLowerCase() !== genre.toLowerCase());
    else if (this.request.wish.genres.length < EX_MAX_GENRES) this.request.wish.genres = [...this.request.wish.genres, genre.slice(0, EX_MAX_GENRE_CHARS)];
  }
  clearGenres() {
    this.request.wish.genres = [];
  }
  setSentence(text: string) {
    this.request.wish.sentence = text.slice(0, EX_MAX_SENTENCE_CHARS);
  }
  // ------------------------------------------------------------ the taste (saved settings)

  /** The taste answers are settings: every change is saved, the next ride starts from them. */
  setTaste(patch: Partial<RideTaste>) {
    return this.saveSettings({ taste: { ...this.settings.taste, ...patch } });
  }
  /** A decade on or off (none is any era). */
  toggleEra(decade: number) {
    const eras = this.settings.taste.eras;
    return this.setTaste({ eras: eras.includes(decade) ? eras.filter((d) => d !== decade) : [...eras, decade].sort((a, b) => a - b) });
  }
  /** Adds a name to the liked or avoided artists (once, any case; the core keeps ten). */
  addArtist(list: "artists" | "avoid", name: string) {
    const clean = name.trim().slice(0, EX_MAX_ARTIST_CHARS);
    const names = this.settings.taste[list];
    if (!clean || names.some((n) => n.toLowerCase() === clean.toLowerCase()) || names.length >= EX_MAX_TASTE_ARTISTS) return;
    return this.setTaste({ [list]: [...names, clean] });
  }
  removeArtist(list: "artists" | "avoid", name: string) {
    return this.setTaste({ [list]: this.settings.taste[list].filter((n) => n !== name) });
  }
  /** Re-reads the suggestions for the genres picked now. */
  async loadSuggestions() {
    try {
      this.suggestions = await exerciseApi.artistSuggestions($state.snapshot(this.request.wish.genres));
    } catch {
      this.suggestions = [];
    }
  }

  get tooManyGenres() {
    return this.request.wish.genres.length >= EX_MAX_GENRES;
  }
  /** Beginners are offered the gentle style until they have seen the notice. */
  get firstRideRecommended() {
    return !this.settings.health_notice_seen;
  }
  /** What it will be called when built: "30 min · Easy · Rock + Metal". */
  get summary() {
    const style = this.presets.find((p) => p.id === this.request.style)?.name ?? PRESET_IDS[0];
    const genres = this.request.wish.genres.length ? this.request.wish.genres.join(" + ") : "Any music";
    return `${this.request.minutes} min · ${style} · ${genres}`;
  }

  // ------------------------------------------------------------ the notice

  /** Runs `then` now, or after the health notice has been accepted (once, before the first ride). */
  private async gated(then: () => void) {
    if (this.settings.health_notice_seen) return then();
    this.after = then;
    if (!this.noticeText) {
      try {
        this.noticeText = await exerciseApi.healthNotice(EX_LANGUAGE);
      } catch {}
    }
    this.noticeOpen = true;
  }
  async acceptNotice() {
    const then = this.after;
    this.after = null;
    this.noticeOpen = false;
    await this.saveSettings({ health_notice_seen: true });
    then?.();
  }
  dismissNotice() {
    this.after = null;
    this.noticeOpen = false;
  }
  /** Settings → "Show it again". */
  async showNoticeAgain() {
    await this.saveSettings({ health_notice_seen: false });
  }

  // ------------------------------------------------------------ build

  build() {
    return this.gated(() => this.runBuild(false));
  }

  /** `again`: "new songs" for the plan on the screen, which stays until the new one is there (a failure keeps it). */
  private async runBuild(again: boolean) {
    if (this.building) return;
    this.building = true;
    this.keptPlan = again ? this.plan : null;
    this.rebuilding = !!this.keptPlan;
    this.cancelled = false;
    this.progress = null;
    this.buildError = null;
    this.rebuildError = null;
    this.editing = false;
    try {
      this.built = await exerciseApi.build($state.snapshot(this.request) as RideRequest, EX_LANGUAGE, (p) => (this.progress = p));
    } catch (e) {
      if (!this.cancelled) {
        if (this.rebuilding) this.rebuildError = errorText(e);
        else this.buildError = errorText(e);
      }
    } finally {
      this.building = false;
      this.rebuilding = false;
      this.keptPlan = null;
      this.progress = null;
    }
  }

  async cancelBuild() {
    this.cancelled = true;
    this.buildError = null;
    this.rebuildError = null;
    try {
      await exerciseApi.cancel();
    } catch {}
  }

  /** Back to the setup with everything kept. */
  changeSetup() {
    this.editing = true;
    this.buildError = null;
    this.rebuildError = null;
  }

  // ------------------------------------------------------------ the ride

  start() {
    const plan = this.plan;
    if (!plan) return Promise.resolve();
    return this.gated(() => this.runStart(plan));
  }

  private async runStart(plan: RidePlan) {
    this.cues.unlock();
    try {
      this.apply(await exerciseApi.start(plan));
      if (this.tvMode === "window") await exerciseApi.tvWindow(true, this.settings.tv_window_display === "auto" ? null : this.settings.tv_window_display);
    } catch (e) {
      app.note(errorText(e), "error", "exercise");
    }
  }

  async stop() {
    this.cues.reset();
    try {
      this.apply(await exerciseApi.stop());
    } catch (e) {
      app.note(errorText(e), "error", "exercise");
    }
  }

  /** The ride is the normal player's queue: pause and next-song are the player's. */
  pause = () => api.toggle().catch((e) => app.note(errorText(e), "error", "exercise"));
  skip = () => api.next().catch((e) => app.note(errorText(e), "error", "exercise"));

  /** "Ride again": the same plan, the same songs. */
  async rideAgain() {
    await this.stop();
    if (this.plan) await this.start();
  }
  /** "Same ride, new songs": the same request built again, on the preview (searched again, with other songs). */
  async rebuild() {
    if (this.stage === "riding" || this.stage === "finished") await this.stop();
    await this.gated(() => this.runBuild(true));
  }
  /** "Done" / leaving the summary. */
  finish() {
    return this.stop();
  }

  // ------------------------------------------------------------ saved rides

  async refreshRides() {
    try {
      this.rides = await exerciseApi.rides();
    } catch {}
  }

  /** Runs one call on the saved rides, then re-reads the list; the error is said, not thrown. */
  private async onRides<T>(job: () => Promise<T>): Promise<T | null> {
    try {
      const out = await job();
      await this.refreshRides();
      return out;
    } catch (e) {
      app.note(errorText(e), "error", "exercise-rides");
      return null;
    }
  }

  /** "Ride again": the saved plan comes back as the preview (no AI, no build); Start ride rides it. */
  async loadRide(id: string) {
    const loaded = await this.onRides(() => exerciseApi.rideLoad(id));
    if (!loaded) return;
    this.request = loaded.request;
    this.built = { plan: loaded.plan, playlist: loaded.playlist, saved: loaded.saved };
    this.buildError = null;
    this.editing = false;
    try {
      this.apply(await exerciseApi.state());
    } catch {}
  }
  /** "Keep it" (the automatic save becomes the rider's: pruning leaves it), with a name if given. */
  keepRide = (id: string, name?: string) => this.onRides(() => exerciseApi.rideKeep(id, name));
  renameRide = (id: string, name: string) => this.onRides(() => exerciseApi.rideRename(id, name));
  favoriteRide = (id: string, favorite: boolean) => this.onRides(() => exerciseApi.rideFavorite(id, favorite));
  async deleteRide(id: string) {
    if (await this.onRides(() => exerciseApi.rideDelete(id))) app.note("Ride deleted", "success", "exercise-rides");
  }
  async exportRide(ride: SavedRide) {
    try {
      const path = await pickSavePath(RIDE_FILE, ride.name);
      if (!path) return;
      app.note(`Exported to ${await exerciseApi.rideExport(ride.id, path)}`, "success", "exercise-rides");
    } catch (e) {
      app.note(errorText(e), "error", "exercise-rides");
    }
  }
  async importRide() {
    try {
      const path = await pickOpenPath(RIDE_FILE);
      if (!path) return;
      const ride = await this.onRides(() => exerciseApi.rideImport(path));
      if (ride) app.note(`Imported “${ride.name}”`, "success", "exercise-rides");
    } catch (e) {
      app.note(errorText(e), "error", "exercise-rides");
    }
  }

  // ------------------------------------------------------------ the TV

  async setTvMode(mode: TvMode) {
    this.tvMode = mode;
    await this.saveSettings({ tv_enabled: mode !== "none" });
    try {
      if (mode === "app") await exerciseApi.tv(true);
      else if (mode === "none") await exerciseApi.tv(false);
      if (mode !== "window" && this.tv.window) await exerciseApi.tvWindow(false, null);
      if (mode === "window") await this.refreshMonitors();
    } catch (e) {
      app.note(errorText(e), "error", "exercise-tv");
    }
  }
  async refreshMonitors() {
    try {
      this.monitors = await exerciseApi.monitors();
    } catch {
      this.monitors = [];
    }
  }
  /** The ride screen's TV button: the window on / off. */
  async toggleTvWindow() {
    try {
      await exerciseApi.tvWindow(!this.tv.window, this.settings.tv_window_display === "auto" ? null : this.settings.tv_window_display);
    } catch (e) {
      app.note(errorText(e), "error", "exercise-tv");
    }
  }
  async openTvServer() {
    try {
      await exerciseApi.tv(true);
    } catch (e) {
      app.note(errorText(e), "error", "exercise-tv");
    }
  }
  async playHere() {
    await this.saveSettings({ tv_audio: "phone" });
  }

  // ------------------------------------------------------------ the AI conversation

  /** Opens the conversation (the AI's fixed opener, no AI call yet). */
  async openChat() {
    this.chatOpen = true;
    if (this.chat.length) return;
    this.chatError = null;
    this.chatBusy = true;
    try {
      this.takeTurn(await exerciseApi.open($state.snapshot(this.request) as RideRequest, EX_LANGUAGE));
    } catch (e) {
      this.chatError = errorText(e);
    } finally {
      this.chatBusy = false;
    }
  }
  async say(text: string) {
    const said = text.trim();
    if (!said || this.chatBusy) return;
    this.chat = [...this.chat, { who: "me" as const, text: said }].slice(-MAX_CHAT_MESSAGES);
    this.chatBusy = true;
    this.chatError = null;
    try {
      this.takeTurn(await exerciseApi.reply(said));
    } catch (e) {
      this.chatError = errorText(e);
    } finally {
      this.chatBusy = false;
    }
  }
  private takeTurn(turn: ExerciseTurn) {
    this.chat = [...this.chat, { who: "ai" as const, text: turn.text }].slice(-MAX_CHAT_MESSAGES);
    this.chatQuick = turn.quick_replies;
    this.request = turn.request;
  }
  async closeChat() {
    this.chatOpen = false;
    this.chat = [];
    this.chatQuick = [];
    this.chatError = null;
    try {
      await exerciseApi.close();
    } catch {}
  }
}

export const exercise = new ExerciseStore();
