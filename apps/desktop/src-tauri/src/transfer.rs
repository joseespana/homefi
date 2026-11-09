//! Reading and writing export files (`.homefiride`, `.homefidj`) at a path the user picked with the
//! dialog plugin. A Google Drive for desktop folder is just a folder. The text and its rules live in
//! `homefi_core::export`; this only moves it to and from the disk, off the async runtime.

use std::path::{Path, PathBuf};

use homefi_core::export::{ExportKind, MAX_EXPORT_BYTES};
use homefi_core::Error;

use crate::commands::err;

type CmdResult<T> = Result<T, String>;

/// `path` ending in the kind's extension: added (not replacing another ending) when it is missing.
fn with_export_extension(path: &str, kind: ExportKind) -> PathBuf {
    let wanted = kind.extension();
    let path = Path::new(path);
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case(wanted)) {
        return path.to_path_buf();
    }
    let mut name = path.as_os_str().to_owned();
    name.push(format!(".{wanted}"));
    PathBuf::from(name)
}

/// Writes `text` atomically; returns the path written.
pub async fn write_export(path: &str, kind: ExportKind, text: &str) -> CmdResult<String> {
    let target = with_export_extension(path, kind);
    let (file, bytes) = (target.clone(), text.as_bytes().to_vec());
    tauri::async_runtime::spawn_blocking(move || homefi_core::paths::write_atomic(&file, &bytes))
        .await
        .map_err(err)?
        .map_err(err)?;
    Ok(target.to_string_lossy().into_owned())
}

/// The text of an export file; a file too big to be one is refused before it is read.
pub async fn read_export(path: &str) -> CmdResult<String> {
    let path = PathBuf::from(path);
    tauri::async_runtime::spawn_blocking(move || -> homefi_core::Result<String> {
        let size = std::fs::metadata(&path)?.len();
        if size > MAX_EXPORT_BYTES as u64 {
            return Err(Error::Export(match homefi_core::humane::language() {
                homefi_core::humane::Lang::Es => "Este archivo es demasiado grande para ser de HomeFi.".into(),
                homefi_core::humane::Lang::En => "This file is too big to be a HomeFi file.".into(),
            }));
        }
        Ok(std::fs::read_to_string(&path)?)
    })
    .await
    .map_err(err)?
    .map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_extension_is_added_not_swapped() {
        assert_eq!(with_export_extension("/a/ride", ExportKind::Ride), Path::new("/a/ride.homefiride"));
        assert_eq!(with_export_extension("/a/My.ride", ExportKind::Ride), Path::new("/a/My.ride.homefiride"));
        assert_eq!(with_export_extension("/a/x.HOMEFIRIDE", ExportKind::Ride), Path::new("/a/x.HOMEFIRIDE"));
        assert_eq!(with_export_extension("/a/club", ExportKind::DjSetup), Path::new("/a/club.homefidj"));
    }

    #[tokio::test]
    async fn write_then_read_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("deep/club").to_string_lossy().into_owned();
        let written = write_export(&path, ExportKind::DjSetup, "{}").await.unwrap();
        assert!(written.ends_with("club.homefidj"));
        assert_eq!(read_export(&written).await.unwrap(), "{}");
        assert!(read_export(&path).await.is_err(), "no such file");
    }
}
