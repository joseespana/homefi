//! macOS updates that keep the app bundle's folder.
//!
//! tauri-plugin-updater swaps the whole `HomeFi.app` folder for the new one.
//! The Dock (and Finder aliases) remember an app by its folder's file id, so
//! after that swap the pinned icon points at the old folder, which is then
//! deleted: the new version opens as a second icon and the pinned one is left
//! dead. Here only `HomeFi.app/Contents` is exchanged, in one atomic step, so
//! the folder (and its file id) never changes.
//!
//! Anything that fails before the swap returns an error and changes nothing;
//! the caller then falls back to the plugin's own install (which also handles
//! a bundle that needs an administrator password).

use std::ffi::CString;
use std::io::{self, Cursor};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use flate2::read::GzDecoder;

/// LaunchServices' register tool: refreshes the app's name, icon and version after the swap.
const LSREGISTER: &str =
    "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";

/// The running app's bundle (`…/HomeFi.app`), when the executable is inside one.
pub fn current_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    // …/HomeFi.app/Contents/MacOS/homefi-desktop
    let bundle = exe.parent()?.parent()?.parent()?.to_path_buf();
    (bundle.extension()? == "app").then_some(bundle)
}

/// Installs the update `bytes` (the signed `.app.tar.gz`, already verified) into `bundle`.
pub fn install(bundle: &Path, bytes: &[u8]) -> io::Result<()> {
    let app_dev = std::fs::symlink_metadata(bundle)?.dev();
    // the swap is a rename, so the new files go on the bundle's volume
    let root = [Some(std::env::temp_dir()), bundle.parent().map(Path::to_path_buf)]
        .into_iter()
        .flatten()
        .find(|dir| dir.metadata().is_ok_and(|m| m.dev() == app_dev))
        .ok_or_else(|| io::Error::other("no temporary folder on the app's volume"))?;
    let staged = tempfile::Builder::new().prefix("homefi_update").tempdir_in(root)?;

    // the archive holds `HomeFi.app/…`: unpack it without its first component
    let mut archive = tar::Archive::new(GzDecoder::new(Cursor::new(bytes)));
    for entry in archive.entries()? {
        let mut entry = entry?;
        let inside: PathBuf = entry.path()?.iter().skip(1).collect();
        if inside.as_os_str().is_empty() {
            continue;
        }
        let to = staged.path().join(&inside);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        entry.unpack(&to)?;
    }
    let new_contents = staged.path().join("Contents");
    if !new_contents.join("Info.plist").is_file() {
        return Err(io::Error::other("the update has no Contents/Info.plist"));
    }

    // one atomic exchange: the old Contents ends up in the staging folder, removed on drop
    swap(&bundle.join("Contents"), &new_contents)?;

    // a new modification date and a fresh registration, so Finder and the Dock show the new version
    let now = std::fs::FileTimes::new().set_modified(std::time::SystemTime::now());
    std::fs::File::open(bundle).and_then(|f| f.set_times(now)).ok();
    Command::new(LSREGISTER).arg("-f").arg(bundle).status().ok();
    Ok(())
}

/// Longest wait for this process to end before the new version opens anyway.
const RELAUNCH_WAIT_TENTHS: u32 = 300;

/// Shell: waits while process `$1` lives, `$3` tenths of a second at most.
const WAIT_FOR_EXIT: &str =
    r#"i=0; while kill -0 "$1" 2>/dev/null && [ "$i" -lt "$3" ]; do sleep 0.1; i=$((i+1)); done"#;

/// Opens `bundle` once this process has exited, through LaunchServices.
///
/// Tauri's restart runs `open -n` while the old process is still alive: two copies of the
/// app run for a moment and the Dock gives the new one its own icon, next to the pinned one.
/// A small detached shell waits for this process to end, then opens the app normally, so the
/// Dock sees one app and lights its pinned icon. The caller exits right after.
pub fn relaunch_after_exit(bundle: &Path) -> io::Result<()> {
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;
    Command::new("/bin/sh")
        .arg("-c")
        .arg(format!(r#"{WAIT_FOR_EXIT}; exec /usr/bin/open "$2""#))
        .arg("homefi-relaunch")
        .arg(std::process::id().to_string())
        .arg(bundle)
        .arg(RELAUNCH_WAIT_TENTHS.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        // its own process group: it outlives us and nothing we send reaches it
        .process_group(0)
        .spawn()
        .map(drop)
}

/// Exchanges two paths on one volume in a single step (`renamex_np` with `RENAME_SWAP`, APFS).
fn swap(a: &Path, b: &Path) -> io::Result<()> {
    let a = CString::new(a.as_os_str().as_bytes())?;
    let b = CString::new(b.as_os_str().as_bytes())?;
    // SAFETY: both are valid NUL-terminated paths that live across the call; renamex_np only reads them.
    let result = unsafe { libc::renamex_np(a.as_ptr(), b.as_ptr(), libc::RENAME_SWAP) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A `.app.tar.gz` like the bundler's: `HomeFi.app/Contents/{Info.plist,MacOS/bin}`.
    fn archive(version: &str) -> Vec<u8> {
        let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast()));
        for (path, body) in [("HomeFi.app/Contents/Info.plist", version), ("HomeFi.app/Contents/MacOS/bin", "x")] {
            let mut header = tar::Header::new_gnu();
            header.set_size(body.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            builder.append_data(&mut header, path, body.as_bytes()).unwrap();
        }
        builder.into_inner().unwrap().finish().unwrap()
    }

    #[test]
    fn the_bundle_folder_is_kept_and_its_contents_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = dir.path().join("HomeFi.app");
        std::fs::create_dir_all(bundle.join("Contents/MacOS")).unwrap();
        std::fs::File::create(bundle.join("Contents/Info.plist")).unwrap().write_all(b"1.5.2").unwrap();
        std::fs::write(bundle.join("Contents/old-only"), "gone").unwrap();
        let inode = std::fs::metadata(&bundle).unwrap().ino();

        install(&bundle, &archive("1.5.3")).unwrap();

        assert_eq!(std::fs::metadata(&bundle).unwrap().ino(), inode, "same folder for the Dock");
        assert_eq!(std::fs::read_to_string(bundle.join("Contents/Info.plist")).unwrap(), "1.5.3");
        assert!(bundle.join("Contents/MacOS/bin").is_file());
        assert!(!bundle.join("Contents/old-only").exists(), "nothing of the old version is left");
    }

    #[test]
    fn the_relaunch_waits_for_the_old_process() {
        // opening needs LaunchServices; the waiting is what matters: not done while the pid lives
        let script = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(format!(r#"{WAIT_FOR_EXIT}; echo "$i""#))
            .arg("t")
            .arg(std::process::id().to_string())
            .arg("unused")
            .arg("3")
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&script.stdout).trim(), "3", "waited while alive, up to the cap");
    }

    #[test]
    fn a_broken_update_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = dir.path().join("HomeFi.app");
        std::fs::create_dir_all(bundle.join("Contents")).unwrap();
        std::fs::write(bundle.join("Contents/Info.plist"), "1.5.2").unwrap();

        assert!(install(&bundle, b"not a tarball").is_err());

        assert_eq!(std::fs::read_to_string(bundle.join("Contents/Info.plist")).unwrap(), "1.5.2");
    }
}
