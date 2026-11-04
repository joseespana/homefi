// What the player's side buttons do, shared by the top bar and the player bar
// (Settings → Listening room → Player buttons decides where they sit).
import { app } from "./state.svelte";
import { lyrics } from "./lyrics.svelte";
import { share } from "./share.svelte";

/** Lyrics, queue and mini player sit in the bar above the content (the default). */
export const buttonsOnTop = () => (app.settings?.room.player_buttons ?? "top") === "top";

/** The lyrics are on screen (they open over the listening room). */
export const lyricsShown = () => lyrics.open && (app.view.name === "room" || app.view.name === "guest");

export const toggleShare = () => (share.inRoom ? share.open("room") : (share.panelOpen = !share.panelOpen));

export const toggleLyrics = () => (lyricsShown() ? lyrics.setOpen(false) : lyrics.show());

export const shareTitle = () => (share.sharing ? "Sharing this music" : share.inRoom ? `In ${share.guest.session_name}` : "Share this music");
