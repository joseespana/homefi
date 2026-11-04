//! The recent shows of an artist, found by the user's own AI with its
//! provider's web search. Two phases: research (searching), then a plain call
//! that turns the notes into structured shows. Only what is on a page counts.

use std::collections::HashSet;

use chrono::{Duration as ChronoDuration, NaiveDate};
use serde::Deserialize;
use serde_json::json;

use super::{ConcertShow, ConcertSong, MIN_SONGS_PER_SHOW};
use crate::ai::{
    chat, chat_with, resolve_model, AiConfig, AiError, AiErrorKind, ChatOptions, Message, Tool, WebSearch,
};
use crate::playlist::{SetlistSource, SourceLink, MAX_SOURCES};
use crate::text::fold;

const RESEARCH_SYSTEM: &str = "You are a careful music researcher. Use web search. Report only what the pages say; \
                               never guess songs or dates.";
const STRUCTURE_SYSTEM: &str = "Turn the research notes into one `submit_shows` call. Copy only shows and songs that \
                                are in the notes, in order. Dates as YYYY-MM-DD. Leave out shows without a song list.";
const SUBMIT_TOOL: &str = "submit_shows";
const DATE_FORMAT: &str = "%Y-%m-%d";
/// A show dated tomorrow can be a time zone away; later ones are made up.
const FUTURE_SLACK_DAYS: i64 = 1;

/// What the AI found.
pub(crate) struct ShowsFound {
    pub shows: Vec<ConcertShow>,
    pub sources: Vec<SourceLink>,
    pub search_widget_html: Option<String>,
    pub source: SetlistSource,
}

fn research_prompt(artist: &str, shows: u8) -> String {
    format!(
        "Find the setlists of the {shows} most recent concerts by “{artist}” (the current tour, the last 12 months). \
         For each show give the date (YYYY-MM-DD), city, country, venue, tour name, every song in the order played, \
         which songs were the encore, which were recorded intros or tapes, which were covers and of whom, and the \
         URL of the page you read it on. Only report shows you found on a page."
    )
}

fn submit_tool() -> Tool {
    Tool {
        name: SUBMIT_TOOL.into(),
        description: "Submit the shows found in the research notes.".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "artist": { "type": "string" },
                "shows": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "date": { "type": "string", "description": "YYYY-MM-DD" },
                            "venue": { "type": "string" },
                            "city": { "type": "string" },
                            "country": { "type": "string" },
                            "tour": { "type": "string" },
                            "url": { "type": "string", "description": "The page it was read from" },
                            "songs": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "name": { "type": "string" },
                                        "original_artist": { "type": "string", "description": "Only for covers" },
                                        "tape": { "type": "boolean", "description": "A recorded intro or outro" },
                                        "encore": { "type": "boolean" }
                                    },
                                    "required": ["name"]
                                }
                            }
                        },
                        "required": ["date", "songs"]
                    }
                }
            },
            "required": ["shows"]
        }),
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Submitted {
    shows: Vec<ConcertShow>,
}

/// Looks up the last `shows` concerts of `artist`. `search_budget` is the
/// number of web searches the model may run.
pub(crate) async fn recent_shows(
    config: &AiConfig,
    artist: &str,
    shows: u8,
    search_budget: u8,
) -> crate::Result<ShowsFound> {
    let mut config = config.clone();
    if config.model.trim().is_empty() {
        config.model = resolve_model(&config).await?;
    }
    let research = chat_with(
        &config,
        RESEARCH_SYSTEM,
        &[Message::User { text: research_prompt(artist, shows) }],
        &[],
        &ChatOptions { web_search: Some(WebSearch { max_uses: search_budget }) },
    )
    .await?;

    let tool = submit_tool();
    let structured =
        chat(&config, STRUCTURE_SYSTEM, &[Message::User { text: research.text }], std::slice::from_ref(&tool)).await?;
    let call = structured
        .tool_calls
        .into_iter()
        .find(|c| c.name == SUBMIT_TOOL)
        .ok_or_else(|| AiError::new(&config, AiErrorKind::BadAnswer, "no submit_shows"))?;
    let submitted: Submitted = serde_json::from_value(call.arguments).unwrap_or_default();

    let today = chrono::Utc::now().date_naive();
    Ok(ShowsFound {
        shows: usable_shows(submitted.shows, usize::from(shows), today),
        sources: source_links(research.sources.iter().map(|s| (s.url.as_str(), s.title.as_str()))),
        search_widget_html: research.search_widget_html.filter(|h| !h.trim().is_empty()),
        source: SetlistSource::Ai { provider_id: config.provider.id().to_owned(), model: config.model.clone() },
    })
}

