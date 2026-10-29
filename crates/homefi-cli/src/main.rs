mod progress;
mod search;
mod update;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand};
use console::style;
use homefi_core::config::{AudioFormat, MediaFilter, SinglesFilter, EXAMPLE_CONFIG};
use homefi_core::download::{DownloadOptions, Downloader};
use homefi_core::quality::{TrackQuality, VideoQuality};
use homefi_core::resource::{Resource, ResourceKind};
use homefi_core::session::{LoginPoll, Session};
use homefi_core::{ffmpeg, paths};
use indicatif::{ProgressBar, ProgressStyle};

use crate::progress::CliProgress;

/// Refresh the token before downloading when it expires within 10 minutes.
const EARLY_EXPIRE: i64 = 600;

#[derive(Parser)]
#[command(
    name = "homefi",
    version,
    about = "HomeFi - download tracks and videos from Tidal in max quality ♫",
    after_help = "Docs: https://github.com/joseespana/homefi",
    arg_required_else_help = true
)]
struct Cli {
    /// Ignore cached API responses.
    #[arg(long, global = true)]
    omit_cache: bool,

    /// Save raw API responses to <app dir>/api_debug.
    #[arg(long, global = true)]
    debug: bool,

    /// Verbose logging (-v info, -vv debug).
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Manage Tidal authentication.
    #[command(subcommand)]
    Auth(AuthCommand),

    /// Download Tidal resources.
    #[command(visible_alias = "dl")]
    Download(DownloadArgs),

    /// Inspect or create the config file.
    #[command(subcommand)]
    Config(ConfigCommand),

    /// List audio output devices and the sample rates they accept.
    Devices,

    /// Play local audio files (FLAC, ALAC, WAV, AIFF, MP3, AAC, Vorbis, ...).
    Play {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Output device name (see `homefi devices`); default: system output.
        #[arg(short, long)]
        device: Option<String>,
        /// Volume from 0.0 to 1.0.
        #[arg(long, default_value_t = 0.8)]
        volume: f32,
        /// Stop after this many seconds (useful for tests).
        #[arg(long)]
        seconds: Option<u64>,
    },
}

#[derive(Subcommand)]
enum AuthCommand {
    /// Log in with your Tidal account.
    Login {
        /// Do not open the browser.
        #[arg(short = 'n', long)]
        no_browser: bool,
        /// Skip printing the personal-use notice (you've read and accept it).
        #[arg(long, env = "HOMEFI_ACCEPT_TERMS")]
        accept_terms: bool,
    },
    /// Log out and remove the token.
    Logout {
        /// Clear local credentials even if the server request fails.
        #[arg(short, long)]
        force: bool,
    },
    /// Refresh the access token.
    Refresh {
        /// Refresh even when the token is still valid.
        #[arg(short, long)]
        force: bool,
        /// Treat the token as expired this many seconds earlier.
        #[arg(short, long, value_name = "SECONDS", default_value_t = 0)]
        early_expire: i64,
    },
    /// Show the login status.
    Status,
}

#[derive(Subcommand)]
enum ConfigCommand {
    /// Print the app directory and config file location.
    Path,
    /// Print the effective configuration.
    Show,
    /// Write a commented example config.toml.
    Init {
        /// Overwrite an existing config file.
        #[arg(short, long)]
        force: bool,
    },
    /// Delete the API response cache.
    ClearCache,
}

#[derive(Args)]
struct DownloadArgs {
    /// low, normal, high (FLAC 16-bit) or max (FLAC up to 24-bit).
    #[arg(short = 'q', long)]
    track_quality: Option<TrackQuality>,

    /// sd, hd or fhd.
    #[arg(long, visible_alias = "vq")]
    video_quality: Option<VideoQuality>,

    /// Download files that already exist.
    #[arg(long, visible_alias = "ns")]
    no_skip: bool,

    /// Rewrite tags of files that already exist.
    #[arg(short = 'r', long)]
    rewrite_metadata: bool,

    /// Number of concurrent downloads.
    #[arg(short = 't', long, value_parser = clap::value_parser!(u16).range(1..))]
    threads_count: Option<u16>,

    /// Base directory for downloads.
    #[arg(short = 'p', long)]
    path: Option<PathBuf>,

    /// Directory searched for existing downloads.
    #[arg(long, visible_alias = "sp")]
    scan_path: Option<PathBuf>,

    /// Output path template (see docs/templating.md).
    #[arg(short = 'o', long)]
    output: Option<String>,

    /// Artist singles: none, only or include.
    #[arg(short = 's', long)]
    singles: Option<SinglesFilter>,

    /// Videos: none, allow or only.
    #[arg(long, visible_alias = "vid")]
    videos: Option<MediaFilter>,

