//! The user's own AI (BYOK) in Exercise mode: a short chat that turns "a 30-minute easy ride with 90s rock"
//! into a request (`set_workout`), and the one call of a build that proposes candidate songs (title and
//! artist, never ids) for each segment. The model never has the last word: every song is resolved against
//! the library and Tidal and kept only if its BPM fits.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::bpm::{bpm_windows, Tolerance};
use super::model::{
    round_minutes, Effort, Interval, MusicWish, Position, PresetId, RideRequest, RpmRange, Segment, SegmentKind,
    Target, Workout, WorkoutOrigin, MAX_NAME_CHARS, MAX_RIDE_MINUTES, MIN_RIDE_MINUTES,
};
use super::words;
use crate::ai::replies::{quick_replies, suggest_replies_tool, SUGGEST_REPLIES};
use crate::ai::{chat, resolve_model, AiConfig, AiError, AiErrorKind, Message, Tool, ToolResult};
use crate::humane::Lang;
use crate::playlist::{new_id, SongCandidate};
use crate::text::song_key;
use crate::Result;

pub(crate) const SET_WORKOUT: &str = "set_workout";
pub(crate) const PROPOSE_RIDE_SONGS: &str = "propose_ride_songs";
/// Model calls per chat message (tool rounds included).
const MAX_ROUNDS_PER_TURN: usize = 3;
/// Messages the rider may send in one conversation.
const MAX_CHAT_TURNS: u8 = 8;
/// Songs asked of the AI for each segment: some are not found, some do not fit.
pub const SONGS_PER_SEGMENT: usize = 5;
/// Longest title or artist of a proposed song (chars).
const MAX_SONG_FIELD_CHARS: usize = 120;
/// Shortest segment the AI may describe (s) and cadence words it may use.
const POSITION_WORDS: [&str; 3] = ["seated", "standing", "jumps"];

/// What the AI says in the chat, and the request as it now understands it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ExerciseTurn {
    pub text: String,
    pub quick_replies: Vec<String>,
    pub request: RideRequest,
    /// The AI's short name for the ride, empty until it sets one.
    pub title: String,
}

/// One conversation. Turns never overlap (the caller serializes them).
pub struct ExerciseSession {
    config: AiConfig,
    lang: Lang,
    request: RideRequest,
    title: String,
    /// The rider's first message: the workout's `wish`.
    first_words: String,
    messages: Vec<Message>,
    user_turns: u8,
}

impl ExerciseSession {
    pub fn new(config: AiConfig, request: RideRequest, lang: Lang) -> Self {
        Self {
            config,
            lang,
            request: clean_request(request),
            title: String::new(),
            first_words: String::new(),
            messages: Vec::new(),
            user_turns: 0,
        }
    }

    pub fn request(&self) -> &RideRequest {
        &self.request
    }

    /// HomeFi's fixed opener and example answers: no AI call.
    pub fn start(&mut self) -> ExerciseTurn {
        self.messages.clear();
        self.user_turns = 0;
        self.first_words.clear();
        ExerciseTurn {
            text: words::opener_text(self.lang),
            quick_replies: words::opener_replies(self.lang),
            request: self.request.clone(),
            title: self.title.clone(),
        }
    }

    /// The rider's answer. After [`MAX_CHAT_TURNS`] the turn is fixed words and no AI is called. On a failed
    /// call the history and the request go back to what they were.
    pub async fn reply(&mut self, text: &str) -> Result<ExerciseTurn> {
        if self.user_turns >= MAX_CHAT_TURNS {
            return Ok(self.turn(words::chat_limit(self.lang), Vec::new()));
        }
        let (messages, turns, request, title, first) =
            (self.messages.len(), self.user_turns, self.request.clone(), self.title.clone(), self.first_words.clone());
        let text = text.trim();
        if self.first_words.is_empty() {
            self.first_words = text.chars().take(super::model::MAX_SENTENCE_CHARS).collect();
        }
        self.messages.push(Message::User { text: text.to_owned() });
        self.user_turns += 1;
        let outcome = self.rounds().await;
        if outcome.is_err() {
            self.messages.truncate(messages);
            (self.user_turns, self.request, self.title, self.first_words) = (turns, request, title, first);
        }
        outcome
    }

