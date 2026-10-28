import { mount } from "svelte";
import "./app.css";
import "./skins.css";
import App from "./App.svelte";
import MiniPlayer from "./components/MiniPlayer.svelte";

// outside Tauri (plain browser during development) use a fake backend
if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
  await import("./lib/mock");
}

// macOS draws its window buttons over the top-left corner (overlay title bar)
if (navigator.userAgent.includes("Mac")) document.documentElement.dataset.os = "mac";

// the floating mini player window loads index.html?mini
const mini = new URLSearchParams(location.search).has("mini");
mount(mini ? MiniPlayer : App, { target: document.getElementById("app")! });
