//! One AI-made-playlist conversation with the user's AI: the brief ([`Brief`]:
//! the party answers or the free chat), the chat that sharpens it and the build.
//!
//! The chat only asks questions (quick replies included). The build asks the
//! AI for songs with a required `propose_songs` call, resolves each one
//! against Tidal and the library ([`Resolver`], whose confidence floor is the
//! guard against invented songs), and arranges the result.

use std::collections::HashSet;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::arrange::{arrange, cap_per_artist, effective_ms, pick_for_duration, Picked, AVG_SONG_MS};
use super::chat::{opener, set_playlist_tool, MAX_SONG_FIELD_CHARS, SET_PLAYLIST};
use super::words::{self, Problem};
use super::{Brief, ChatBrief, PartyBrief};
use crate::ai::prompt::{json_in_text, language_rule, one_line, tagged, DATA_RULE, ORIGINAL_TITLES_RULE};
use crate::ai::replies::{quick_replies, suggest_replies_tool, SUGGEST_REPLIES};
use crate::ai::{
    chat, chat_with, resolve_model, AiConfig, AiError, AiErrorKind, ChatOptions, Message, Tool, ToolResult,
    MAX_OUTPUT_CEILING,
};
use crate::api::TidalApi;
use crate::concert::words::{self as concert_words, Problem as ConcertProblem};
use crate::concert::BUILD_DEADLINE;
use crate::humane::Lang;
use crate::playlist::{
    new_id, now_ms, HomePlaylist, LibraryIndex, PlaylistEntry, PlaylistSettings, Resolver, SeenEntries, SongCandidate,
    PLAYLIST_FORMAT_VERSION,
};
use crate::text::song_key;
use crate::Result;

/// Model calls per chat message (tool rounds included).
const MAX_ROUNDS_PER_TURN: usize = 3;
/// The first chat message: the answers are already in the system prompt.
const FIRST_QUESTION_REQUEST: &str = "The answers are in the system prompt. Ask your first question.";
/// More songs are asked for than needed: some are not found, some repeat or are left out.
const CANDIDATE_OVERSHOOT: f64 = 1.5;
/// Fewest and most songs asked of the AI in one call.
const MIN_ASK: usize = 12;
const MAX_ASK: usize = 80;
/// Tokens one proposed song takes in the tool call (title, artist, energy, JSON around them), with room to spare.
const TOKENS_PER_SONG: usize = 50;
/// Room for the model's own thinking and the tool call's frame: those count against the limit too.
const BUILD_OVERHEAD_TOKENS: usize = 2048;
/// Extra calls to the AI when the songs found don't fill the length.
const MAX_FILL_ROUNDS: u8 = 2;
/// Songs kept in the "Not found" list.
const MAX_UNMATCHED: usize = 50;
/// Songs of the last playlist remembered for "build again".
const MAX_LAST_SONGS: usize = 100;
const PROPOSE_SONGS: &str = "propose_songs";
const DEFAULT_ENERGY: u8 = 3;
const MIN_ENERGY: u8 = 1;
const MAX_ENERGY: u8 = 5;
const MS_PER_MINUTE: u64 = 60_000;

/// What a failed chat turn goes back to.
struct Checkpoint {
    messages: usize,
    user_turns: u8,
    brief: Brief,
}

/// What the build works with. Borrowed: the app owns all of it.
pub struct PartyContext<'a> {
    /// Clamped inside.
    pub settings: &'a PlaylistSettings,
    /// `None` = not signed in to Tidal: library only.
    pub api: Option<&'a TidalApi>,
    pub library: &'a LibraryIndex,
}

/// What the AI says in the chat, and the answers it offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PartyTurn {
    pub text: String,
    pub quick_replies: Vec<String>,
    /// The brief as the AI now understands it (chat mode only; party turns carry `None`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_brief: Option<ChatBrief>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartyBuilt {
    /// Not saved: the app saves it on "Save".
    pub playlist: HomePlaylist,
    /// Shortfall, no Tidal or explicit songs left out, already in the session language.
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PartyProgress {
    /// Waiting for the AI's songs (`round` 0 is the first ask, then the fill-ups).
    Asking {
        round: u8,
    },
    Resolving {
        done: usize,
        total: usize,
    },
}

type Progress<'a> = &'a (dyn Fn(PartyProgress) + Send + Sync);

pub struct PartySession {
    config: AiConfig,
    brief: Brief,
    lang: Lang,
    messages: Vec<Message>,
    user_turns: u8,
    /// "Artist – Title" of the last playlist built, so "build again" can adjust it.
    last_songs: Vec<String>,
}