    /// Dolby Atmos: none, allow or only.
    #[arg(long, visible_alias = "da")]
    dolby_atmos: Option<MediaFilter>,

    /// Output audio format: original, flac or mp3 (needs ffmpeg).
    #[arg(short = 'f', long)]
    format: Option<AudioFormat>,

    /// Stop at the first error.
    #[arg(long, visible_alias = "err")]
    raise_errors: bool,

    #[command(subcommand)]
    source: Source,
}

#[derive(Subcommand)]
enum Source {
    /// Tidal links or `type/id` (track/123, album/456, playlist/<uuid>, mix/<id>, artist/7, video/8).
    Url {
        #[arg(required = true, value_parser = parse_resource)]
        urls: Vec<Resource>,
    },
    /// Your Tidal favorites.
    Fav {
        /// Resource types to include (track, video, album, playlist, artist).
        #[arg(short = 't', long = "types", value_delimiter = ',', default_values = ["track", "video", "album", "playlist", "artist"])]
        types: Vec<ResourceKind>,
    },
    /// Search Tidal and pick what to download.
    Search {
        query: String,
        /// Resource types to show (track, video, album, playlist, artist).
        #[arg(short = 't', long = "types", value_delimiter = ',', default_values = ["track", "video", "album", "playlist", "artist"])]
        types: Vec<ResourceKind>,
        /// Results shown per type.
        #[arg(short = 'n', long = "num-top", default_value_t = 3)]
        num_top: usize,
        /// Download the top hit without asking.
        #[arg(short = 'T', long = "top")]
        top: bool,
    },
}

fn parse_resource(s: &str) -> Result<Resource, String> {
    Resource::parse(s).map_err(|e| e.to_string())
}

