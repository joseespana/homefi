// Development-only backend for discs (mock.ts spreads `handlers`): the same shapes as the Rust
// commands (src-tauri/src/disc.rs). Flags, all of which open the Disc screen:
//   ?disc            an audio CD with no names on it (the form shows what blanks become)
//   ?disc=cdtext     an audio CD with CD-Text (the form is filled)
//   ?disc=protected  a copy-protected DVD       ?disc=data   a data disc with files
//   ?disc=nodrive    no drive at all            ?disc=empty  a drive with no disc
//   ?ripping         ripping, stalled at track 4 (?ripping=done runs to the end)
//   ?discinsert      the "Audio CD inserted" notice arrives 1.5 s after loading
//   ?mb              turns "Look up discs on MusicBrainz" on
import type { DiscMatch, DiscSettings, DiscView, DriveView, RipProgress, RipRequest, RipSummary } from "./api";
import { DEFAULT_DISC_SETTINGS } from "./api";

const url = new URLSearchParams(location.search);
const flag = url.get("disc");
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
const SECTORS_PER_SECOND = 75;
const STEP_MS = 140;
const INSERT_DELAY_MS = 2500;

// dev: the flags open their screen (App.svelte reads `#name` once the app is ready)
if (!location.hash && (url.has("disc") || url.has("ripping") || url.has("discinsert") || url.has("mb"))) location.hash = "#disc";

const TITLES = ["Blind", "Ball Tongue", "Need To", "Clown", "Divine", "Faget", "Shoots and Ladders", "Predictable", "Fake", "Lies", "Helmet in the Bush", "Daddy"];
const LENGTHS_S = [259, 263, 241, 277, 170, 223, 138, 252, 283, 201, 229, 456];
const ARTIST = "Korn";
const ALBUM = "Korn";

const starts = LENGTHS_S.reduce<number[]>((acc, s, i) => [...acc, (acc[i] ?? 150) + s * SECTORS_PER_SECOND], [150]);
const tocTracks = LENGTHS_S.map((_, i) => ({ number: i + 1, startLba: starts[i], endLba: starts[i + 1], preEmphasis: false }));
const totalMs = LENGTHS_S.reduce((a, s) => a + s * 1000, 0);

const DRIVE_NAME = "HL-DT-ST BD-RE WH16NS40";
const audio: DriveView = { id: "disk4", name: DRIVE_NAME, media: "audio", tracks: LENGTHS_S.length, lengthMs: totalMs };
const drive = (): DriveView | null => {
  switch (flag) {
    case "nodrive":
      return null;
    case "empty":
      return { id: "disk4", name: DRIVE_NAME, media: "none" };
    case "protected":
      return { id: "disk4", name: DRIVE_NAME, media: "protected", protection: "css" };
    case "data":
      return { id: "disk4", name: DRIVE_NAME, media: "data", mount: "/Volumes/PHOTOS_2019" };
    default:
      return audio;
  }
};

const cdText: DiscMatch = {
  source: "cdText",
  confidence: 1,
  album: { title: ALBUM, artist: ARTIST, year: null, genre: null, upc: null, discNumber: 1, discTotal: 1, tidalAlbumId: null, mbid: null },
  tracks: TITLES.map((title, i) => ({ number: i + 1, title, artists: [ARTIST], isrc: null, tidalTrackId: null })),
  cover: null,
  note: null,
};
const musicBrainz: DiscMatch = { ...cdText, source: "musicbrainz", confidence: 0.94, album: { ...cdText.album, year: 1994, genre: "Nu metal", mbid: "5a3c1b7e-0000-4000-8000-000000000000" } };

const view = (d: DriveView): DiscView => ({
  drive: d,
  toc: { tracks: tocTracks, leadoutLba: starts[starts.length - 1] },
  ids: { musicbrainz: "Vt0EOo0VYSDm5OPoWBGr2agVLQE-", freedb: "8b0c3c0c", ctdb: "H7ZsqBTaSzq9W8o1cUNq1m4k7wk-" },
  mcn: "0602498234567",
  cdText: flag === "cdtext" ? cdText : null,
});