/// "Party playlists are made by your AI…" in `lang`.
pub fn needs_ai_message(lang: Lang) -> String {
    words::needs_ai(lang)
}

/// "The conversation was closed…" in `lang`.
pub fn no_session_message(lang: Lang) -> String {
    words::no_session(lang)
}

impl PartySession {
    pub fn new(config: AiConfig, brief: impl Into<Brief>, lang: Lang) -> Self {
        Self {
            config,
            brief: brief.into().clamped(),
            lang,
            messages: Vec::new(),
            user_turns: 0,
            last_songs: Vec::new(),
        }
    }

    pub fn brief(&self) -> &Brief {
        &self.brief
    }

    /// New answers; the chat so far is kept.
    pub fn set_brief(&mut self, brief: impl Into<Brief>) {
        self.brief = brief.into().clamped();
    }

    /// Clears the chat. Party: the AI's first question, with quick replies.
    /// Chat: HomeFi's fixed opener and example answers, no AI call.
    pub async fn start(&mut self) -> Result<PartyTurn> {
        self.messages.clear();
        self.user_turns = 0;
        if let Brief::Chat(brief) = &self.brief {
            let mut turn = opener(self.lang, &brief.wish);
            turn.chat_brief = Some(brief.clone());
            return Ok(turn);
        }
        let undo = self.checkpoint();
        self.messages.push(Message::User { text: FIRST_QUESTION_REQUEST.into() });
        self.run_chat(undo).await
    }

    /// The user's answer. After the brief's turn limit the turn is fixed words and no AI is called.
    pub async fn reply(&mut self, text: &str) -> Result<PartyTurn> {
        if self.messages.is_empty() && matches!(self.brief, Brief::Party(_)) {
            return self.start().await;
        }
        if self.user_turns >= self.brief.max_chat_turns() {
            let chat_brief = self.brief.as_chat().cloned();
            return Ok(PartyTurn { text: words::chat_limit(self.lang), quick_replies: Vec::new(), chat_brief });
        }
        let undo = self.checkpoint();
        let text = text.trim();
        if let Brief::Chat(brief) = &mut self.brief {
            if brief.wish.is_empty() {
                brief.wish = text.to_owned();
                *brief = std::mem::take(brief).clamped();
            }
        }
        self.messages.push(Message::User { text: text.to_owned() });
        self.user_turns += 1;
        self.run_chat(undo).await
    }

    /// What a failed chat turn goes back to.
    fn checkpoint(&self) -> Checkpoint {
        Checkpoint { messages: self.messages.len(), user_turns: self.user_turns, brief: self.brief.clone() }
    }

    /// The chat loop. On a failed call the history and the brief go back to `undo`
    /// (the user's words stay out, so "try again" resends them cleanly).
    async fn run_chat(&mut self, undo: Checkpoint) -> Result<PartyTurn> {
        let outcome = self.chat_rounds().await;
        if outcome.is_err() {
            self.messages.truncate(undo.messages);
            self.user_turns = undo.user_turns;
            self.brief = undo.brief;
        }
        outcome
    }