    fn turn(&self, text: String, quick_replies: Vec<String>) -> ExerciseTurn {
        ExerciseTurn { text, quick_replies, request: self.request.clone(), title: self.title.clone() }
    }

    async fn rounds(&mut self) -> Result<ExerciseTurn> {
        if self.config.model.trim().is_empty() {
            self.config.model = resolve_model(&self.config).await?;
        }
        let tools = [suggest_replies_tool(), set_workout_tool()];
        let system = chat_system_prompt(&self.request, self.lang);
        let (mut texts, mut replies) = (Vec::new(), Vec::new());
        for _ in 0..MAX_ROUNDS_PER_TURN {
            let reply = chat(&self.config, &system, &self.messages, &tools).await?;
            if !reply.text.is_empty() {
                texts.push(reply.text.clone());
            }
            let calls = reply.tool_calls.clone();
            self.messages.push(reply.into_message());
            if calls.is_empty() {
                break;
            }
            let mut unknown = false;
            let results: Vec<ToolResult> = calls
                .iter()
                .map(|call| {
                    let content = match call.name.as_str() {
                        SUGGEST_REPLIES => {
                            replies = quick_replies(call.arguments.get("options"));
                            Some(json!({ "shown": replies.len() }).to_string())
                        }
                        SET_WORKOUT => {
                            self.apply(&call.arguments);
                            Some(json!({ "ok": true }).to_string())
                        }
                        _ => None,
                    };
                    unknown |= content.is_none();
                    ToolResult {
                        call_id: call.id.clone(),
                        name: call.name.clone(),
                        is_error: content.is_none(),
                        content: content.unwrap_or_else(|| format!("unknown tool {}", call.name)),
                    }
                })
                .collect();
            self.messages.push(Message::Tool { results });
            if !unknown && !texts.is_empty() {
                break;
            }
        }
        let text = if texts.is_empty() { words::empty_turn(self.lang) } else { texts.join("\n\n") };
        Ok(self.turn(text, replies))
    }

    /// Merges one `set_workout` call: malformed fields are ignored, present ones replace.
    fn apply(&mut self, arguments: &Value) {
        let provider_id = self.config.provider.id().to_owned();
        let model = self.config.model.clone();
        apply_set_workout(&mut self.request, &mut self.title, &self.first_words, (&provider_id, &model), arguments);
    }
}

fn clean_request(mut request: RideRequest) -> RideRequest {
    request.minutes = round_minutes(request.minutes);
    request.wish = request.wish.clamped();
    request
}

/// The request after one `set_workout` call (the session's logic, testable without a model).
pub(crate) fn apply_set_workout(
    request: &mut RideRequest,
    title: &mut String,
    wish_words: &str,
    ai: (&str, &str),
    arguments: &Value,
) {
    if let Some(t) = arguments.get("title").and_then(Value::as_str).map(str::trim).filter(|t| !t.is_empty()) {
        *title = t.chars().take(MAX_NAME_CHARS).collect();
    }
    let mut shape_changed = false;
    if let Some(m) = arguments.get("minutes").and_then(Value::as_f64).filter(|m| m.is_finite()) {
        let minutes = m.round().clamp(0.0, f64::from(u16::MAX)) as u16;
        request.minutes = round_minutes(minutes.clamp(MIN_RIDE_MINUTES, MAX_RIDE_MINUTES));
        shape_changed = true;
    }
    if let Some(style) = arguments.get("style").and_then(Value::as_str) {
        if let Ok(id) = serde_json::from_value::<PresetId>(Value::String(style.trim().to_lowercase())) {
            request.style = id;
            shape_changed = true;
        }
    }
    // a new length or style means the preset is built again; explicit segments replace everything
    if shape_changed {
        request.workout = None;
    }
    if let Some(list) = arguments.get("segments").and_then(Value::as_array) {
        let segments: Vec<Segment> = list.iter().filter_map(segment_from).collect();
        if !segments.is_empty() {
            request.workout = Some(
                Workout {
                    id: new_id(),
                    name: title.clone(),
                    segments,
                    origin: WorkoutOrigin::Ai {
                        wish: wish_words.to_owned(),
                        provider_id: ai.0.to_owned(),
                        model: ai.1.to_owned(),
                    },
                    ..Workout::default()
                }
                .sanitized(),
            );
        }
    }
    let mut wish = request.wish.clone();
    if let Some(genres) = arguments.get("genres").and_then(Value::as_array) {
        wish.genres = genres.iter().filter_map(|g| g.as_str().map(str::to_owned)).collect();
    }
    if let Some(mix) = arguments.get("mix").and_then(Value::as_bool) {
        wish.mix = mix;
    }
    if let Some(sentence) = arguments.get("sentence").and_then(Value::as_str) {
        wish.sentence = sentence.to_owned();
    }
    request.wish = wish.clamped();
}

