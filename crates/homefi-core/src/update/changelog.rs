//! "What's new": the notes of the versions a user just updated to, read from the repo's
//! `CHANGELOG.md` embedded at build time (offline, identical on every platform).
//!
//! The apps only render what this module returns. Text only: no HTML, links limited to
//! `http(s)://`.

use serde::{Deserialize, Serialize};

use super::Version;
use crate::humane::Lang;

/// The repo's CHANGELOG.md, embedded at build time (offline, the same on every platform).
pub const CHANGELOG: &str = include_str!("../../../../CHANGELOG.md");
/// Most versions shown at once after a long jump.
pub const MAX_WHATS_NEW_VERSIONS: usize = 5;

/// Link schemes the apps may open.
const LINK_SCHEMES: [&str; 2] = ["https://", "http://"];
/// Length of the `yyyy-mm-dd` dates.
const ISO_DATE_LEN: usize = 10;

/// A run of text with one style.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct NoteSpan {
    pub text: String,
    pub bold: bool,
    pub code: bool,
    /// `http://` or `https://` only.
    pub link: Option<String>,
}

/// One paragraph or bullet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct NoteLine {
    pub spans: Vec<NoteSpan>,
}

/// `Added`, `Fixed`...: a title and its bullets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct NoteSection {
    /// Localized.
    pub title: String,
    pub items: Vec<NoteLine>,
}

/// The notes of one released version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ReleaseNotes {
    pub version: String,
    /// `yyyy-mm-dd` or empty.
    pub date: String,
    pub summary: Option<NoteLine>,
    pub sections: Vec<NoteSection>,
}

/// What to show now and what to remember.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct WhatsNewPlan {
    /// Newest first.
    pub notes: Vec<ReleaseNotes>,
    /// The running version: the app stores it right away, so notes show once per version.
    pub record_version: String,
}

/// Keep a Changelog titles in Spanish; anything else passes through.
fn section_title(title: &str, lang: Lang) -> String {
    let title = title.trim();
    if lang == Lang::En {
        return title.to_owned();
    }
    match title.to_ascii_lowercase().as_str() {
        "added" => "Nuevo",
        "changed" => "Cambios",
        "deprecated" => "Obsoleto",
        "removed" => "Quitado",
        "fixed" => "Arreglado",
        "security" => "Seguridad",
        _ => title,
    }
    .to_owned()
}

/// A version being read: plain strings until the version ends.
struct Draft {
    version: Version,
    notes: ReleaseNotes,
    summary: String,
    in_summary: bool,
    section: Option<(String, Vec<String>)>,
    item: Option<String>,
}

impl Draft {
    fn new(version: Version, date: String) -> Self {
        let notes = ReleaseNotes { version: version.to_string(), date, ..ReleaseNotes::default() };
        Self { version, notes, summary: String::new(), in_summary: true, section: None, item: None }
    }

    fn flush_item(&mut self) {
        if let (Some(item), Some((_, items))) = (self.item.take(), self.section.as_mut()) {
            items.push(item);
        }
    }

    fn close_section(&mut self) {
        self.flush_item();
        if let Some((title, items)) = self.section.take() {
            if !items.is_empty() {
                let items = items.iter().map(|t| NoteLine { spans: inline_spans(t) }).collect();
                self.notes.sections.push(NoteSection { title, items });
            }
        }
    }

    fn finish(mut self) -> (Version, ReleaseNotes) {
        self.close_section();
        if !self.summary.is_empty() {
            self.notes.summary = Some(NoteLine { spans: inline_spans(&self.summary) });
        }
        (self.version, self.notes)
    }

    fn line(&mut self, raw: &str, lang: Lang) {
        let text = raw.trim();
        if let Some(title) = text.strip_prefix("### ") {
            self.close_section();
            self.in_summary = false;
            self.section = Some((section_title(title, lang), Vec::new()));
            return;
        }
        if text.starts_with('#') || text == "---" {
            self.flush_item();
            return;
        }
        if text.is_empty() {
            self.flush_item();
            return;
        }
        if self.in_summary {
            if !self.summary.is_empty() {
                self.summary.push(' ');
            }
            self.summary.push_str(text);
            return;
        }
        if self.section.is_none() {
            return;
        }
        let bullet = text.strip_prefix("- ").or_else(|| text.strip_prefix("* "));
        match (bullet, self.item.as_mut()) {
            (Some(body), _) => {
                self.flush_item();
                self.item = Some(body.trim().to_owned());
            }
            (None, Some(item)) => {
                item.push(' ');
                item.push_str(text);
            }
            (None, None) => self.item = Some(text.to_owned()),
        }
    }
}