    async fn chat_rounds(&mut self) -> Result<PartyTurn> {
        if self.config.model.trim().is_empty() {
            self.config.model = resolve_model(&self.config).await?;
        }
        let tools: Vec<Tool> = match self.brief {
            Brief::Party(_) => vec![suggest_replies_tool()],
            Brief::Chat(_) => vec![suggest_replies_tool(), set_playlist_tool()],
        };
        let system = chat_system_prompt(&self.brief, self.lang);
        let mut turn = PartyTurn::default();
        let mut texts: Vec<String> = Vec::new();
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
            let mut needs_model = false;
            let results: Vec<ToolResult> = calls
                .iter()
                .map(|call| {
                    let content = match (call.name.as_str(), &mut self.brief) {
                        (SUGGEST_REPLIES, _) => {
                            turn.quick_replies = quick_replies(call.arguments.get("options"));
                            Some(json!({ "shown": turn.quick_replies.len() }).to_string())
                        }
                        (SET_PLAYLIST, Brief::Chat(brief)) => {
                            brief.apply(&call.arguments);
                            Some(json!({ "ok": true }).to_string())
                        }
                        _ => None,
                    };
                    let known = content.is_some();
                    let content = content.unwrap_or_else(|| {
                        needs_model = true;
                        format!("unknown tool {}", call.name)
                    });
                    ToolResult { call_id: call.id.clone(), name: call.name.clone(), content, is_error: !known }
                })
                .collect();
            self.messages.push(Message::Tool { results });
            // the tools are answered and the AI has spoken: wait for the user. Some models call the
            // tools first and write after the results, so with no words yet they get one more round.
            if !needs_model && !texts.is_empty() {
                break;
            }
        }
        turn.text = texts.join("\n\n");
        if turn.text.is_empty() {
            let title = self.brief.as_chat().map(|b| b.title.as_str()).unwrap_or_default();
            turn.text = words::empty_turn(self.lang, title);
        }
        turn.chat_brief = self.brief.as_chat().cloned();
        Ok(turn)
    }

    /// Builds the playlist within [`BUILD_DEADLINE`]. Cancel-safe: the session
    /// changes only when the build succeeds.
    pub async fn build(&mut self, ctx: &PartyContext<'_>, progress: Progress<'_>) -> Result<PartyBuilt> {
        self.build_within(ctx, progress, BUILD_DEADLINE).await
    }

    pub(crate) async fn build_within(
        &mut self,
        ctx: &PartyContext<'_>,
        progress: Progress<'_>,
        deadline: Duration,
    ) -> Result<PartyBuilt> {
        let lang = self.lang;
        let result = tokio::time::timeout(deadline, self.build_inner(ctx, progress))
            .await
            .unwrap_or_else(|_| Err(concert_words::problem(lang, ConcertProblem::TooSlow)))?;
        self.config.model = result.model;
        self.last_songs = result.last_songs;
        Ok(result.built)
    }

    /// Reads the session, changes nothing: the caller commits what it returns.
    async fn build_inner(&self, ctx: &PartyContext<'_>, progress: Progress<'_>) -> Result<BuildResult> {
        if ctx.api.is_none() && ctx.library.is_empty() {
            return Err(words::problem(self.lang, Problem::NothingToUse));
        }
        if self.brief.as_chat().is_some_and(|b| !b.is_asked()) {
            return Err(words::problem(self.lang, Problem::NothingAsked));
        }
        let settings = ctx.settings.clamped();
        let mut config = self.config.clone();
        if config.model.trim().is_empty() {
            config.model = resolve_model(&config).await?;
        }
        let target_ms = self.brief.target_ms();
        let resolver = Resolver { api: ctx.api, library: ctx.library, prefer_library: settings.prefer_library };
        let mut pool = Pool::default();
        self.pin_asked_songs(&resolver, &mut pool, progress).await;

        for round in 0..=MAX_FILL_ROUNDS {
            progress(PartyProgress::Asking { round });
            let missing_ms = target_ms.saturating_sub(pool.total_ms());
            let instruction = build_instruction(&self.brief, &settings, ask_count(missing_ms), &self.last_songs, &pool);
            let proposed = self.ask_for_songs(&config, instruction, ask_count(missing_ms)).await?;
            let fresh: Vec<Proposed> = proposed.into_iter().filter(|p| pool.asked.insert(p.key())).collect();
            pool.proposed += fresh.len();
            let (candidates, energies): (Vec<SongCandidate>, Vec<u8>) =
                fresh.into_iter().map(|p| (p.candidate, p.energy)).unzip();
            let resolved = resolver
                .resolve_all(candidates, &|done, total| progress(PartyProgress::Resolving { done, total }))
                .await;
            for (resolved, energy) in resolved.into_iter().zip(energies) {
                pool.failed += usize::from(resolved.failed);
                pool.add(resolved.entry, resolved.candidate, energy, self.brief.allow_explicit());
            }
            pool.kept = cap_per_artist(std::mem::take(&mut pool.kept), settings.party_max_per_artist);
            if pool.total_ms() >= target_ms.saturating_sub(AVG_SONG_MS) {
                break;
            }
        }

        // nothing of the AI's own was found: a playlist of only the songs the user named is no playlist
        if pool.kept.is_empty() {
            let problem = if pool.proposed == 0 {
                Problem::NoProposals
            } else if pool.failed * 2 >= pool.proposed {
                Problem::TidalBusy { failed: pool.failed, asked: pool.proposed }
            } else {
                Problem::NoSongs
            };
            tracing::warn!(
                proposed = pool.proposed,
                failed = pool.failed,
                "a playlist with AI found none of its songs"
            );
            return Err(words::problem(self.lang, problem));
        }
        let pinned_ms = pool.pinned_ms();
        let (picked, _rest) = pick_for_duration(std::mem::take(&mut pool.kept), target_ms.saturating_sub(pinned_ms));
        let picked_ms = pinned_ms + picked.iter().map(|p| effective_ms(&p.entry)).sum::<u64>();
        // the songs the user named open the playlist, in their order; the rest follows the energy curve
        let mut entries = std::mem::take(&mut pool.pinned);
        entries.extend(arrange(picked, self.brief.curve(), self.lang));
        let note = self.note(target_ms.saturating_sub(picked_ms), &pool, ctx.api.is_none());
        let last_songs = entries.iter().take(MAX_LAST_SONGS).map(|e| format!("{} – {}", e.artist, e.title)).collect();
        let now = now_ms();
        let playlist = HomePlaylist {
            version: PLAYLIST_FORMAT_VERSION,
            id: new_id(),
            name: self.brief.name(self.lang),
            description: self.brief.description(self.lang),
            created_ms: now,
            updated_ms: now,
            origin: self.brief.origin(config.provider.id().to_owned(), config.model.clone(), now),
            entries,
            unmatched: pool.unmatched,
            sources: Vec::new(),
            search_widget_html: None,
            tidal_uuid: None,
        };
        Ok(BuildResult { built: PartyBuilt { playlist, note }, model: config.model, last_songs })
    }

    /// Finds the songs the user named (chat mode) and pins them: they are kept even when
    /// explicit, and the AI's own proposals of them count as repeats.
    async fn pin_asked_songs(&self, resolver: &Resolver<'_>, pool: &mut Pool, progress: Progress<'_>) {
        let Some(brief) = self.brief.as_chat().filter(|b| !b.songs.is_empty()) else {
            return;
        };
        let candidates: Vec<SongCandidate> = brief
            .songs
            .iter()
            .map(|s| SongCandidate { title: s.title.clone(), artist: s.artist.clone(), ..SongCandidate::default() })
            .collect();
        pool.asked.extend(brief.songs.iter().map(|s| s.key()));
        let resolved =
            resolver.resolve_all(candidates, &|done, total| progress(PartyProgress::Resolving { done, total })).await;
        for resolved in resolved {
            match resolved.entry {
                Some(entry) if pool.seen.first_time(&entry) => pool.pinned.push(entry),
                Some(_) => {}
                None => pool.unmatched.push(resolved.candidate),
            }
        }
    }

    /// Short of the length, searches TIDAL didn't answer, explicit songs left out, no Tidal: said in
    /// words, joined with a space.
    fn note(&self, short_ms: u64, pool: &Pool, no_tidal: bool) -> Option<String> {
        let mut parts = Vec::new();
        if short_ms > AVG_SONG_MS {
            parts.push(words::short_note(self.lang, short_ms / MS_PER_MINUTE, self.brief.minutes()));
        }
        if pool.failed > 0 {
            parts.push(words::failed_note(self.lang, pool.failed));
        }
        if pool.explicit_left_out > 0 {
            parts.push(words::explicit_note(self.lang, pool.explicit_left_out));
        }
        if no_tidal {
            parts.push(concert_words::no_tidal_note(self.lang));
        }
        (!parts.is_empty()).then(|| parts.join(" "))
    }

    /// One `propose_songs` call on the chat so far plus `instruction` (not stored).
    async fn ask_for_songs(&self, config: &AiConfig, instruction: String, count: usize) -> Result<Vec<Proposed>> {
        let mut conversation = self.messages.clone();
        conversation.push(Message::User { text: instruction });
        let tools = [propose_songs_tool()];
        let options =
            ChatOptions { max_tokens: Some(build_output_tokens(count)), light_reasoning: true, ..Default::default() };
        let reply = chat_with(config, &build_system_prompt(&self.brief), &conversation, &tools, &options).await?;
        let arguments = match reply.tool_calls.iter().find(|c| c.name == PROPOSE_SONGS) {
            Some(call) => call.arguments.clone(),
            // a small model may write the answer instead of calling the tool
            None => rescued_songs(&reply.text)
                .ok_or_else(|| AiError::new(config, AiErrorKind::BadAnswer, "no propose_songs"))?,
        };
        Ok(parse_proposed(&arguments))
    }
}

