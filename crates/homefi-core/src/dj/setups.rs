//! Named DJ setups: the whole of [`DjSettings`] (options, keymap and screen layout) saved under a
//! name, applied later or on another machine. One JSON file per setup in
//! `<app dir>/dj/setups/<id>.json`; exported as `.homefidj` through the shared envelope.
//!
//! A setup holds no account data and no paths. Platform-specific parts (the desktop keymap) travel
//! along and are ignored where they do not apply (Android).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::DjSettings;
use crate::error::{Error, Result};
use crate::export::{self, ExportKind};
use crate::humane::{language, Lang};
use crate::json_dir::{new_id, JsonDir};
use crate::playlist::now_ms;

pub const DJ_SETUP_FORMAT_VERSION: u32 = 1;
/// Folder of the setups inside the app folder.
pub const SETUPS_DIR: [&str; 2] = ["dj", "setups"];
const MAX_NAME_CHARS: usize = 80;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DjSetup {
    pub v: u32,
    /// `[A-Za-z0-9_-]{1,64}`.
    pub id: String,
    pub name: String,
    pub created_ms: u64,
    pub updated_ms: u64,
    pub settings: DjSettings,
}

impl Default for DjSetup {
    fn default() -> Self {
        Self {
            v: DJ_SETUP_FORMAT_VERSION,
            id: String::new(),
            name: String::new(),
            created_ms: 0,
            updated_ms: 0,
            settings: DjSettings::default(),
        }
    }
}

fn clean_name(name: &str) -> String {
    name.trim().chars().take(MAX_NAME_CHARS).collect()
}

fn blank_name(lang: Lang) -> Error {
    Error::Export(match lang {
        Lang::En => "Give the setup a name.".into(),
        Lang::Es => "Ponle un nombre a la configuración.".into(),
    })
}

#[derive(Debug, Clone)]
pub struct DjSetupStore {
    files: JsonDir,
}

impl DjSetupStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { files: JsonDir::new(dir) }
    }

    /// The store inside the app folder (`<app dir>/dj/setups`).
    pub fn in_app_dir(app_dir: &Path) -> Self {
        Self::new(SETUPS_DIR.iter().fold(app_dir.to_path_buf(), |dir, part| dir.join(part)))
    }

    /// Every readable setup by name (a file that doesn't parse is skipped).
    pub fn list(&self) -> Vec<DjSetup> {
        let mut setups: Vec<DjSetup> = self.files.list::<DjSetup>().into_iter().filter(|s| !s.id.is_empty()).collect();
        setups.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.id.cmp(&b.id)));
        setups
    }

    pub fn get(&self, id: &str) -> Result<DjSetup> {
        self.files.load(id)
    }

    /// Saves the current `settings` (clamped) as a new setup called `name`.
    pub fn save(&self, name: &str, settings: &DjSettings) -> Result<DjSetup> {
        let name = clean_name(name);
        if name.is_empty() {
            return Err(blank_name(language()));
        }
        let now = now_ms();
        let setup = DjSetup {
            id: new_id("d"),
            name,
            created_ms: now,
            updated_ms: now,
            settings: settings.clamped(),
            ..DjSetup::default()
        };
        self.files.save(&setup.id, &setup)?;
        Ok(setup)
    }

    /// Replaces the settings of an existing setup with `settings` (keeps its name).
    pub fn overwrite(&self, id: &str, settings: &DjSettings) -> Result<DjSetup> {
        let mut setup = self.get(id)?;
        setup.settings = settings.clamped();
        setup.updated_ms = now_ms();
        self.files.save(id, &setup)?;
        Ok(setup)
    }

    /// The settings of a setup, clamped, ready to become the current ones.
    pub fn apply(&self, id: &str) -> Result<DjSettings> {
        Ok(self.get(id)?.settings.clamped())
    }

    pub fn rename(&self, id: &str, name: &str) -> Result<DjSetup> {
        let name = clean_name(name);
        if name.is_empty() {
            return Err(blank_name(language()));
        }
        let mut setup = self.get(id)?;
        setup.name = name;
        setup.updated_ms = now_ms();
        self.files.save(id, &setup)?;
        Ok(setup)
    }

    /// Deletes the setup's file (HomeFi's own data); `false` when there is none.
    pub fn delete(&self, id: &str) -> Result<bool> {
        self.files.remove(id)
    }

    /// The shareable text of a setup (`.homefidj`).
    pub fn export(&self, id: &str) -> Result<String> {
        export_setup(&self.get(id)?)
    }

    /// Reads an export text and saves it as a new setup.
    pub fn import(&self, text: &str) -> Result<DjSetup> {
        let setup = import_setup(text)?;
        self.files.save(&setup.id, &setup)?;
        Ok(setup)
    }
}

/// What `data` of a DJ setup export holds: the name and the settings, nothing else.
#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
struct SetupFile {
    v: u32,
    name: String,
    settings: DjSettings,
}

pub fn export_setup(setup: &DjSetup) -> Result<String> {
    let file = SetupFile { v: DJ_SETUP_FORMAT_VERSION, name: setup.name.clone(), settings: setup.settings.clamped() };
    export::wrap(ExportKind::DjSetup, &file)
}

