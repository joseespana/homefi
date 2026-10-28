//! Terminal rendering of download events.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use console::style;
use homefi_core::download::{Event, ExtraFile, Observer, Outcome, Summary};
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};

pub struct CliProgress {
    multi: MultiProgress,
    total: ProgressBar,
    tasks: Mutex<HashMap<u64, ProgressBar>>,
    download_dir: std::path::PathBuf,
}

fn spinner_style() -> ProgressStyle {
    ProgressStyle::with_template("{spinner:.magenta} {msg} {bytes:.dim} {binary_bytes_per_sec:.dim}")
        .unwrap()
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ ")
}

fn bar_style() -> ProgressStyle {
    ProgressStyle::with_template(
        "{spinner:.magenta} {msg} {bar:24.magenta/dim} {bytes:.dim}/{total_bytes:.dim} {binary_bytes_per_sec:.dim}",
    )
    .unwrap()
    .progress_chars("━╸─")
    .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ ")
}

impl CliProgress {
    pub fn new(download_dir: &Path) -> Self {
        let multi = MultiProgress::with_draw_target(ProgressDrawTarget::stderr_with_hz(10));
        let total = multi.add(ProgressBar::new(0));
        total.set_style(
            ProgressStyle::with_template("{elapsed_precise:.dim} {bar:40.green/dim} {pos}/{len} {msg:.dim}")
                .unwrap()
                .progress_chars("━╸─"),
        );
        total.enable_steady_tick(Duration::from_millis(250));
        Self { multi, total, tasks: Mutex::new(HashMap::new()), download_dir: download_dir.to_path_buf() }
    }

    fn display_path(&self, path: &Path) -> String {
        path.strip_prefix(&self.download_dir).unwrap_or(path).display().to_string()
    }

    fn line(&self, text: String) {
        // println on MultiProgress keeps bars below the log lines
        let _ = self.multi.println(text);
    }

    pub fn finish(&self, summary: &Summary) {
        self.total.finish_and_clear();
        let mut parts = vec![format!("{} downloaded", style(summary.downloaded).green().bold())];
        if summary.existing > 0 {
            parts.push(format!("{} already existed", style(summary.existing).yellow()));
        }
        if summary.skipped > 0 {
            parts.push(format!("{} skipped", style(summary.skipped).blue()));
        }
        if summary.failed > 0 {
            parts.push(format!("{} failed", style(summary.failed).red().bold()));
        }
        eprintln!("{}", parts.join(", "));
    }
}

impl Observer for CliProgress {
    fn on_event(&self, event: Event) {
        match event {
            Event::Queued { .. } => self.total.inc_length(1),
            Event::Started { task, title, quality, .. } => {
                let bar = self.multi.insert_before(&self.total, ProgressBar::no_length());
                bar.set_style(spinner_style());
                bar.set_message(format!("{} {}", style(title).bold(), style(quality).cyan()));
                bar.enable_steady_tick(Duration::from_millis(100));
                self.tasks.lock().unwrap().insert(task, bar);
            }
            Event::Progress { task, downloaded, total } => {
                if let Some(bar) = self.tasks.lock().unwrap().get(&task) {
                    if let (Some(total), None) = (total, bar.length()) {
                        bar.set_length(total);
                        bar.set_style(bar_style());
                    }
                    bar.set_position(downloaded);
                }
            }
            Event::Finished { task, title, quality, path, outcome, .. } => {
                if let Some(bar) = self.tasks.lock().unwrap().remove(&task) {
                    bar.finish_and_clear();
                    self.multi.remove(&bar);
                }
                self.total.inc(1);
                let label = match outcome {
                    Outcome::Downloaded => style("Downloaded ").green(),
                    Outcome::Overwritten => style("Overwrote  ").cyan(),
                };
                self.line(format!(
                    "{label}{} {} {}",
                    style(title).bold(),
                    style(quality).cyan(),
                    style(self.display_path(&path)).dim()
                ));
            }
            Event::Exists { title, path, .. } => {
                self.total.inc(1);
                self.line(format!(
                    "{}{} {}",
                    style("Exists     ").yellow(),
                    title,
                    style(self.display_path(&path)).dim()
                ));
            }
            Event::Skipped { title, reason, .. } => {
                self.total.inc(1);
                self.line(format!("{}{} {}", style("Skipped    ").blue(), title, style(reason).dim()));
            }
            Event::Failed { context, message, .. } => {
                self.line(format!("{}{} {}", style("Error      ").red().bold(), message, style(context).dim()));
            }
            Event::ExtraWritten { kind, path } => {
                let kind = match kind {
                    ExtraFile::Cover => "Cover      ",
                    ExtraFile::Lyrics => "Lyrics     ",
                    ExtraFile::Playlist => "Playlist   ",
                };
                self.line(format!("{}{}", style(kind).magenta(), style(self.display_path(&path)).dim()));
            }
        }
    }
}