/// What a successful build hands back to be committed to the session.
struct BuildResult {
    built: PartyBuilt,
    model: String,
    last_songs: Vec<String>,
}

/// A song the AI proposed.
struct Proposed {
    candidate: SongCandidate,
    energy: u8,
}

impl Proposed {
    /// Same song under another spelling or credit counts once.
    fn key(&self) -> (String, String) {
        (crate::playlist::artist_key(&self.candidate.artist), song_key(&self.candidate.title))
    }
}

/// What has been found so far across the rounds.
#[derive(Default)]
struct Pool {
    /// The songs the user named, found: they open the playlist.
    pinned: Vec<PlaylistEntry>,
    kept: Vec<Picked>,
    unmatched: Vec<SongCandidate>,
    seen: SeenEntries,
    asked: HashSet<(String, String)>,
    explicit_left_out: usize,
    /// Songs the AI proposed (repeats left out), and those whose TIDAL search failed or timed out.
    proposed: usize,
    failed: usize,
}

impl Pool {
    fn total_ms(&self) -> u64 {
        self.pinned_ms() + self.kept.iter().map(|p| effective_ms(&p.entry)).sum::<u64>()
    }

    fn pinned_ms(&self) -> u64 {
        self.pinned.iter().map(effective_ms).sum()
    }