let settings: DiscSettings = { ...DEFAULT_DISC_SETTINGS, sources: { ...DEFAULT_DISC_SETTINGS.sources, musicbrainz: url.has("mb") || flag === "cdtext" ? true : false } };

// ---- a rip: a real timeline over a channel, like the Rust side
let aborted: (() => void) | null = null;

async function rip(request: RipRequest, channel: { id: number }): Promise<RipSummary> {
  let index = 0;
  const send = (message: RipProgress) => (window as unknown as Record<string, ((v: unknown) => void) | undefined>)[`_${channel.id}`]?.({ index: index++, message });
  let cancelled = false;
  aborted = () => (cancelled = true);
  const stallAt = url.get("ripping") === "done" ? Infinity : 4;
  let ok = 0;
  for (const n of request.tracks) {
    if (cancelled) throw "cancelled";
    const total = tocTracks[n - 1].endLba - tocTracks[n - 1].startLba;
    const title = request.meta.tracks[n - 1]?.title ?? `Track ${n}`;
    send({ type: "started", track: n, title });
    const steps = n === stallAt ? 9 : 5;
    for (let i = 1; i <= steps; i++) {
      await wait(STEP_MS);
      if (cancelled) throw "cancelled";
      send({ type: "reading", track: n, doneSectors: Math.floor((total * i) / (n === stallAt ? 20 : steps)), totalSectors: total, rereads: n === 2 ? 3 * i : 0, suspiciousSamples: n === 6 && i > 3 ? 12 : 0 });
    }
    if (n === stallAt) {
      await new Promise<void>((resolve) => (aborted = () => ((cancelled = true), resolve())));
      throw "cancelled";
    }
    send({ type: "encoding", track: n, percent: 100 });
    await wait(STEP_MS);
    send({ type: "verifying", track: n });
    await wait(STEP_MS);
    const bad = n === 6;
    send({ type: "trackDone", track: n, path: `~/Music/homefi/${request.meta.album.artist}/${request.meta.album.title}/${n}.flac`, read: { samplesTotal: total * 588, rereads: n === 2 ? 15 : 0, suspiciousSamples: bad ? 12 : 0, unreadableSamples: 0, c2Errors: 0 }, verified: null });
    ok++;
  }
  send({ type: "finished", ok, failed: 0, logPath: null });
  return { ok, failed: 0, logPath: `~/Music/homefi/${request.meta.album.artist}/${request.meta.album.title}/${request.meta.album.title}.homefi-rip.log`, albumDir: `~/Music/homefi/${request.meta.album.artist}/${request.meta.album.title}` };
}

export const discMock = {
  settings: () => ({ disc: settings }),
  setSettings: (s: { disc?: DiscSettings }) => {
    if (s.disc) settings = s.disc;
    return { disc: settings };
  },
  /** ?discinsert: the notice a real insert makes. */
  start: (emit: (event: string, payload: unknown) => void) => {
    if (url.has("discinsert")) setTimeout(() => emit("disc", { type: "inserted", drive: audio }), INSERT_DELAY_MS);
    if (!flag || flag === "cdtext") setTimeout(() => emit("disc", { type: "isrcs", drive: audio, isrcs: TITLES.map((_, i) => `USRC1940${String(i + 10)}`) }), INSERT_DELAY_MS);
  },
  handlers: {
    disc_drives: (): DriveView[] => {
      const d = drive();
      return d ? [d] : [];
    },
    disc_open: async (): Promise<DiscView> => {
      await wait(250);
      return view(drive() ?? audio);
    },
    disc_identify: async (): Promise<DiscMatch[]> => {
      await wait(900);
      return [musicBrainz];
    },
    disc_rip: ({ request, onProgress }: { request: RipRequest; onProgress: { id: number } }) => rip(request, onProgress),
    disc_cancel: () => aborted?.(),
    disc_eject: () => undefined,
  },
};