fn u_field(v: &Value, key: &str) -> Option<u32> {
    v.get(key).and_then(Value::as_f64).filter(|x| x.is_finite() && *x >= 0.0).map(|x| x.round() as u32)
}

/// A segment from the AI's plain fields; `None` when it has neither a length nor a kind.
fn segment_from(v: &Value) -> Option<Segment> {
    let kind =
        serde_json::from_value::<SegmentKind>(Value::String(v.get("kind")?.as_str()?.trim().to_lowercase())).ok()?;
    let seconds = u_field(v, "seconds").or_else(|| u_field(v, "minutes").map(|m| m * 60))?;
    let rpm = |key: &str, default: u16| u_field(v, key).map_or(default, |r| r.min(u32::from(u16::MAX)) as u16);
    let range = RpmRange::new(rpm("min_rpm", 80), rpm("max_rpm", 95));
    let position = match v.get("position").and_then(Value::as_str).map(str::to_lowercase).as_deref() {
        Some("standing") => Position::Standing,
        Some("jumps") => Position::jumps(),
        _ => Position::Seated,
    };
    let interval = v.get("sprint").filter(|s| s.is_object()).map(|s| Interval {
        work_s: u_field(s, "work_s").unwrap_or(20),
        rest_s: u_field(s, "rest_s").unwrap_or(60),
        rounds: u_field(s, "rounds").map_or(4, |r| r.min(u32::from(u8::MAX)) as u8),
        rest_cadence: RpmRange::new(
            u_field(s, "rest_min_rpm").map_or(80, |r| r.min(u32::from(u16::MAX)) as u16),
            u_field(s, "rest_max_rpm").map_or(90, |r| r.min(u32::from(u16::MAX)) as u16),
        ),
    });
    Some(Segment {
        kind,
        seconds,
        target: Target::from(range),
        effort: Effort(u_field(v, "effort").map_or(5, |e| e.min(u32::from(u8::MAX)) as u8)),
        position,
        interval,
        note: String::new(),
    })
}

fn set_workout_tool() -> Tool {
    let kinds: Vec<&str> =
        ["warm_up", "seated_flat", "seated_climb", "standing_climb", "jumps", "sprint", "recovery", "cool_down"]
            .to_vec();
    Tool {
        name: SET_WORKOUT.into(),
        description: "Set what you understood of the ride. Pass only what the rider said or you decided; \
                      leave out `segments` to let HomeFi build the style at the length."
            .into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "title": { "type": "string", "description": "A short name for the ride, in the rider's language" },
                "minutes": { "type": "integer", "minimum": MIN_RIDE_MINUTES, "maximum": MAX_RIDE_MINUTES },
                "style": { "type": "string", "enum": ["first_ride", "easy", "classic", "intervals"] },
                "genres": { "type": "array", "items": { "type": "string" } },
                "mix": { "type": "boolean", "description": "Several genres spread over the ride by tempo" },
                "sentence": { "type": "string", "description": "Anything else about the music, in the rider's words" },
                "segments": { "type": "array", "items": { "type": "object", "properties": {
                    "kind": { "type": "string", "enum": kinds },
                    "minutes": { "type": "number" },
                    "min_rpm": { "type": "integer" }, "max_rpm": { "type": "integer" },
                    "effort": { "type": "integer", "minimum": 1, "maximum": 10 },
                    "position": { "type": "string", "enum": POSITION_WORDS },
                    "sprint": { "type": "object", "properties": {
                        "work_s": { "type": "integer" }, "rest_s": { "type": "integer" }, "rounds": { "type": "integer" },
                        "rest_min_rpm": { "type": "integer" }, "rest_max_rpm": { "type": "integer" } } }
                }, "required": ["kind", "minutes"] } }
            }
        }),
    }
}