    fn add(&mut self, entry: Option<PlaylistEntry>, candidate: SongCandidate, energy: u8, allow_explicit: bool) {
        match entry {
            None if self.unmatched.len() < MAX_UNMATCHED => self.unmatched.push(candidate),
            None => {}
            Some(entry) if !allow_explicit && entry.explicit => self.explicit_left_out += 1,
            Some(entry) if self.seen.first_time(&entry) => self.kept.push(Picked { entry, energy }),
            Some(_) => {}
        }
    }
}

/// Songs to ask the AI for, to cover `missing_ms`.
fn ask_count(missing_ms: u64) -> usize {
    let needed = missing_ms.div_ceil(AVG_SONG_MS);
    let wanted = (needed as f64 * CANDIDATE_OVERSHOOT).ceil() as usize;
    wanted.clamp(MIN_ASK, MAX_ASK)
}

/// Output tokens the answer asking for `count` songs may take: the songs plus the overhead, up to the ceiling.
fn build_output_tokens(count: usize) -> u32 {
    u32::try_from(count * TOKENS_PER_SONG + BUILD_OVERHEAD_TOKENS).unwrap_or(MAX_OUTPUT_CEILING).min(MAX_OUTPUT_CEILING)
}

/// The answer of a model that wrote JSON in its text: the object with `songs`, or the bare list.
fn rescued_songs(text: &str) -> Option<Value> {
    match json_in_text(text)? {
        list @ Value::Array(_) => Some(json!({ "songs": list })),
        object if object.get("songs").is_some() => Some(object),
        _ => None,
    }
}

fn parse_proposed(arguments: &Value) -> Vec<Proposed> {
    let text = |song: &Value, key: &str| {
        one_line(song.get(key).and_then(Value::as_str).unwrap_or_default(), MAX_SONG_FIELD_CHARS)
    };
    let mut seen = HashSet::new();
    arguments
        .get("songs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|song| {
            let (title, artist) = (text(song, "title"), text(song, "artist"));
            if title.is_empty() || artist.is_empty() {
                return None;
            }
            let energy = song
                .get("energy")
                .and_then(Value::as_f64)
                .map_or(DEFAULT_ENERGY, |e| e.round().clamp(f64::from(MIN_ENERGY), f64::from(MAX_ENERGY)) as u8);
            let proposed = Proposed { candidate: SongCandidate { title, artist, ..SongCandidate::default() }, energy };
            seen.insert(proposed.key()).then_some(proposed)
        })
        .collect()
}

fn propose_songs_tool() -> Tool {
    Tool {
        name: PROPOSE_SONGS.into(),
        description: "Propose the songs of the playlist, in the order they should be tried.".into(),
        parameters: json!({
            "type": "object",
            "properties": { "songs": { "type": "array", "items": { "type": "object", "properties": {
                "title": { "type": "string" },
                "artist": { "type": "string", "description": "Main artist as credited on the original release" },
                "energy": { "type": "integer", "minimum": MIN_ENERGY, "maximum": MAX_ENERGY }
            }, "required": ["title", "artist", "energy"] } } },
            "required": ["songs"]
        }),
    }
}

