// No console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod media;
mod settings;
mod share;
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

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        // cover://localhost/<url-encoded path>: embedded (or folder) cover art of a local file
        .register_asynchronous_uri_scheme_protocol("cover", |_ctx, request, responder| {
            let path = percent_decode(request.uri().path().trim_start_matches('/'));
            std::thread::spawn(move || {
                let response = match homefi_core::library::read_cover(std::path::Path::new(&path)) {
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

            let handle = app.handle().clone();
            let listener = Arc::new(move |snapshot: &homefi_player::Snapshot| {
                let Some(state) = handle.try_state::<AppState>() else { return };
                let track = snapshot.current.and_then(|i| snapshot.queue.get(i)).and_then(|p| state.track(p));
                media::update(&handle, snapshot, track.clone());
                share::on_player(&handle, snapshot);
                let _ = handle.emit("player", PlayerView { snapshot: snapshot.clone(), track });
            });
            let player = Player::new(settings.volume, settings.output_device.clone(), listener);

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
            });

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
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::account,
            commands::start_login,
            commands::poll_login,
            commands::logout,
            commands::search,
            commands::tidal_library_page,
            commands::tidal_detail,
            commands::tidal_detail_page,
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
        ])
        .build(tauri::generate_context!())
        .expect("error while running HomeFi")
        .run(|app, event| {
            // closing: the hub says goodbye to its guests, a guest deletes the party's songs
            if let tauri::RunEvent::Exit = event {
                share::shutdown(app);
            }
        });
}

fn sniff_image(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else {
        "image/jpeg"
    }
}