fn language_rule(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Answer in English.",
        Lang::Es => "Answer in Spanish.",
    }
}

fn chat_system_prompt(request: &RideRequest, lang: Lang) -> String {
    let wish = &request.wish;
    let genres = if wish.genres.is_empty() { "none yet".to_owned() } else { wish.genres.join(", ") };
    format!(
        "You are the coach of an indoor cycling (spin) app. The rider describes the ride they want: how long \
         ({MIN_RIDE_MINUTES} to {MAX_RIDE_MINUTES} minutes), how hard, and the music (genres, bands or artists, a favourite song, a vibe). What you know \
         so far: {} minutes, style {:?}, genres: {genres}. Reply in one to three short sentences: say what you \
         understood and ask ONE question if something important is missing (length, how hard, genres). Call \
         {SUGGEST_REPLIES} with 2 to 4 short answers every time. Whenever you learn something, call {SET_WORKOUT} \
         with it. Cadence is in RPM: warm-up 80-95, flat 85-100, seated climb 60-80, standing climb 55-70, sprints \
         100-120 work and 80-90 rest. Be kind to beginners. Never list songs in the chat: HomeFi picks them later \
         from the genres. Always write a reply, never only call tools. {}",
        request.minutes,
        request.style,
        language_rule(lang)
    )
}

// ------------------------------------------------------------------- the songs

/// A song proposed for a segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Proposal {
    pub segment: usize,
    /// Its place in the segment's list: the AI lists the best-known songs first.
    pub rank: usize,
    pub candidate: SongCandidate,
}

fn windows_text(range: RpmRange, tolerance: &Tolerance) -> String {
    bpm_windows(range, tolerance).iter().map(|(a, b)| format!("{a}-{b} BPM")).collect::<Vec<_>>().join(" or ")
}

/// What to ask for one segment, in words.
fn segment_line(index: usize, seg: &Segment, tolerance: &Tolerance) -> String {
    let range = seg.cadence();
    let name = words::segment_name(Lang::En, seg.kind);
    let mut line = format!(
        "- segment {index}: {name}, {} s, pedal {}-{} RPM, so the song's tempo must be {}",
        seg.seconds,
        range.min_rpm,
        range.max_rpm,
        windows_text(range, tolerance)
    );
    if seg.interval.is_some() {
        line.push_str(" (sprint intervals inside one song: pick a driving song)");
    }
    line
}

/// The rider wants to sing along: well-known songs first, and the bands or songs they name lead the ride.
const KNOWN_SONGS_RULE: &str = "Prefer well-known songs: the hits, singles and signature songs most people know \
    and sing along to, in their original studio versions (no covers, live versions, remixes or deep cuts unless the \
    rider asks). If the rider names bands or artists (in the genres or in their words), most songs must be those \
    artists' best-known songs; if they name a song, put it in the segment whose tempo fits it and pick the rest in \
    its spirit (same artist first, then similar well-known artists). For each segment list the candidates from the \
    most famous to the least, so the best-known song that fits the tempo is played.";

fn build_system_prompt(wish: &MusicWish, allow_explicit: bool, lang: Lang) -> String {
    let genres = if wish.genres.is_empty() {
        "any genre that suits the vibe".to_owned()
    } else if wish.mix && wish.genres.len() > 1 {
        format!(
            "a mix of {}: spread the genres over the segments by tempo, so each genre sits where its usual tempo \
             fits best (for example metal or punk for sprints and flats, rock for climbs, something calmer for the \
             warm-up and cool-down); use every genre at least once",
            wish.genres.join(", ")
        )
    } else {
        format!("only {}", wish.genres.join(" / "))
    };
    let vibe =
        if wish.sentence.is_empty() { String::new() } else { format!(" The rider also said: \"{}\".", wish.sentence) };
    let explicit = if allow_explicit { "Explicit songs are fine." } else { "No explicit songs." };
    format!(
        "You are an expert indoor cycling DJ. Pick songs for the segments of a ride; the rider pedals on the \
         beat, so each song's tempo (BPM) must fit its segment. Music: {genres}.{vibe} {KNOWN_SONGS_RULE} Call \
         {PROPOSE_RIDE_SONGS} once. Propose only real, released songs by their exact title and main artist, never \
         the same song twice, and prefer songs you know the BPM of. {explicit} {}",
        language_rule(lang)
    )
}