/// The answers in plain words, for the prompts.
pub(super) fn describe_brief(brief: &PartyBrief) -> String {
    let guests = words::guests_word(Lang::En, brief.guests);
    let ages = if brief.ages.is_empty() {
        "any age".to_owned()
    } else {
        brief.ages.iter().map(|a| words::age_word(Lang::En, *a)).collect::<Vec<_>>().join(", ")
    };
    let vibe = match brief.vibe {
        super::PartyVibe::Background => "background music, nothing that demands attention",
        super::PartyVibe::Chill => "chill and relaxed",
        super::PartyVibe::Dance => "a dance party",
        super::PartyVibe::Singalong => "songs everyone can sing along to",
        super::PartyVibe::Throwback => "throwbacks, songs that bring back memories",
    };
    let curve = brief.curve.prompt_phrase();
    let explicit = if brief.allow_explicit { "allowed" } else { "to avoid" };
    let mut text = format!(
        "- Guests: {guests}\n- Ages: {ages}\n- Vibe: {vibe}\n- Length: {}\n- Energy: {curve}\n- Explicit lyrics: {explicit}",
        words::length(brief.minutes)
    );
    if !brief.notes.is_empty() {
        text.push_str(&format!("\n- Anything else: {}", tagged("user_notes", &brief.notes)));
    }
    text
}

fn chat_system_prompt(brief: &Brief, lang: Lang) -> String {
    let describe = brief.describe();
    let rule = language_rule(lang);
    match brief {
        Brief::Party(_) => format!(
            "You are the DJ helper of a music app. The user is planning a party playlist. Their answers so far:\n{describe}\n\n\
             Ask at most ONE short question per message about their taste: artists everyone likes, genres, languages, \
             songs that must play or must not. Call {SUGGEST_REPLIES} with 2 to 4 short answers every time. Never list \
             songs in the chat: the playlist is built later. {DATA_RULE} {rule}"
        ),
        Brief::Chat(_) => format!(
            "You are the DJ helper of a music app. The user wants a playlist and says in their own words what it is \
             for: a mood, a decade, a language, a genre, a moment of the day. HomeFi already asked them: \"What kind \
             of playlist do you want today?\". What you know so far:\n{describe}\n\n\
             Reply in one to three short sentences: say what you understood and suggest ONE direction or ask ONE \
             question (artists to lean on, a decade, a language, calmer or more upbeat, how long). Call \
             {SUGGEST_REPLIES} with 2 to 4 short answers every time. Whenever you learn what the playlist is about, \
             call {SET_PLAYLIST} with a short title in the user's language and whatever else you learned (minutes, \
             allow_explicit, energy). When the user names songs (even misspelled or without the artist), work out the \
             real song and pass every one of them in `songs`, and name it back in your reply (\"Alors on danse by \
             Stromae\") so they can correct you. Always write a reply, never only call tools. Never list songs of \
             your own in the chat: the playlist is built later. {DATA_RULE} {rule}"
        ),
    }
}

fn build_system_prompt(brief: &Brief) -> String {
    let describe = brief.describe();
    let rule = format!("{ORIGINAL_TITLES_RULE} {DATA_RULE}");
    match brief {
        Brief::Party(_) => format!(
            "You are an expert party DJ. Pick the songs of a playlist for these answers:\n{describe}\n\nCall \
             {PROPOSE_SONGS} once with the songs. Propose only real, released songs by their exact title and main \
             artist. {rule}"
        ),
        Brief::Chat(_) => format!(
            "You are an expert music curator. Pick the songs of a playlist for what the user asked in the \
             conversation:\n{describe}\n\nCall {PROPOSE_SONGS} once with the songs. Propose only real, released songs \
             by their exact title and main artist. Stay inside the decade, language and genre the user asked for. The \
             songs to include are already in the playlist, do not propose them again: every other song must sound like \
             them (same genre and scene, era, language and energy; the same artist and close artists first), not just \
             share a mood. {rule}"
        ),
    }
}

