//! Filesystem locations used by HomeFi.

use std::path::{Component, Path, PathBuf};

/// Environment variable overriding the app directory.
pub const APP_PATH_ENV: &str = "HOMEFI_PATH";
pub const APP_DIR_NAME: &str = ".homefi";

pub const CONFIG_FILENAME: &str = "config.toml";
pub const AUTH_FILENAME: &str = "auth.json";
pub const CACHE_DIRNAME: &str = "api_cache";
pub const DEBUG_DIRNAME: &str = "api_debug";

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// `$HOMEFI_PATH` or `~/.homefi`.
pub fn app_dir() -> PathBuf {
    match std::env::var_os(APP_PATH_ENV) {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => home().join(APP_DIR_NAME),
    }
}

/// `~/Music/homefi`.
pub fn default_download_dir() -> PathBuf {
    home().join("Music").join("homefi")
}

/// Expands a leading `~` to the home directory.
pub fn expand_tilde(path: &Path) -> PathBuf {
    match path.strip_prefix("~") {
        Ok(rest) => home().join(rest),
        Err(_) => path.to_path_buf(),
    }
}

/// Writes `bytes` to `path` all at once: into a temporary file next to it
/// (mode `0600` on Unix), then renamed over it. A reader never sees half a
/// file and a crash mid-write leaves the old one. Parent folders are created.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let dir = match path.parent() {
        Some(d) if !d.as_os_str().is_empty() => d,
        _ => Path::new("."),
    };
    std::fs::create_dir_all(dir)?;
    let mut tmp = tempfile::Builder::new().prefix(".homefi-").suffix(".tmp").tempfile_in(dir)?;
    tmp.write_all(bytes)?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

/// Replaces (or appends) the extension of the *last* path component.
///
/// Mirrors Python's `Path.with_suffix`: only the text after the last dot of
/// the file name is replaced (template paths end with a `.*` placeholder).
pub fn with_suffix(path: &Path, ext: &str) -> PathBuf {
    let ext = ext.trim_start_matches('.');
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let stem = match name.rfind('.') {
        Some(i) if i > 0 => &name[..i],
        _ => name.as_str(),
    };
    path.with_file_name(format!("{stem}.{ext}"))
}

/// Joins `relative` onto `base`, reusing the casing of path components that
/// already exist on disk (useful on case-sensitive file systems).
pub fn resolve_existing_path_case(base: &Path, relative: &Path) -> PathBuf {
    let mut resolved = base.to_path_buf();

    for component in relative.components() {
        let Component::Normal(part) = component else {
            continue;
        };
        let part = part.to_string_lossy();
        let existing = find_existing_child_case(&resolved, &part);
        resolved.push(existing.as_deref().unwrap_or(&part));
    }

    resolved
}

fn find_existing_child_case(parent: &Path, name: &str) -> Option<String> {
    let entries = std::fs::read_dir(parent).ok()?;
    let lowered = name.to_lowercase();
    let mut fallback = None;

    for entry in entries.flatten() {
        let child = entry.file_name().to_string_lossy().into_owned();
        if child == name {
            return Some(child);
        }
        if fallback.is_none() && child.to_lowercase() == lowered {
            fallback = Some(child);
        }
    }

    fallback
}

/// Path of `path` relative to `base` (both absolute), using `..` when needed.
pub fn relative_to(path: &Path, base: &Path) -> PathBuf {
    let path: Vec<_> = path.components().collect();
    let base: Vec<_> = base.components().collect();
    let common = path.iter().zip(&base).take_while(|(a, b)| a == b).count();

    let mut out = PathBuf::new();
    for _ in common..base.len() {
        out.push("..");
    }
    for c in &path[common..] {
        out.push(c.as_os_str());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_writes_replace_whole_files_and_leave_no_temporaries() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("sub/data.json");
        write_atomic(&file, b"first").unwrap();
        write_atomic(&file, b"second").unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"second");
        let names: Vec<_> =
            std::fs::read_dir(file.parent().unwrap()).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(names, vec![std::ffi::OsString::from("data.json")]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
        }
    }

    #[test]
    fn suffix_replaces_only_last_extension() {
        assert_eq!(with_suffix(Path::new("a/b/Song.*"), ".flac"), PathBuf::from("a/b/Song.flac"));
        assert_eq!(with_suffix(Path::new("a/Mr. Song.*"), "m4a"), PathBuf::from("a/Mr. Song.m4a"));
        assert_eq!(with_suffix(Path::new("a/Song"), "mp3"), PathBuf::from("a/Song.mp3"));
    }

    #[test]
    fn relative_paths() {
        assert_eq!(
            relative_to(Path::new("/m/Artist/Album/1.flac"), Path::new("/m/m3u/album")),
            PathBuf::from("../../Artist/Album/1.flac")
        );
    }

    #[test]
    fn existing_case_is_reused() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("FooBar")).unwrap();
        let resolved = resolve_existing_path_case(dir.path(), Path::new("foobar/new/x.flac"));
        assert_eq!(resolved, dir.path().join("FooBar/new/x.flac"));
    }
}