/// Pages cited, deduplicated by URL, at most [`MAX_SOURCES`].
fn source_links<'a>(pages: impl Iterator<Item = (&'a str, &'a str)>) -> Vec<SourceLink> {
    let mut seen = HashSet::new();
    pages
        .filter(|(url, _)| !url.trim().is_empty() && seen.insert(*url))
        .take(MAX_SOURCES)
        .map(|(url, title)| SourceLink { url: url.to_owned(), title: title.to_owned() })
        .collect()
}

/// Keeps real shows: a valid date that isn't in the future, enough songs that
/// aren't tapes, one per (date, venue). Most recent first, at most `wanted`,
/// with ids `"{date}-{i}"`.
pub(crate) fn usable_shows(shows: Vec<ConcertShow>, wanted: usize, today: NaiveDate) -> Vec<ConcertShow> {
    let latest = today + ChronoDuration::days(FUTURE_SLACK_DAYS);
    let mut seen = HashSet::new();
    let mut kept: Vec<(NaiveDate, ConcertShow)> = shows
        .into_iter()
        .filter_map(|show| {
            let date = NaiveDate::parse_from_str(show.date.trim(), DATE_FORMAT).ok()?;
            let real_songs = show.songs.iter().filter(|s| !s.tape && !s.name.trim().is_empty()).count();
            (date <= latest && real_songs >= MIN_SONGS_PER_SHOW && seen.insert((date, fold(&show.venue))))
                .then_some((date, show))
        })
        .collect();
    kept.sort_by_key(|(date, _)| std::cmp::Reverse(*date));
    kept.into_iter()
        .take(wanted)
        .enumerate()
        .map(|(i, (date, mut show))| {
            show.id = format!("{}-{i}", date.format(DATE_FORMAT));
            show.date = date.format(DATE_FORMAT).to_string();
            show.url = show.url.take().filter(|u| !u.trim().is_empty());
            show.songs = show.songs.into_iter().filter(|s| !s.name.trim().is_empty()).map(clean_song).collect();
            show
        })
        .collect()
}

fn clean_song(mut song: ConcertSong) -> ConcertSong {
    song.original_artist = song.original_artist.take().map(|a| a.trim().to_owned()).filter(|a| !a.is_empty());
    song
}

#[cfg(test)]
mod tests {
    use super::*;

    fn show(date: &str, venue: &str, songs: usize, tapes: usize) -> ConcertShow {
        let mut list: Vec<ConcertSong> =
            (0..songs).map(|i| ConcertSong { name: format!("Song {i}"), ..ConcertSong::default() }).collect();
        list.extend((0..tapes).map(|i| ConcertSong {
            name: format!("Tape {i}"),
            tape: true,
            ..ConcertSong::default()
        }));
        ConcertShow { date: date.into(), venue: venue.into(), songs: list, ..ConcertShow::default() }
    }

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 2).unwrap()
    }

    #[test]
    fn drops_bad_dates_futures_short_and_duplicates() {
        let found = usable_shows(
            vec![
                show("2026-10-24", "Arena", 6, 0),
                show("2026-10-24", "ARENA!", 7, 0),
                show("2026-12-01", "Future", 6, 0),
                show("not a date", "Nowhere", 6, 0),
                show("2026-10-20", "Small", 3, 0),
                show("2026-10-19", "Tapes", 3, 4),
                show("2026-10-30", "Other", 5, 1),
                show("2026-11-03", "Tomorrow", 5, 0),
            ],
            8,
            today(),
        );
        let dates: Vec<_> = found.iter().map(|s| s.date.as_str()).collect();
        assert_eq!(dates, ["2026-11-03", "2026-10-30", "2026-10-24"]);
        let ids: Vec<_> = found.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["2026-11-03-0", "2026-10-30-1", "2026-10-24-2"]);
    }

    #[test]
    fn keeps_only_the_wanted_most_recent() {
        let shows = (1..=6).map(|d| show(&format!("2026-10-{d:02}"), &format!("V{d}"), 5, 0)).collect();
        let found = usable_shows(shows, 3, today());
        let dates: Vec<_> = found.iter().map(|s| s.date.as_str()).collect();
        assert_eq!(dates, ["2026-10-06", "2026-10-05", "2026-10-04"]);
    }

    #[test]
    fn sources_are_deduplicated_and_capped() {
        let pages: Vec<(String, String)> =
            (0..30).map(|i| (format!("https://e.example.com/{}", i % 25), "t".into())).collect();
        let links = source_links(pages.iter().map(|(u, t)| (u.as_str(), t.as_str())));
        assert_eq!(links.len(), MAX_SOURCES);
        assert_eq!(source_links([("", "x")].into_iter()).len(), 0);
    }
}