fn build_instruction(
    brief: &Brief,
    settings: &PlaylistSettings,
    count: usize,
    last_songs: &[String],
    pool: &Pool,
) -> String {
    let explicit = if brief.allow_explicit() { "Explicit songs are fine." } else { "No explicit songs." };
    let mut text = format!(
        "Propose {count} real songs for this {}, in the order to try them, each with an energy from \
         {MIN_ENERGY} (calm) to {MAX_ENERGY} (peak). Exact titles, main artist of the original release. At most {} \
         songs per artist. {explicit}",
        brief.what(),
        settings.party_max_per_artist
    );
    if !last_songs.is_empty() {
        text.push_str(&format!(
            " The previous playlist was: {}. Apply what the user asked since.",
            last_songs.join("; ")
        ));
    }
    let in_playlist = pool.pinned.iter().chain(pool.kept.iter().map(|p| &p.entry));
    if !pool.kept.is_empty() || !pool.unmatched.is_empty() {
        let names = |songs: Vec<String>| songs.join("; ");
        let have = names(in_playlist.map(|e| format!("{} – {}", e.artist, e.title)).collect());
        let missing = names(pool.unmatched.iter().map(|c| format!("{} – {}", c.artist, c.title)).collect());
        text.push_str(&format!(
            " We still need more songs. Already in the playlist: {have}. Not found, do not repeat: {missing}. \
             Propose different songs."
        ));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::super::{EnergyCurve, MAX_FREE_CHAT_TURNS};
    use super::*;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn assert_send<T: Send>() {}

    #[test]
    fn the_session_can_be_held_across_awaits() {
        assert_send::<PartySession>();
    }

    #[test]
    fn songs_to_ask_follow_the_length() {
        assert_eq!(ask_count(0), MIN_ASK);
        assert_eq!(ask_count(120 * MS_PER_MINUTE), 48);
        assert_eq!(ask_count(480 * MS_PER_MINUTE), MAX_ASK);
    }

    #[test]
    fn proposals_are_cleaned() {
        let args = json!({ "songs": [
            { "title": " Tití Me Preguntó ", "artist": "Bad Bunny", "energy": 5 },
            { "title": "Titi me preguntó", "artist": "Bad Bunny feat. X", "energy": 2 },
            { "title": "", "artist": "Nobody", "energy": 3 },
            { "title": "No Artist", "artist": " ", "energy": 3 },
            { "title": "Loud", "artist": "A", "energy": 99 },
            { "title": "Quiet", "artist": "B", "energy": -4 },
            { "title": "Unknown", "artist": "C" }
        ] });
        let got = parse_proposed(&args);
        let view: Vec<(&str, u8)> = got.iter().map(|p| (p.candidate.title.as_str(), p.energy)).collect();
        assert_eq!(view, [("Tití Me Preguntó", 5), ("Loud", 5), ("Quiet", 1), ("Unknown", 3)]);
        assert!(parse_proposed(&json!({})).is_empty());
    }

    #[tokio::test]
    async fn a_failed_build_leaves_the_session_as_it_was() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).respond_with(ResponseTemplate::new(500)).mount(&server).await;
        let config = AiConfig::new(crate::ai::AiProvider::Anthropic, "m", Some("k".into())).with_base_url(server.uri());
        let library = LibraryIndex::new(vec![crate::playlist::LibraryRef { title: "A".into(), ..Default::default() }]);
        let settings = PlaylistSettings::default();
        let ctx = PartyContext { settings: &settings, api: None, library: &library };
        let mut session = PartySession::new(config, PartyBrief::default(), Lang::En);
        session.messages = vec![Message::User { text: "hi".into() }];
        session.user_turns = 1;
        session.last_songs = vec!["A – B".into()];
        assert!(session.build(&ctx, &|_| {}).await.is_err());
        assert_eq!(session.messages, [Message::User { text: "hi".into() }]);
        assert_eq!((session.user_turns, session.last_songs.as_slice()), (1, ["A – B".to_owned()].as_slice()));
    }

    #[tokio::test]
    async fn a_slow_build_ends_with_a_worded_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(5)))
            .mount(&server)
            .await;
        let config = AiConfig::new(crate::ai::AiProvider::Anthropic, "m", Some("k".into())).with_base_url(server.uri());
        let (settings, library) = (
            PlaylistSettings::default(),
            LibraryIndex::new(vec![crate::playlist::LibraryRef {
                path: "/m/a.flac".into(),
                title: "A".into(),
                artist: "B".into(),
                ..Default::default()
            }]),
        );
        let ctx = PartyContext { settings: &settings, api: None, library: &library };
        let mut session = PartySession::new(config, PartyBrief::default(), Lang::En);
        let started = std::time::Instant::now();
        let err = session.build_within(&ctx, &|_| {}, Duration::from_millis(100)).await.unwrap_err();
        assert_eq!(err.to_string(), "Building the playlist took too long. Try again.");
        assert!(started.elapsed() < Duration::from_secs(3));
        assert!(session.messages.is_empty() && session.last_songs.is_empty());
    }

    fn anthropic(blocks: Value) -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_json(json!({
            "id": "m", "type": "message", "role": "assistant", "model": "x", "stop_reason": "tool_use",
            "content": blocks, "usage": { "input_tokens": 1, "output_tokens": 1 }
        }))
    }

    fn chat_session(server: &MockServer) -> PartySession {
        let config = AiConfig::new(crate::ai::AiProvider::Anthropic, "m", Some("k".into())).with_base_url(server.uri());
        PartySession::new(config, ChatBrief::default(), Lang::En)
    }

    #[tokio::test]
    async fn a_chat_session_opens_without_calling_the_ai() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).respond_with(ResponseTemplate::new(500)).expect(0).mount(&server).await;
        let mut session = chat_session(&server);
        let turn = session.start().await.unwrap();
        assert!(turn.text.starts_with("What kind of playlist do you want today?"));
        assert_eq!(turn.quick_replies.len(), 4);
        assert_eq!(turn.chat_brief, Some(ChatBrief::default()));
        assert!(session.messages.is_empty());
    }

    #[tokio::test]
    async fn the_first_reply_becomes_the_wish() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(anthropic(json!([{ "type": "text", "text": "Nice." }])))
            .mount(&server)
            .await;
        let mut session = chat_session(&server);
        session.start().await.unwrap();
        let turn = session.reply("  sad 90s  ").await.unwrap();
        assert_eq!(turn.chat_brief.unwrap().wish, "sad 90s");
        session.reply("calmer").await.unwrap();
        assert_eq!(session.brief().as_chat().unwrap().wish, "sad 90s", "only the first message is the wish");
        assert_eq!(session.user_turns, 2);
    }

    #[tokio::test]
    async fn set_playlist_updates_the_brief_and_the_turn_carries_it() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(anthropic(json!([
                { "type": "text", "text": "Got it." },
                { "type": "tool_use", "id": "t1", "name": "suggest_replies", "input": { "options": ["A", "B"] } },
                { "type": "tool_use", "id": "t2", "name": "set_playlist",
                  "input": { "title": "Sad 90s", "minutes": 95, "energy": "rising", "allow_explicit": false } }
            ])))
            .mount(&server)
            .await;
        let mut session = chat_session(&server);
        let turn = session.reply("sad 90s").await.unwrap();
        let brief = turn.chat_brief.unwrap();
        assert_eq!(
            (brief.title.as_str(), brief.minutes, brief.curve, brief.allow_explicit),
            ("Sad 90s", 90, EnergyCurve::Rising, false)
        );
        assert_eq!(turn.quick_replies, ["A", "B"]);
        assert_eq!(session.brief().as_chat(), Some(&brief));
        // one model call: both tools are answered without asking the model again
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn a_model_that_calls_tools_first_gets_to_say_its_words() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(anthropic(json!([
                { "type": "tool_use", "id": "t1", "name": "suggest_replies", "input": { "options": ["A", "B"] } },
                { "type": "tool_use", "id": "t2", "name": "set_playlist", "input": { "title": "French club" } }
            ])))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .respond_with(anthropic(
                json!([{ "type": "text", "text": "French club from the 2010s, opening with Stromae." }]),
            ))
            .mount(&server)
            .await;
        let mut session = chat_session(&server);
        let turn = session.reply("alors dance and similar").await.unwrap();
        assert_eq!(turn.text, "French club from the 2010s, opening with Stromae.");
        assert_eq!(turn.quick_replies, ["A", "B"], "the replies of the first round stay");
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn a_failed_chat_turn_restores_the_brief() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).respond_with(ResponseTemplate::new(500)).mount(&server).await;
        let mut session = chat_session(&server);
        assert!(session.reply("sad 90s").await.is_err());
        assert_eq!(session.brief().as_chat(), Some(&ChatBrief::default()), "the wish went back too");
        assert_eq!((session.user_turns, session.messages.len()), (0, 0));
    }

    #[tokio::test]
    async fn chat_mode_allows_twelve_turns() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(anthropic(json!([{ "type": "text", "text": "ok" }])))
            .mount(&server)
            .await;
        let mut session = chat_session(&server);
        for n in 0..MAX_FREE_CHAT_TURNS {
            assert_eq!(session.reply(&format!("m{n}")).await.unwrap().text, "ok");
        }
        let sent = server.received_requests().await.unwrap().len();
        let last = session.reply("one more").await.unwrap();
        assert!(last.text.starts_with("That's plenty"));
        assert_eq!(server.received_requests().await.unwrap().len(), sent);
        assert_eq!(Brief::from(PartyBrief::default()).max_chat_turns(), 6);
    }
}
