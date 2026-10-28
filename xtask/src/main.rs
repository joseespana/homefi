//! Repository automation: `cargo xtask <command>`.
//!
//!   icons                     render assets/brand/*.svg to PNG / ICO
//!   bump <x.y.z|major|minor|patch>
//!                             set the workspace version (CLI, FFI, Android)
//!   version                   print the workspace version

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use regex::Regex;
use resvg::{tiny_skia, usvg};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("icons") => icons(),
        Some("bump") => bump(args.get(1).context("usage: cargo xtask bump <x.y.z|major|minor|patch>")?),
        Some("version") => {
            println!("{}", workspace_version()?);
            Ok(())
        }
        _ => {
            eprintln!("usage: cargo xtask <icons | bump <x.y.z|major|minor|patch> | version>");
            std::process::exit(2);
        }
    }
}

// ---------------------------------------------------------------------------
// icons
// ---------------------------------------------------------------------------

fn load_svg(path: &Path) -> Result<usvg::Tree> {
    let data = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let mut options = usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    Ok(usvg::Tree::from_data(&data, &options)?)
}

fn render(tree: &usvg::Tree, width: u32) -> Result<tiny_skia::Pixmap> {
    let size = tree.size();
    let scale = width as f32 / size.width();
    let height = (size.height() * scale).round() as u32;
    let mut pixmap = tiny_skia::Pixmap::new(width, height).context("invalid size")?;
    resvg::render(tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    Ok(pixmap)
}

fn rgba(pixmap: &tiny_skia::Pixmap) -> Vec<u8> {
    pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect()
}

fn icons() -> Result<()> {
    let brand = root().join("assets/brand");
    let out = brand.join("png");
    std::fs::create_dir_all(&out)?;

    let logo = load_svg(&brand.join("logo.svg"))?;
    for size in [16, 32, 48, 64, 128, 180, 192, 256, 512, 1024] {
        let file = out.join(format!("homefi-{size}.png"));
        render(&logo, size)?.save_png(&file)?;
        println!("wrote {}", file.display());
    }

    let mut ico = ico::IconDir::new(ico::ResourceType::Icon);
    for size in [16, 24, 32, 48, 64, 128, 256] {
        let pixmap = render(&logo, size)?;
        let image = ico::IconImage::from_rgba_data(size, size, rgba(&pixmap));
        ico.add_entry(ico::IconDirEntry::encode(&image)?);
    }
    let ico_file = brand.join("homefi.ico");
    ico.write(std::fs::File::create(&ico_file)?)?;
    println!("wrote {}", ico_file.display());

    let wordmark = load_svg(&brand.join("wordmark.svg"))?;
    let file = out.join("homefi-wordmark.png");
    render(&wordmark, 1100)?.save_png(&file)?;
    println!("wrote {}", file.display());

    // Play Store / F-Droid listing icon
    let store = root().join("apps/android/app/src/main/ic_launcher-playstore.png");
    render(&logo, 512)?.save_png(&store)?;
    println!("wrote {}", store.display());

    // iPhone app icon: square, iOS rounds the corners itself
    let square = load_svg(&brand.join("logo-square.svg"))?;
    let iphone = root().join("apps/ios/HomeFi/Assets.xcassets/AppIcon.appiconset/icon-1024.png");
    render(&square, 1024)?.save_png(&iphone)?;
    println!("wrote {}", iphone.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// bump
// ---------------------------------------------------------------------------

fn version_re() -> Regex {
    Regex::new(r#"(?s)(\[workspace\.package\].*?\nversion\s*=\s*")([^"]+)(")"#).unwrap()
}

fn workspace_version() -> Result<String> {
    let toml = std::fs::read_to_string(root().join("Cargo.toml"))?;
    let caps = version_re().captures(&toml).context("no [workspace.package] version in Cargo.toml")?;
    Ok(caps[2].to_owned())
}

fn parse_semver(v: &str) -> Result<(u64, u64, u64, Option<String>)> {
    let re = Regex::new(r"^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?$").unwrap();
    let c = re.captures(v).with_context(|| format!("`{v}` is not a semantic version (x.y.z[-pre])"))?;
    Ok((c[1].parse()?, c[2].parse()?, c[3].parse()?, c.get(4).map(|m| m.as_str().to_owned())))
}

fn bump(arg: &str) -> Result<()> {
    let current = workspace_version()?;
    let (major, minor, patch, pre) = parse_semver(&current)?;

    let next = match arg {
        "major" => format!("{}.0.0", major + 1),
        "minor" => format!("{major}.{}.0", minor + 1),
        // 1.2.3-beta.1 -> 1.2.3, 1.2.3 -> 1.2.4
        "patch" if pre.is_some() => format!("{major}.{minor}.{patch}"),
        "patch" => format!("{major}.{minor}.{}", patch + 1),
        explicit => {
            let explicit = explicit.trim_start_matches('v');
            parse_semver(explicit)?;
            explicit.to_owned()
        }
    };
    if next == current {
        bail!("version is already {current}");
    }
    let (n_major, n_minor, n_patch, _) = parse_semver(&next)?;
    if n_minor >= 1000 || n_patch >= 1000 {
        bail!("minor and patch must be < 1000 (Android versionCode = major*1e6 + minor*1e3 + patch)");
    }

    let cargo_toml = root().join("Cargo.toml");
    let text = std::fs::read_to_string(&cargo_toml)?;
    let updated = version_re().replace(&text, format!("${{1}}{next}${{3}}").as_str()).into_owned();
    std::fs::write(&cargo_toml, updated)?;

    // refresh the versions of the workspace crates in Cargo.lock
    let status = Command::new(env!("CARGO")).args(["update", "--workspace", "--offline"]).current_dir(root()).status();
    if !status.map(|s| s.success()).unwrap_or(false) {
        eprintln!("warning: `cargo update --workspace` failed, run it manually");
    }

    prepend_changelog(&next)?;

    let code = n_major * 1_000_000 + n_minor * 1_000 + n_patch;
    println!("homefi {current} -> {next} (Android versionCode {code})");
    println!();
    println!("next steps:");
    println!("  1. edit CHANGELOG.md");
    println!("  2. git commit -am \"release: v{next}\"");
    println!("  3. git tag v{next} && git push --follow-tags   # CI builds and publishes the release");
    Ok(())
}

fn prepend_changelog(version: &str) -> Result<()> {
    let file = root().join("CHANGELOG.md");
    let text = std::fs::read_to_string(&file).unwrap_or_else(|_| "# Changelog\n".into());
    let marker = "## [Unreleased]";
    let today = today();
    let section = format!("{marker}\n\n## [{version}] - {today}\n");
    let updated = if text.contains(marker) {
        text.replacen(marker, &section, 1)
    } else {
        let (head, rest) = text.split_once('\n').unwrap_or((&text, ""));
        format!("{head}\n\n{section}\n{rest}")
    };
    std::fs::write(file, updated)?;
    Ok(())
}

fn today() -> String {
    // days since epoch -> civil date (Howard Hinnant's algorithm), avoids a chrono dependency
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let z = secs / 86_400 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}