fn init_logging(verbose: u8) {
    let default = match verbose {
        0 => "warn",
        1 => "homefi_core=info,homefi=info,warn",
        _ => "homefi_core=debug,homefi=debug,info",
    };
    let filter = tracing_subscriber::EnvFilter::try_from_env("HOMEFI_LOG").unwrap_or_else(|_| default.into());
    tracing_subscriber::fmt().with_env_filter(filter).with_writer(std::io::stderr).with_target(false).init();
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    init_logging(cli.verbose);

    if let Err(e) = run(cli).await {
        eprintln!("{} {e:#}", style("error:").red().bold());
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<()> {
    let session = Session::open_default().context("could not load HomeFi")?;
    let omit_cache = cli.omit_cache || !session.config().enable_cache;
    let debug = cli.debug || session.config().debug;
    let session = session.omit_cache(omit_cache).debug(debug);
    // "A new version is available" (at most once a day, never installs)
    let notice = update::start(&session);

    let result = match cli.command {
        Command::Auth(cmd) => auth(&session, cmd).await,
        Command::Config(cmd) => config(&session, cmd),
        Command::Download(args) => download(&session, args).await,
        Command::Devices => {
            devices();
            Ok(())
        }
        Command::Play { files, device, volume, seconds } => play(files, device, volume, seconds).await,
    };
    if result.is_ok() {
        notice.finish().await;
    }
    result
}

async fn auth(session: &Session, cmd: AuthCommand) -> Result<()> {
    match cmd {
        AuthCommand::Login { no_browser, accept_terms } => login(session, no_browser, accept_terms).await,
        AuthCommand::Logout { force } => {
            if session.auth_data()?.token.is_none() {
                eprintln!("{}", style("No active session found.").yellow());
                return Ok(());
            }
            match session.logout(force).await {
                Ok(()) => eprintln!("{}", style("Logged out.").green().bold()),
                Err(e) if force => eprintln!("{} {e}", style("Logout request failed, token removed anyway:").yellow()),
                Err(e) => bail!("logout request failed: {e} (use --force to remove the local token)"),
            }
            Ok(())
        }
        AuthCommand::Refresh { force, early_expire } => {
            if session.refresh(early_expire, force).await? {
                eprintln!("{}", style("Auth token refreshed.").green().bold());
            } else {
                print_expiry(session)?;
            }
            Ok(())
        }
        AuthCommand::Status => {
            let data = session.auth_data()?;
            if !data.is_logged_in() {
                eprintln!("{}", style("Not logged in. Run `homefi auth login`.").yellow());
                return Ok(());
            }
            eprintln!(
                "Logged in as user {} ({})",
                style(data.user_id.as_deref().unwrap_or("?")).bold(),
                data.country_code.as_deref().unwrap_or("?")
            );
            print_expiry(session)
        }
    }
}

fn print_expiry(session: &Session) -> Result<()> {
    let left = session.auth_data()?.expires_at - homefi_core::auth::now_secs();
    if left <= 0 {
        eprintln!("{}", style("Auth token expired, it will be refreshed on next use.").yellow());
    } else {
        let (d, h, m) = (left / 86_400, left % 86_400 / 3600, left % 3600 / 60);
        eprintln!("{}", style(format!("Auth token expires in {d}d {h}h {m}m")).green());
    }
    Ok(())
}

/// The personal-use notice the desktop and Android apps ask to accept before
/// connecting Tidal (same text, version 1).
const TERMS_NOTICE: &str = "HomeFi is an independent project, not affiliated with or endorsed by TIDAL. \
It requires your own paid TIDAL subscription. It is for personal, non-commercial use only: listening to \
and keeping copies, for yourself and on your own devices, of music your subscription gives you access to. \
Do not share, sell, upload or distribute the files, and do not use them in public or commercial settings. \
You are responsible for complying with TIDAL's terms and your country's copyright laws. If you cancel \
your subscription, delete what you downloaded.";

async fn login(session: &Session, no_browser: bool, accept_terms: bool) -> Result<()> {
    if session.is_logged_in() {
        eprintln!("{}", style("Already logged in.").cyan().bold());
        return Ok(());
    }
    if !accept_terms {
        // informative only: it never waits, so scripts keep working
        eprintln!("{}\n", style(TERMS_NOTICE).dim());
        eprintln!(
            "{}\n",
            style(
                "Logging in means you understand and accept this. (--accept-terms or HOMEFI_ACCEPT_TERMS=1 hides it.)"
            )
            .dim()
        );
    }

    let device = session.start_login().await?;
    let url = device.url();
    eprintln!("Open {} and approve the code {}", style(&url).underlined().cyan(), style(&device.user_code).bold());
    if !no_browser {
        let _ = open::that(&url);
    }

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(ProgressStyle::with_template("{spinner:.magenta} {msg}").unwrap());
    spinner.enable_steady_tick(Duration::from_millis(100));

    let deadline = Instant::now() + Duration::from_secs(device.expires_in.max(1) as u64);
    let interval = Duration::from_secs(device.interval.max(1) as u64);

    loop {
        let left = deadline.saturating_duration_since(Instant::now()).as_secs();
        spinner.set_message(format!("Waiting for approval... {}:{:02} left", left / 60, left % 60));
        tokio::time::sleep(interval).await;

        match session.poll_login(&device.device_code).await? {
            LoginPoll::Pending if Instant::now() < deadline => continue,
            LoginPoll::Success(_) => {
                spinner.finish_and_clear();
                eprintln!("{}", style("Logged in!").green().bold());
                return Ok(());
            }
            LoginPoll::Pending | LoginPoll::Expired => {
                spinner.finish_and_clear();
                bail!("the login code expired, run `homefi auth login` again");
            }
        }
    }
}

fn config(session: &Session, cmd: ConfigCommand) -> Result<()> {
    let config_file = session.app_dir().join(paths::CONFIG_FILENAME);
    match cmd {
        ConfigCommand::Path => {
            println!("app dir:     {}", session.app_dir().display());
            println!("config file: {}", config_file.display());
        }
        ConfigCommand::Show => {
            print!("{}", toml::to_string_pretty(session.config())?);
        }
        ConfigCommand::Init { force } => {
            if config_file.exists() && !force {
                bail!("{} already exists (use --force to overwrite)", config_file.display());
            }
            std::fs::write(&config_file, EXAMPLE_CONFIG)?;
            eprintln!("{} {}", style("Wrote").green(), config_file.display());
        }
        ConfigCommand::ClearCache => {
            session.cache().clear()?;
            eprintln!("{}", style("API cache cleared.").green());
        }
    }
    Ok(())
}

fn khz(rate: u32) -> String {
    if rate % 1000 == 0 {
        format!("{}", rate / 1000)
    } else {
        format!("{:.1}", rate as f64 / 1000.0)
    }
}

fn devices() {
    for d in homefi_player::list_devices() {
        let rates: Vec<String> = d.sample_rates.iter().map(|r| khz(*r)).collect();
        println!(
            "{} {}  {}",
            if d.is_default { style("*").green().bold() } else { style(" ") },
            style(&d.name).bold(),
            style(format!("{} kHz · {} ch", rates.join("/"), d.max_channels)).dim()
        );
    }
}

async fn play(files: Vec<PathBuf>, device: Option<String>, volume: f32, seconds: Option<u64>) -> Result<()> {
    use homefi_player::{Player, Status};

    let player = Player::new(volume, device, Arc::new(|_| {}));
    player.load(files, 0, 0, true);

    let started = Instant::now();
    let mut last_line = String::new();
    let mut seen_playing = false;
    loop {
        tokio::time::sleep(Duration::from_millis(250)).await;
        let s = player.snapshot();
        if let Some(e) = &s.error {
            eprintln!("{} {e}", style("error:").red());
        }
        if s.status == Status::Playing {
            seen_playing = true;
        }
        let name = s
            .current
            .and_then(|i| s.queue.get(i))
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned());
        let line = match (&name, &s.stream) {
            (Some(name), Some(st)) => format!(
                "{name}  {}:{:02}  {} {}-bit/{} kHz → {} kHz {}{}",
                s.position_ms / 60_000,
                s.position_ms / 1000 % 60,
                st.codec,
                st.bits_per_sample.map(|b| b.to_string()).unwrap_or("?".into()),
                khz(st.source_rate),
                khz(st.output_rate),
                st.device,
                if st.resampled { " (resampled)" } else { "" },
            ),
            _ => String::new(),
        };
        if !line.is_empty() && line != last_line {
            eprint!("\r\x1b[2K{line}");
            last_line = line;
        }
        // finished, or nothing could be played at all
        let failed = !seen_playing
            && s.status == Status::Stopped
            && s.error.is_some()
            && started.elapsed() > Duration::from_secs(1);
        if failed {
            anyhow::bail!("nothing could be played");
        }
        let done = seen_playing && s.status == Status::Stopped;
        if done || seconds.is_some_and(|max| started.elapsed() >= Duration::from_secs(max)) {
            eprintln!();
            let s = player.snapshot();
            println!("position={}ms status={:?}", s.position_ms, s.status);
            return Ok(());
        }
    }
}

async fn download(session: &Session, args: DownloadArgs) -> Result<()> {
    let config = session.config();
    let mut opts = DownloadOptions::from_config(config);

    if let Some(q) = args.track_quality {
        opts.track_quality = q;
    }
    if let Some(q) = args.video_quality {
        opts.video_quality = q;
    }
    if args.no_skip {
        opts.skip_existing = false;
    }
    if args.rewrite_metadata {
        opts.rewrite_metadata = true;
    }
    if let Some(t) = args.threads_count {
        opts.threads = t as usize;
    }
    if let Some(p) = &args.path {
        opts.download_dir = paths::expand_tilde(p);
        if config.download.scan_path.is_none() {
            opts.scan_dir = opts.download_dir.clone();
        }
    }
    if let Some(p) = &args.scan_path {
        opts.scan_dir = paths::expand_tilde(p);
    }
    if let Some(s) = args.singles {
        opts.singles_filter = s;
    }
    if let Some(v) = args.videos {
        opts.videos_filter = v;
    }
    if let Some(d) = args.dolby_atmos {
        opts.atmos_filter = d;
    }
    if let Some(f) = args.format {
        opts.audio_format = f;
    }
    opts.template_override = args.output.filter(|t| !t.trim().is_empty());
    opts.fail_fast = args.raise_errors;

    if !ffmpeg::is_available() {
        eprintln!(
            "{}",
            style("warning: ffmpeg was not found; FLAC is kept inside M4A, videos stay .ts and --format is ignored.")
                .yellow()
        );
    }

    session.refresh(EARLY_EXPIRE, false).await.context("could not refresh the Tidal token")?;
    let api = session.api()?;

    let resources = match args.source {
        Source::Url { urls } => urls,
        Source::Fav { types } => {
            let fav = api.favorites().await?;
            let mut resources = Vec::new();
            for kind in types {
                let ids = match kind {
                    ResourceKind::Track => &fav.track,
                    ResourceKind::Video => &fav.video,
                    ResourceKind::Album => &fav.album,
                    ResourceKind::Playlist => &fav.playlist,
                    ResourceKind::Artist => &fav.artist,
                    ResourceKind::Mix => continue,
                };
                eprintln!("{:<10}{}", format!("{kind}s:"), ids.len());
                resources.extend(ids.iter().map(|id| Resource::new(kind, id.clone())));
            }
            resources
        }
        Source::Search { query, types, num_top, top } => search::run(&api, &query, &types, num_top, top).await?,
    };

    if resources.is_empty() {
        return Ok(());
    }

    let progress = Arc::new(CliProgress::new(&opts.download_dir));
    let downloader = Downloader::new(api, opts, progress.clone());
    let summary = downloader.run(&resources).await;

    match summary {
        Ok(summary) => {
            progress.finish(&summary);
            if summary.failed > 0 {
                std::process::exit(2);
            }
            Ok(())
        }
        Err(e) => {
            progress.finish(&Default::default());
            Err(e.into())
        }
    }
}
