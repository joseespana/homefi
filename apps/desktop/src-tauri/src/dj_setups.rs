//! Named DJ setups on the desktop: the whole of the DJ settings (options, keymap, layout and pad
//! sounds) saved under a name, applied later and exported to a `.homefidj` file to share or move to
//! another machine, phone or tablet (a Google Drive folder is just a folder). The rules and the
//! files are `homefi_core::dj::setups`; this wires them to the settings and the dialog's paths.
//!
//! Events: `dj-setups` (no payload) when the list changes; `settings` (as always) when one is applied.

use std::path::Path;

use homefi_core::dj::{DjSettings, DjSetup, DjSetupStore};
use homefi_core::export::ExportKind;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::commands::{err, AppState};
use crate::dj::Dj;

type CmdResult<T> = Result<T, String>;

/// The setups on disk (`~/.homefi/dj/setups`), managed by Tauri.
pub struct DjSetups {
    store: DjSetupStore,
}

impl DjSetups {
    pub fn new(app_dir: &Path) -> Self {
        Self { store: DjSetupStore::in_app_dir(app_dir) }
    }
}

/// A setup as a list shows it.
#[derive(Debug, Clone, Serialize)]
pub struct DjSetupSummary {
    pub id: String,
    pub name: String,
    pub created_ms: u64,
    pub updated_ms: u64,
}

impl From<&DjSetup> for DjSetupSummary {
    fn from(s: &DjSetup) -> Self {
        Self { id: s.id.clone(), name: s.name.clone(), created_ms: s.created_ms, updated_ms: s.updated_ms }
    }
}

/// What `dj_setup_apply` returns.
#[derive(Debug, Clone, Serialize)]
pub struct DjSetupApplied {
    /// The DJ settings now in force (also in the `settings` event).
    pub settings: DjSettings,
    /// DJ mode is on and the setup changed the pad sounds: they are rendered when the engine starts,
    /// so they play from the next time DJ mode is turned on.
    pub restart_needed: bool,
}

fn emit_setups(app: &AppHandle) {
    let _ = app.emit("dj-setups", ());
}

async fn with_store<T: Send + 'static>(
    app: &AppHandle,
    job: impl FnOnce(DjSetupStore) -> homefi_core::Result<T> + Send + 'static,
) -> CmdResult<T> {
    let store = app.state::<DjSetups>().store.clone();
    tauri::async_runtime::spawn_blocking(move || job(store)).await.map_err(err)?.map_err(err)
}

/// The setups by name.
#[tauri::command]
pub async fn dj_setups(app: AppHandle) -> Vec<DjSetupSummary> {
    let store = app.state::<DjSetups>().store.clone();
    tauri::async_runtime::spawn_blocking(move || store.list().iter().map(Into::into).collect())
        .await
        .unwrap_or_default()
}

/// Saves the DJ settings now in force (options, keymap, layout, pads) as a new setup.
#[tauri::command]
pub async fn dj_setup_save(app: AppHandle, name: String) -> CmdResult<DjSetupSummary> {
    let current = app.state::<AppState>().settings().dj;
    let setup = with_store(&app, move |store| store.save(&name, &current)).await?;
    emit_setups(&app);
    Ok((&setup).into())
}

/// Replaces the settings of an existing setup with the ones now in force.
#[tauri::command]
pub async fn dj_setup_overwrite(app: AppHandle, id: String) -> CmdResult<DjSetupSummary> {
    let current = app.state::<AppState>().settings().dj;
    let setup = with_store(&app, move |store| store.overwrite(&id, &current)).await?;
    emit_setups(&app);
    Ok((&setup).into())
}

/// Makes a setup the current DJ settings (running DJ mode follows at once, except the pad sounds).
#[tauri::command]
pub async fn dj_setup_apply(app: AppHandle, id: String) -> CmdResult<DjSetupApplied> {
    let applied = with_store(&app, move |store| store.apply(&id)).await?;
    let state = app.state::<AppState>();
    let before = state.settings();
    let saved = state.save_settings(|s| s.dj = applied.clone())?;
    crate::dj::on_settings(&app, &before.dj, &saved.dj);
    let restart_needed = app.state::<Dj>().is_on() && before.dj.layout.pads != saved.dj.layout.pads;
    let _ = app.emit("settings", &saved);
    Ok(DjSetupApplied { settings: saved.dj, restart_needed })
}

#[tauri::command]
pub async fn dj_setup_rename(app: AppHandle, id: String, name: String) -> CmdResult<DjSetupSummary> {
    let setup = with_store(&app, move |store| store.rename(&id, &name)).await?;
    emit_setups(&app);
    Ok((&setup).into())
}

/// Removes HomeFi's own file of the setup. `false`: there was none.
#[tauri::command]
pub async fn dj_setup_delete(app: AppHandle, id: String) -> CmdResult<bool> {
    let removed = with_store(&app, move |store| store.delete(&id)).await?;
    emit_setups(&app);
    Ok(removed)
}

/// Writes the setup's `.homefidj` to `path` (picked with the dialog plugin). Returns the path written.
#[tauri::command]
pub async fn dj_setup_export(app: AppHandle, id: String, path: String) -> CmdResult<String> {
    let text = with_store(&app, move |store| store.export(&id)).await?;
    crate::transfer::write_export(&path, ExportKind::DjSetup, &text).await
}

/// Reads a `.homefidj` from `path` and saves it as a new setup (apply it with `dj_setup_apply`).
#[tauri::command]
pub async fn dj_setup_import(app: AppHandle, path: String) -> CmdResult<DjSetupSummary> {
    let text = crate::transfer::read_export(&path).await?;
    let setup = with_store(&app, move |store| store.import(&text)).await?;
    emit_setups(&app);
    Ok((&setup).into())
}