/// `## [1.5.1] - 2026-11-02` → the version and its date; `None` for anything else.
fn version_heading(heading: &str) -> Option<(Version, String)> {
    let rest = heading.trim().strip_prefix('[')?;
    let (version, after) = rest.split_once(']')?;
    let version = version.parse::<Version>().ok()?;
    let date = after.trim().strip_prefix('-').map(str::trim).filter(|d| is_iso_date(d)).unwrap_or_default();
    Some((version, date.to_owned()))
}

fn is_iso_date(s: &str) -> bool {
    s.len() == ISO_DATE_LEN
        && s.bytes().enumerate().all(|(i, b)| if i == 4 || i == 7 { b == b'-' } else { b.is_ascii_digit() })
}

/// Released versions with their parsed version, file order.
fn released(markdown: &str, lang: Lang) -> Vec<(Version, ReleaseNotes)> {
    let mut out = Vec::new();
    let mut draft: Option<Draft> = None;
    for raw in markdown.lines() {
        if let Some(heading) = raw.strip_prefix("## ") {
            out.extend(draft.take().map(Draft::finish));
            draft = version_heading(heading).map(|(v, date)| Draft::new(v, date));
        } else if let Some(d) = draft.as_mut() {
            d.line(raw, lang);
        }
    }
    out.extend(draft.map(Draft::finish));
    out
}

/// Released versions in file order (newest first); `[Unreleased]` and headings that aren't a
/// version are skipped.
pub fn parse_changelog(markdown: &str, lang: Lang) -> Vec<ReleaseNotes> {
    released(markdown, lang).into_iter().map(|(_, notes)| notes).collect()
}

/// Pushes the plain text gathered so far as one span.
fn flush_plain(plain: &mut String, spans: &mut Vec<NoteSpan>) {
    if !plain.is_empty() {
        spans.push(NoteSpan { text: std::mem::take(plain), ..NoteSpan::default() });
    }
}

/// `[text](url)` at the start of `rest` → (text, url, bytes used).
fn markdown_link(rest: &str) -> Option<(&str, &str, usize)> {
    let inner = rest.strip_prefix('[')?;
    let (text, after) = inner.split_once("](")?;
    let (url, _) = after.split_once(')')?;
    (!text.is_empty()).then_some((text, url, 1 + text.len() + 2 + url.len() + 1))
}

/// `**bold**`, `` `code` ``, `[text](url)`; anything else is plain text.
pub fn inline_spans(text: &str) -> Vec<NoteSpan> {
    let mut spans = Vec::new();
    let mut plain = String::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        let mut used = 0;
        if let Some(after) = rest.strip_prefix("**") {
            if let Some(end) = after.find("**").filter(|&e| e > 0) {
                flush_plain(&mut plain, &mut spans);
                spans.push(NoteSpan { text: after[..end].to_owned(), bold: true, ..NoteSpan::default() });
                used = 2 + end + 2;
            }
        } else if let Some(after) = rest.strip_prefix('`') {
            if let Some(end) = after.find('`').filter(|&e| e > 0) {
                flush_plain(&mut plain, &mut spans);
                spans.push(NoteSpan { text: after[..end].to_owned(), code: true, ..NoteSpan::default() });
                used = 1 + end + 1;
            }
        } else if rest.starts_with('[') {
            if let Some((label, url, len)) = markdown_link(rest) {
                let url = url.trim();
                if LINK_SCHEMES.iter().any(|s| url.starts_with(s)) {
                    flush_plain(&mut plain, &mut spans);
                    spans.push(NoteSpan { text: label.to_owned(), link: Some(url.to_owned()), ..NoteSpan::default() });
                } else {
                    plain.push_str(label);
                }
                used = len;
            }
        }
        if used == 0 {
            let ch = rest.chars().next().unwrap_or_default();
            plain.push(ch);
            used = ch.len_utf8().max(1);
        }
        i += used;
    }
    flush_plain(&mut plain, &mut spans);
    spans
}

/// Testable core of [`whats_new`] / [`latest_notes`] with any changelog and version.
pub(crate) fn plan_for(
    changelog: &str,
    current: &str,
    last_seen: Option<&str>,
    upgraded: bool,
    enabled: bool,
    lang: Lang,
) -> WhatsNewPlan {
    let mut plan = WhatsNewPlan { notes: Vec::new(), record_version: current.to_owned() };
    let Ok(current) = current.parse::<Version>() else { return plan };
    if !enabled {
        return plan;
    }
    // An unparsable last-seen version counts as "updated from something unknown".
    let seen = last_seen.and_then(|s| s.parse::<Version>().ok());
    let upgraded = upgraded || (last_seen.is_some() && seen.is_none());
    let mut candidates: Vec<(Version, ReleaseNotes)> =
        released(changelog, lang).into_iter().filter(|(v, _)| *v <= current).collect();
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    let limit = match &seen {
        Some(seen) if *seen >= current => return plan,
        Some(_) => MAX_WHATS_NEW_VERSIONS,
        None if upgraded => 1,
        None => return plan,
    };
    plan.notes = candidates
        .into_iter()
        .filter(|(v, _)| seen.as_ref().is_none_or(|s| v > s))
        .take(limit)
        .map(|(_, notes)| notes)
        .collect();
    plan
}