fn parse_proposals(arguments: &Value, segment_count: usize) -> Vec<Proposal> {
    let text = |v: &Value, key: &str| -> String {
        v.get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default()
            .chars()
            .take(MAX_SONG_FIELD_CHARS)
            .collect()
    };
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for entry in arguments.get("segments").and_then(Value::as_array).into_iter().flatten() {
        let Some(segment) =
            entry.get("segment").and_then(Value::as_u64).map(|s| s as usize).filter(|s| *s < segment_count)
        else {
            continue;
        };
        for (rank, song) in entry.get("songs").and_then(Value::as_array).into_iter().flatten().enumerate() {
            let (title, artist) = (text(song, "title"), text(song, "artist"));
            if title.is_empty() || artist.is_empty() {
                continue;
            }
            if seen.insert((crate::playlist::artist_key(&artist), song_key(&title))) {
                out.push(Proposal {
                    segment,
                    rank,
                    candidate: SongCandidate { title, artist, ..SongCandidate::default() },
                });
            }
        }
    }
    out
}

fn propose_tool() -> Tool {
    Tool {
        name: PROPOSE_RIDE_SONGS.into(),
        description: "Propose candidate songs for each segment of the ride.".into(),
        parameters: json!({
            "type": "object",
            "properties": { "segments": { "type": "array", "items": { "type": "object", "properties": {
                "segment": { "type": "integer" },
                "songs": { "type": "array", "items": { "type": "object", "properties": {
                    "title": { "type": "string" },
                    "artist": { "type": "string", "description": "Main artist as credited on the original release" }
                }, "required": ["title", "artist"] } }
            }, "required": ["segment", "songs"] } } },
            "required": ["segments"]
        }),
    }
}