/// A setup (not yet saved) from export text. A file from a newer HomeFi, of another kind or
/// damaged is refused with words.
pub fn import_setup(text: &str) -> Result<DjSetup> {
    let lang = language();
    let file: SetupFile = export::unwrap(ExportKind::DjSetup, text)?;
    if file.v > DJ_SETUP_FORMAT_VERSION {
        return Err(Error::Export(match lang {
            Lang::En => "This DJ setup was saved by a newer HomeFi. Update the app to open it.".into(),
            Lang::Es => "Esta configuración de DJ la guardó una versión más nueva de HomeFi. Actualiza la app.".into(),
        }));
    }
    let name = clean_name(&file.name);
    let now = now_ms();
    Ok(DjSetup {
        id: new_id("d"),
        name: if name.is_empty() { "DJ".into() } else { name },
        created_ms: now,
        updated_ms: now,
        settings: file.settings.clamped(),
        ..DjSetup::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dj::layout::{DjSection, DjSectionKind};
    use crate::dj::SfxKind;

    fn custom() -> DjSettings {
        let mut s = DjSettings { quantize: false, tempo_range_pct: 16.0, ..DjSettings::default() };
        s.key_overrides.insert("deck_a.play".into(), "x".into());
        s.homifito = crate::dj::HomifitoDj {
            mode: crate::dj::HomifitoMode::Big,
            place: crate::dj::HomifitoPlace::Bottom,
            look: true,
        };
        s.layout.pads[0] = SfxKind::Hit;
        s.layout.sections[0] = DjSection { kind: DjSectionKind::DeckA, visible: false };
        s
    }

    fn store() -> (tempfile::TempDir, DjSetupStore) {
        let tmp = tempfile::tempdir().unwrap();
        let store = DjSetupStore::in_app_dir(tmp.path());
        (tmp, store)
    }

    #[test]
    fn save_apply_rename_delete() {
        let (_tmp, store) = store();
        let saved = store.save("  Club  ", &custom()).unwrap();
        assert_eq!(saved.name, "Club");
        assert_eq!(store.apply(&saved.id).unwrap(), custom());
        assert!(store.save("   ", &custom()).is_err());
        assert_eq!(store.rename(&saved.id, "Home").unwrap().name, "Home");
        store.save("alpha", &DjSettings::default()).unwrap();
        let names: Vec<String> = store.list().into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["alpha", "Home"], "sorted by name, any case");
        let changed = store.overwrite(&saved.id, &DjSettings::default()).unwrap();
        assert_eq!(changed.name, "Home");
        assert_eq!(store.apply(&saved.id).unwrap(), DjSettings::default());
        assert!(store.delete(&saved.id).unwrap() && !store.delete(&saved.id).unwrap());
        assert!(store.apply("../x").is_err());
    }

    #[test]
    fn export_and_import_round_trip_with_a_new_id() {
        let (_tmp, store) = store();
        let saved = store.save("Club", &custom()).unwrap();
        let text = store.export(&saved.id).unwrap();
        assert!(text.contains("\"kind\": \"dj_setup\""));
        let (_tmp2, other) = self::store();
        let imported = other.import(&text).unwrap();
        assert_ne!(imported.id, saved.id);
        assert_eq!(imported.name, "Club");
        assert_eq!(imported.settings, custom(), "keymap, layout and Homifito travel");
        assert_eq!(other.list().len(), 1);
    }

    #[test]
    fn refuses_other_kinds_newer_files_and_garbage() {
        let (_tmp, store) = store();
        let ride = export::wrap(ExportKind::Ride, &1).unwrap();
        assert!(store.import(&ride).is_err());
        assert!(store.import("nope").is_err());
        let ok = export_setup(&DjSetup { name: "x".into(), ..DjSetup::default() }).unwrap();
        let newer = ok.replacen("\"v\": 1", "\"v\": 5", 1);
        assert!(store.import(&newer).unwrap_err().to_string().to_lowercase().contains("newer"));
        assert!(store.list().is_empty());
    }

    #[test]
    fn an_old_setup_without_homifito_gets_the_defaults() {
        let text = r#"{"homefi_export":1,"kind":"dj_setup","data":{"v":1,"name":"old","settings":{"quantize":false}}}"#;
        let s = import_setup(text).unwrap();
        assert_eq!(s.settings.homifito, crate::dj::HomifitoDj::default());
    }

    #[test]
    fn imported_settings_are_clamped() {
        let text = r#"{"homefi_export":1,"kind":"dj_setup","data":{"v":1,"name":"w","settings":
            {"tempo_range_pct":900,"layout":{"pads":["kazoo"],"sections":[]}}}}"#;
        let s = import_setup(text).unwrap();
        assert_eq!(s.settings.tempo_range_pct, 50.0);
        assert_eq!(s.settings.layout, crate::dj::layout::DjLayout::default());
    }

    #[test]
    fn a_setup_holds_no_paths_or_secrets() {
        let text = export_setup(&DjSetup { name: "x".into(), ..DjSetup::default() }).unwrap();
        for word in ["token", "key\"", "/Users", "password"] {
            assert!(!text.to_lowercase().contains(&word.to_lowercase()), "{word}");
        }
    }
}