/// What to show now. Uses [`CHANGELOG`] and the running version.
pub fn whats_new(last_seen: Option<&str>, upgraded: bool, enabled: bool, lang: Lang) -> WhatsNewPlan {
    plan_for(CHANGELOG, crate::VERSION, last_seen, upgraded, enabled, lang)
}

/// The notes of the newest version ≤ the running one (Settings → About → What's new).
pub fn latest_notes(lang: Lang) -> Option<ReleaseNotes> {
    plan_for(CHANGELOG, crate::VERSION, None, true, true, lang).notes.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# Changelog\n\nIntro.\n\n## [Unreleased]\n\n### Added\n- Not yet\n\n\
## [1.5.2] - 2026-11-03\n\n\nOne **big** thing.\nSecond line.\n\n### Added\n- First item that\n  wraps here.\n\
  - Nested bullet\n* Star bullet\n\nA plain paragraph.\n\n### Fixed\n- Bug `fix`\n\n### Weird\n- Odd\n\n### Empty\n\n\
## [not-a-version]\n\n### Added\n- Skipped\n\n## [1.5.1]\n\n### Changed\n- Only change\n\n---\nNotice\n\n\
## [1.5.0-beta.1] - soon\n\n## [1.4.0] - 2026-10-30\n\n### Removed\n- Gone\n";

    fn text(line: &NoteLine) -> String {
        line.spans.iter().map(|s| s.text.as_str()).collect()
    }

    #[test]
    fn parses_versions_dates_and_skips_the_rest() {
        let notes = parse_changelog(SAMPLE, Lang::En);
        let versions: Vec<_> = notes.iter().map(|n| n.version.as_str()).collect();
        assert_eq!(versions, ["1.5.2", "1.5.1", "1.5.0-beta.1", "1.4.0"]);
        assert_eq!(notes[0].date, "2026-11-03");
        assert_eq!(notes[1].date, "");
        assert_eq!(notes[2].date, "");
    }

    #[test]
    fn reads_the_summary_after_double_blank_lines() {
        let notes = parse_changelog(SAMPLE, Lang::En);
        let summary = notes[0].summary.as_ref().unwrap();
        assert_eq!(text(summary), "One big thing. Second line.");
        assert!(summary.spans[1].bold);
        assert!(notes[1].summary.is_none());
    }

    #[test]
    fn reads_wrapped_nested_star_and_plain_items() {
        let notes = parse_changelog(SAMPLE, Lang::En);
        let added = &notes[0].sections[0];
        assert_eq!(added.title, "Added");
        let items: Vec<_> = added.items.iter().map(text).collect();
        assert_eq!(items, ["First item that wraps here.", "Nested bullet", "Star bullet", "A plain paragraph."]);
        assert_eq!(notes[0].sections[1].items[0].spans[1].text, "fix");
        assert!(notes[0].sections[1].items[0].spans[1].code);
    }

    #[test]
    fn unknown_titles_pass_through_and_empty_sections_drop() {
        let notes = parse_changelog(SAMPLE, Lang::Es);
        let titles: Vec<_> = notes[0].sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, ["Nuevo", "Arreglado", "Weird"]);
        assert_eq!(notes[1].sections[0].title, "Cambios");
        assert_eq!(notes[3].sections[0].title, "Quitado");
    }

    #[test]
    fn rules_and_notices_are_not_items() {
        let notes = parse_changelog(SAMPLE, Lang::En);
        // "Notice" follows `---` after a blank line, so it is a paragraph item, `---` is not.
        let items: Vec<_> = notes[1].sections[0].items.iter().map(text).collect();
        assert_eq!(items, ["Only change", "Notice"]);
    }

    #[test]
    fn crlf_is_fine() {
        let crlf = SAMPLE.replace('\n', "\r\n");
        assert_eq!(parse_changelog(&crlf, Lang::En), parse_changelog(SAMPLE, Lang::En));
    }

    #[test]
    fn spans_bold_code_and_links() {
        let s = inline_spans("a **b** `c` [d](https://x.io/y) e");
        let kinds: Vec<_> = s.iter().map(|s| (s.text.as_str(), s.bold, s.code, s.link.as_deref())).collect();
        assert_eq!(
            kinds,
            [
                ("a ", false, false, None),
                ("b", true, false, None),
                (" ", false, false, None),
                ("c", false, true, None),
                (" ", false, false, None),
                ("d", false, false, Some("https://x.io/y")),
                (" e", false, false, None),
            ]
        );
    }

    #[test]
    fn unsafe_or_relative_links_become_plain_text() {
        assert_eq!(
            inline_spans("see [doc](docs/x.md) ok"),
            vec![NoteSpan { text: "see doc ok".into(), ..Default::default() }]
        );
        let s = inline_spans("[x](javascript:alert(1))");
        assert!(s.iter().all(|s| s.link.is_none()));
        assert_eq!(s[0].text.chars().next(), Some('x'));
    }

    #[test]
    fn unclosed_markers_and_code_inside_bold_stay_text() {
        assert_eq!(inline_spans("a **b").len(), 1);
        assert_eq!(inline_spans("a **b").first().unwrap().text, "a **b");
        let s = inline_spans("**a `b` c**");
        assert_eq!(s.len(), 1);
        assert!(s[0].bold && !s[0].code);
        assert_eq!(s[0].text, "a `b` c");
        assert_eq!(inline_spans("[open")[0].text, "[open");
        assert!(inline_spans("").is_empty());
    }

    #[test]
    fn utf8_never_panics() {
        let s = inline_spans("café 🎧 **ñandú 🎶** `✓` [é](https://a.b/é) 🎧[ **");
        assert!(s.iter().any(|s| s.bold && s.text == "ñandú 🎶"));
        assert!(s.iter().any(|s| s.code && s.text == "✓"));
        assert_eq!(s[0].text, "café 🎧 ");
    }

    fn plan(current: &str, last: Option<&str>, upgraded: bool, enabled: bool) -> Vec<String> {
        plan_for(SAMPLE, current, last, upgraded, enabled, Lang::En).notes.into_iter().map(|n| n.version).collect()
    }

    #[test]
    fn plan_table() {
        // fresh install
        assert!(plan("1.5.2", None, false, true).is_empty());
        // upgrade from unknown: newest only
        assert_eq!(plan("1.5.2", None, true, true), ["1.5.2"]);
        assert_eq!(plan("1.5.1", None, true, true), ["1.5.1"]);
        // garbage last seen
        assert_eq!(plan("1.5.2", Some("abc"), false, true), ["1.5.2"]);
        // same, downgrade
        assert!(plan("1.5.1", Some("1.5.1"), true, true).is_empty());
        assert!(plan("1.5.1", Some("1.5.2"), true, true).is_empty());
        // several versions, newest first, prerelease between
        assert_eq!(plan("1.5.2", Some("1.4.0"), true, true), ["1.5.2", "1.5.1", "1.5.0-beta.1"]);
        // disabled still records
        let off = plan_for(SAMPLE, "1.5.2", Some("1.4.0"), true, false, Lang::En);
        assert!(off.notes.is_empty());
        assert_eq!(off.record_version, "1.5.2");
        // prerelease current hides its own stable version
        assert_eq!(plan("1.5.2-beta.1", Some("1.4.0"), true, true), ["1.5.1", "1.5.0-beta.1"]);
        // dev build with no entry in range
        assert!(plan("1.0.0", Some("0.9.0"), true, true).is_empty());
    }

    #[test]
    fn plan_is_capped() {
        let mut md = String::new();
        for minor in 1..=8 {
            md.push_str(&format!("## [1.{minor}.0] - 2026-01-0{minor}\n\n### Added\n- x\n\n"));
        }
        let plan = plan_for(&md, "1.8.0", Some("1.0.0"), true, true, Lang::En);
        let versions: Vec<_> = plan.notes.iter().map(|n| n.version.as_str()).collect();
        assert_eq!(versions, ["1.8.0", "1.7.0", "1.6.0", "1.5.0", "1.4.0"]);
        assert_eq!(plan.notes.len(), MAX_WHATS_NEW_VERSIONS);
    }

    #[test]
    fn the_real_changelog_parses() {
        let headings = CHANGELOG
            .lines()
            .filter(|l| {
                l.strip_prefix("## [")
                    .and_then(|r| r.split_once(']'))
                    .is_some_and(|(v, _)| v.parse::<Version>().is_ok())
            })
            .count();
        let notes = parse_changelog(CHANGELOG, Lang::En);
        assert!(headings > 0);
        assert_eq!(notes.len(), headings);
        for n in &notes {
            assert!(n.summary.is_some() || n.sections.iter().any(|s| !s.items.is_empty()), "{} is empty", n.version);
        }
        let latest = latest_notes(Lang::En).expect("a released version <= the running one");
        assert!(latest.version.parse::<Version>().unwrap() <= crate::VERSION.parse::<Version>().unwrap());
    }
}