/// One call: candidate songs for each segment of `workout` from the rider's wish. The model is resolved
/// if the config names none; the answer is read leniently (a bad entry is skipped).
pub(crate) async fn propose_songs(
    config: &AiConfig,
    workout: &Workout,
    wish: &MusicWish,
    tolerance: &Tolerance,
    allow_explicit: bool,
    lang: Lang,
) -> Result<Vec<Proposal>> {
    let mut config = config.clone();
    if config.model.trim().is_empty() {
        config.model = resolve_model(&config).await?;
    }
    let lines: Vec<String> = workout.segments.iter().enumerate().map(|(i, s)| segment_line(i, s, tolerance)).collect();
    let instruction = format!(
        "Propose {SONGS_PER_SEGMENT} songs for each of these {} segments, each song lasting about as long as its \
         segment (a song may be up to 20 % shorter or longer):\n{}",
        workout.segments.len(),
        lines.join("\n")
    );
    let messages = [Message::User { text: instruction }];
    let reply = chat(&config, &build_system_prompt(wish, allow_explicit, lang), &messages, &[propose_tool()]).await?;
    let call = reply
        .tool_calls
        .into_iter()
        .find(|c| c.name == PROPOSE_RIDE_SONGS)
        .ok_or_else(|| AiError::new(&config, AiErrorKind::BadAnswer, "no propose_ride_songs"))?;
    Ok(parse_proposals(&call.arguments, workout.segments.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exercise::settings::ExerciseSettings;

    #[test]
    fn set_workout_builds_a_request_from_plain_fields() {
        let mut request = RideRequest::default();
        let mut title = String::new();
        let args = json!({
            "title": "Rock climb", "minutes": 32, "style": "easy",
            "genres": ["Rock", "rock", "Metal"], "mix": true, "sentence": "90s please",
            "bogus": 1
        });
        apply_set_workout(&mut request, &mut title, "30 min rock", ("anthropic", "m"), &args);
        assert_eq!((request.minutes, request.style, title.as_str()), (30, PresetId::Easy, "Rock climb"));
        assert_eq!(
            request.wish,
            MusicWish { genres: vec!["Rock".into(), "Metal".into()], mix: true, sentence: "90s please".into() }
        );
        assert!(request.workout.is_none(), "a style at a length is built by the preset");
    }

    #[test]
    fn explicit_segments_become_a_validated_workout() {
        let mut request = RideRequest::default();
        let mut title = String::new();
        let args = json!({ "segments": [
            { "kind": "warm_up", "minutes": 4, "min_rpm": 80, "max_rpm": 90, "effort": 3 },
            { "kind": "sprint", "minutes": 4, "min_rpm": 100, "max_rpm": 130, "effort": 11, "position": "standing",
              "sprint": { "work_s": 20, "rest_s": 40, "rounds": 50 } },
            { "kind": "teleport", "minutes": 1 },
            { "minutes": 2 }
        ]});
        apply_set_workout(&mut request, &mut title, "words", ("p", "m"), &args);
        let w = request.workout.clone().unwrap();
        assert_eq!(w.segments.len(), 3, "an unknown kind reads as a flat; a segment without a kind is skipped");
        assert_eq!(w.segments[1].effort, Effort(10));
        let iv = w.segments[1].interval.unwrap();
        assert!(iv.total_s() <= w.segments[1].seconds, "rounds were lowered to fit");
        assert!(
            matches!(&w.origin, WorkoutOrigin::Ai { wish, provider_id, .. } if wish == "words" && provider_id == "p")
        );
        // a later length change goes back to the preset
        apply_set_workout(&mut request, &mut title, "", ("p", "m"), &json!({ "minutes": 60 }));
        assert!(request.workout.is_none() && request.minutes == 60);
    }

    #[test]
    fn proposals_are_read_leniently_and_deduplicated() {
        let args = json!({ "segments": [
            { "segment": 0, "songs": [ { "title": "Africa", "artist": "Toto" }, { "title": "Africa (Remastered)", "artist": "Toto" },
                                       { "title": "", "artist": "x" }, { "artist": "no title" } ] },
            { "segment": 9, "songs": [ { "title": "Out of range", "artist": "X" } ] },
            { "segment": 1, "songs": [ { "title": "Enter Sandman", "artist": "Metallica" } ] }
        ]});
        let p = parse_proposals(&args, 3);
        assert_eq!(
            p.iter().map(|p| (p.segment, p.candidate.title.as_str())).collect::<Vec<_>>(),
            [(0, "Africa"), (1, "Enter Sandman")]
        );
        assert!(parse_proposals(&json!({}), 3).is_empty());
    }

    #[test]
    fn the_prompts_carry_the_genres_the_mix_and_the_tempo_windows() {
        let wish = MusicWish { genres: vec!["rock".into(), "metal".into()], mix: true, sentence: "no ballads".into() };
        let system = build_system_prompt(&wish, false, Lang::En);
        assert!(system.contains("spread the genres over the segments by tempo") && system.contains("no ballads"));
        assert!(system.contains("No explicit songs.") && system.contains(PROPOSE_RIDE_SONGS));
        assert!(system.contains("Prefer well-known songs") && system.contains("most famous to the least"));
        let single =
            build_system_prompt(&MusicWish { genres: vec!["metal".into()], ..MusicWish::default() }, true, Lang::Es);
        assert!(single.contains("only metal") && single.contains("Spanish"));
        let tol = Tolerance::from_settings(&ExerciseSettings::default());
        let line = segment_line(2, &Segment { kind: SegmentKind::WarmUp, seconds: 120, ..Segment::default() }, &tol);
        assert!(line.contains("segment 2") && line.contains("76-99 BPM or 152-198 BPM"), "{line}");
    }

    #[test]
    fn tools_are_named_for_the_model() {
        assert_eq!(set_workout_tool().name, "set_workout");
        assert_eq!(propose_tool().name, "propose_ride_songs");
    }
}
