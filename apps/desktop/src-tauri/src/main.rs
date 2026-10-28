// No console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod commands;
mod eq;
mod media;
mod settings;
mod share;
mod sound;
mod stream;

use std::sync::{Arc, Mutex};

use homefi_core::config::Config;
use homefi_core::paths;
use homefi_core::session::Session;
use homefi_player::Player;
use tauri::http::{Response, StatusCode};
use tauri::{Emitter, Manager};

use crate::commands::{AppState, PlayerView};
use crate::settings::DesktopSettings;

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
            if let Ok(b) = u8::from_str_radix(hex, 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_env("HOMEFI_LOG").unwrap_or_else(|_| "warn".into()))
        .init();

    let handler = commands_of(tauri::generate_handler![
        commands::app_info,
        commands::account,
        commands::start_login,
        commands::poll_login,
        commands::logout,
        commands::search,
        commands::tidal_library_page,
        commands::tidal_detail,
        commands::tidal_detail_page,
        commands::tidal_album_versions,
        commands::tidal_library_stream,
        commands::tidal_detail_stream,
        commands::tidal_cancel,
        commands::tidal_favorite_ids,
        commands::lyrics,
        commands::stream_upcoming,
        commands::tidal_tracks_by_isrc,
        commands::tidal_favorite,
        commands::tidal_my_playlists,
        commands::tidal_create_playlist,
        commands::tidal_playlist_add,
        commands::tidal_playlist_remove,
        commands::tidal_playlist_delete,
        commands::fix_artwork,
        commands::scan_folder,
        commands::download,
        commands::cancel_downloads,
        commands::library,
        commands::local_playlists,
        commands::queue_add,
        commands::queue_tidal,
        commands::rescan_library,
        commands::prune_library,
        commands::delete_tracks,
        commands::player_state,
        commands::play_tracks,
        commands::play_tidal,
        commands::tracks_for,
        commands::player_move,
        commands::mini_player,
        commands::mini_ready,
        commands::player_toggle,
        commands::player_next,
        commands::player_previous,
        commands::player_seek,
        commands::player_jump,
        commands::player_shuffle,
        commands::player_repeat,
        commands::player_volume,
        commands::visual,
        commands::output_devices,
        commands::set_output,
        commands::get_settings,
        commands::set_settings,
        eq::eq_get,
        eq::eq_set,
        eq::eq_bypass,
        eq::eq_response,
        eq::eq_log_freqs,
        eq::eq_auto_preamp,
        eq::eq_with_taste,
        eq::autoeq_search,
        eq::autoeq_profile,
        sound::taste_get,
        sound::taste_set,
        sound::loudness_get,
        sound::loudness_set,
        sound::crossfade_get,
        sound::crossfade_set,
        sound::output_options_get,
        sound::output_options_set,
        sound::sound_now,
        sound::sound_react,
        sound::sound_undo,
        sound::taste_suggest,
        sound::listening_summary,
        ai::ai_providers,
        ai::ai_key_set,
        ai::ai_key_clear,
        ai::ai_key_status,
        ai::ai_list_models,
        ai::ai_test_key,
        ai::ai_default_model,
        ai::eq_assistant_start,
        ai::eq_assistant_reply,
        ai::eq_assistant_close,
        share::share_status,
        share::share_start,
        share::share_stop,
        share::share_set_party,
        share::share_set_play_here,
        share::share_kick,
        share::share_approve,
        share::share_reject,
        share::share_join,
        share::share_leave,
        share::share_guest,
        share::share_play_along,
        share::share_probe,
        share::share_parse_link,
    ]);
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        // cover://localhost/<url-encoded path>: embedded (or folder) cover art of a local file
        .register_asynchronous_uri_scheme_protocol("cover", |_ctx, request, responder| {
            let path = percent_decode(request.uri().path().trim_start_matches('/'));
            // a grid of hundreds of covers: a few files read at a time, on the
            // blocking pool (not one new thread per image)
            tauri::async_runtime::spawn(async move {
                let _permit = COVER_READS.acquire().await;
                let read = tauri::async_runtime::spawn_blocking(move || {
                    homefi_core::library::read_cover(std::path::Path::new(&path))
                });
                let response = match read.await.ok().flatten() {
                    Some(bytes) => Response::builder()
                        .header("Content-Type", sniff_image(&bytes))
                        .header("Cache-Control", "max-age=3600")
                        // lets the UI read pixels (album colors) from a canvas
                        .header("Access-Control-Allow-Origin", "*")
                        .body(bytes),
                    None => Response::builder().status(StatusCode::NOT_FOUND).body(Vec::new()),
                };
                responder.respond(response.expect("valid response"));
            });
        })
        .setup(|app| {
            let app_dir = paths::app_dir();
            std::fs::create_dir_all(&app_dir)?;
            let config = Config::load(&app_dir.join(paths::CONFIG_FILENAME)).unwrap_or_default();
            let session = Session::new(app_dir.clone(), config);
            let settings_path = app_dir.join("desktop.json");
            let settings = DesktopSettings::load(&settings_path);

            // The engine thread only hands its snapshot over (it must never wait
            // on a lock or the disk, or the audio stutters); a task looks up the
            // track, tells the OS and the UI. A slow moment skips stale snapshots.
            let (snapshots_tx, mut snapshots) = tokio::sync::watch::channel::<Option<homefi_player::Snapshot>>(None);
            let listener = Arc::new(move |snapshot: &homefi_player::Snapshot| {
                snapshots_tx.send_replace(Some(snapshot.clone()));
            });
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while snapshots.changed().await.is_ok() {
                    let Some(snapshot) = snapshots.borrow_and_update().clone() else { continue };
                    let app = handle.clone();
                    // tags of a file outside the library are read from disk the first time
                    let _ = tauri::async_runtime::spawn_blocking(move || {
                        let Some(state) = app.try_state::<AppState>() else { return };
                        let track = snapshot.current.and_then(|i| snapshot.queue.get(i)).and_then(|p| state.track(p));
                        media::update(&app, &snapshot, track.clone());
                        share::on_player(&app, &snapshot);
                        let _ = app.emit("player", PlayerView { snapshot, track });
                    })
                    .await;
                }
            });
            let player = Player::new(settings.volume, settings.output_device.clone(), listener);
            player.set_eq(settings.eq.clone());
            app.manage(homefi_core::dsp::autoeq::AutoEq::new(app_dir.join(eq::AUTOEQ_CACHE)));
            app.manage(ai::Ai::new(&app_dir));

            let (jobs_tx, jobs_rx) = tokio::sync::mpsc::unbounded_channel();
            app.manage(AppState {
                session,
                player: player.clone(),
                library: Mutex::new(Vec::new()),
                settings: Mutex::new(settings),
                settings_path,
                library_cache: app_dir.join("desktop_library_cache.json"),
                jobs: jobs_tx,
                cancel: Mutex::new(None),
                stream_generation: Default::default(),
                upcoming: Default::default(),
                outside: Default::default(),
                api_cache: Default::default(),
                loads: Default::default(),
                settings_dirty: Default::default(),
            });
            tauri::async_runtime::spawn(commands::settings_writer(app.handle().clone()));
            // the first check runs ffmpeg once: not on the UI's first call
            std::thread::spawn(homefi_core::ffmpeg::is_available);

            app.manage(sound::Sound::default());
            sound::init(app.handle());
            app.manage(share::Share::default());
            media::init(app.handle(), player);
            share::init(app.handle());

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(commands::download_worker(handle.clone(), jobs_rx));
            tauri::async_runtime::spawn(async move {
                if let Err(e) = commands::rescan(&handle).await {
                    tracing::warn!("library scan failed: {e}");
                }
            });
            Ok(())
        })
        .invoke_handler(move |invoke| timed(invoke, &handler))
        .build(tauri::generate_context!())
        .expect("error while running HomeFi")
        .run(|app, event| {
            // closing: the hub says goodbye to its guests, a guest deletes the party's songs
            if let tauri::RunEvent::Exit = event {
                share::shutdown(app);
                app.state::<AppState>().flush_settings();
            }
        });
}

/// Names the runtime of the generated command handler (so it can be wrapped).
fn commands_of<F: Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync + 'static>(f: F) -> F {
    f
}

/// Covers read from disk at the same time.
static COVER_READS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(6);

/// Commands slower than this on the thread that received them are logged
/// (debug builds, or `HOMEFI_TIMING=1`). A plain `fn` command runs on the main
/// thread, where it freezes the window for that long.
const SLOW_COMMAND: std::time::Duration = std::time::Duration::from_millis(50);

fn timed<R: tauri::Runtime>(invoke: tauri::ipc::Invoke<R>, handler: &dyn Fn(tauri::ipc::Invoke<R>) -> bool) -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if !*ON.get_or_init(|| cfg!(debug_assertions) || std::env::var_os("HOMEFI_TIMING").is_some()) {
        return handler(invoke);
    }
    let command = invoke.message.command().to_owned();
    let start = std::time::Instant::now();
    let handled = handler(invoke);
    let took = start.elapsed();
    if took > SLOW_COMMAND {
        let thread = std::thread::current();
        tracing::warn!("command `{command}` held the {} thread for {took:?}", thread.name().unwrap_or("unnamed"));
    }
    handled
}

fn sniff_image(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else {
        "image/jpeg"
    }
}
