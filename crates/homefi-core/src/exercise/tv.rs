//! The TV: what a screen across the room needs to show the ride. [`RideTvView`] is the stable, versioned
//! JSON (`/ride.json`, `"v": 1`) that the embedded browser page and a native client (an Android TV app)
//! both read; with feature `share` this module also has the small HTTP server, its mDNS announcement
//! (`_homefi-ride._tcp`) and the client's discovery. Free, LAN only: no Google Cast registration.
//!
//! Visual and audio clocks: the view carries the song position and when it was sampled, and the beat grid;
//! a client that plays the music itself (audio `tv`) takes the position from its own player, so the pulse
//! matches what it plays. A client that only draws (audio `phone`) extrapolates from `sampled_ms`.

use std::hash::{BuildHasher, Hasher};
use std::net::IpAddr;

use serde::{Deserialize, Serialize};

use super::cues::CueKind;
use super::model::{SegmentKind, Workout};
use super::profile::{HotPoint, ProfilePoint};
use super::ride::RideView;
use super::settings::{CourseLook, CourseRider, PulseStyle, TvAudio};
use super::words;
use crate::error::{Error, Result};
use crate::humane::Lang;
use crate::share::{AddMode, EditMode, Permissions, SkipMode};

/// Version of the JSON a TV client reads. A client refuses a higher one.
pub const TV_PROTOCOL_VERSION: u8 = 1;
const TOKEN_LEN: usize = 8;
/// No look-alike characters (0/O, 1/I/l) for a code someone may have to read.
const TOKEN_ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";

/// The words the TV shows, already in the ride's language, so a client only draws.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TvText {
    pub segment: String,
    /// "Standing" / "Seated".
    pub position: String,
    /// "Hard 7", or "Hard 7 · level 6/8" when the bike's levels are known.
    pub effort: String,
    /// "Standing climb · 65 RPM" (or the bike's unit: "· 14.3 km/h"), empty on the last segment.
    pub next: String,
    /// The unit the target is shown in: "RPM", "km/h" or "mph".
    pub unit: String,
    /// "Resistance +1 · 2 of 3" once a resistance step has started, else empty. The page decides how long to show it.
    pub step: String,
}

/// How a TV client joins the ride's hub as a listening guest (audio `tv`): the hub accepts only
/// listeners and lives only while the ride runs, so the code is a one-ride token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct RideListen {
    pub port: u16,
    pub code: String,
    /// Session name the hub shows.
    pub name: String,
}

/// One part of the ride for the TV's mountain: which kind (its colour) and where it sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TvPart {
    pub kind: SegmentKind,
    pub start_s: u32,
    pub seconds: u32,
}

impl TvPart {
    /// The parts of `workout`, in order.
    pub fn of(workout: &Workout) -> Vec<Self> {
        let mut start_s = 0;
        workout
            .segments
            .iter()
            .map(|s| {
                let part = Self { kind: s.kind, start_s, seconds: s.seconds };
                start_s += s.seconds;
                part
            })
            .collect()
    }
}

/// Most lines of lyrics the TV carries: a song has far fewer, a broken text must not bloat every poll.
pub const MAX_TV_LYRIC_LINES: usize = 400;
/// Most text bytes of lyrics the TV carries (the JSON is polled twice a second).
pub const MAX_TV_LYRICS_BYTES: usize = 40 * 1024;
/// Longest line the TV carries (chars); the page shrinks and then cuts what does not fit anyway.
const MAX_TV_LYRIC_LINE_CHARS: usize = 160;

/// One line of the song on the TV. `t_ms` is when it starts in the song (0 on every line of plain lyrics).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TvLyricLine {
    pub t_ms: u64,
    /// Empty = an instrumental gap. The page inserts it with `textContent`, so nothing is escaped here.
    pub text: String,
}

/// The lyrics of the song that plays now, sent once per song by the app; the page follows them on its own
/// from `ride.pulse.song_position_ms`, so there is no per-line traffic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TvLyrics {
    /// Identifies the song (its Tidal id or file path): the page restarts its scroll when it changes.
    pub song_key: String,
    /// Every line has a start time. Plain lyrics (`false`) scroll with the song's progress instead.
    pub synced: bool,
    /// Shift of the lyrics against the music (ms, positive shows them later): the Bluetooth delay.
    pub offset_ms: i32,
    /// Length of the song (ms), 0 = unknown. Only plain lyrics need it, to scroll at the song's pace.
    pub duration_ms: u64,
    pub lines: Vec<TvLyricLine>,
}

impl TvLyrics {
    /// The lyrics of `song_key` capped to [`MAX_TV_LYRIC_LINES`] and [`MAX_TV_LYRICS_BYTES`] (the tail is
    /// dropped); `None` when no line has text.
    pub fn new(song_key: &str, lyrics: &crate::lyrics::Lyrics, offset_ms: i32, duration_ms: u64) -> Option<Self> {
        let mut lines: Vec<TvLyricLine> = Vec::new();
        let mut bytes = 0;
        for line in &lyrics.lines {
            let text: String = line.text.trim().chars().take(MAX_TV_LYRIC_LINE_CHARS).collect();
            // plain lyrics have a blank line between verses: one gap is enough
            if text.is_empty() && lines.last().is_none_or(|l| l.text.is_empty()) {
                continue;
            }
            bytes += text.len();
            if lines.len() >= MAX_TV_LYRIC_LINES || bytes > MAX_TV_LYRICS_BYTES {
                break;
            }
            lines.push(TvLyricLine { t_ms: line.time_ms.unwrap_or(0), text });
        }
        while lines.last().is_some_and(|l| l.text.is_empty()) {
            lines.pop();
        }
        if lines.is_empty() {
            return None;
        }
        Some(Self { song_key: song_key.to_owned(), synced: lyrics.synced, offset_ms, duration_ms, lines })
    }
}

/// What stays the same during a ride.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TvSession {
    pub title: String,
    pub profile: Vec<ProfilePoint>,
    /// The parts under the profile, coloured by kind.
    pub parts: Vec<TvPart>,
    pub audio: TvAudio,
    pub listen: Option<RideListen>,
    /// How the TV draws the beat (the rider's setting).
    pub pulse_style: PulseStyle,
    /// How the TV draws the course mountain (the rider's setting).
    pub course_look: CourseLook,
    /// The mountain labels its summits with their time.
    pub course_summits: bool,
    /// The resistance steps of the climbs, marked on the mountain.
    pub hot_points: Vec<HotPoint>,
    /// How the TV draws the rider on the mountain.
    pub course_rider: CourseRider,
}

/// The newest ride state the app published.
#[derive(Debug, Clone, PartialEq)]
pub struct TvFrame {
    pub view: RideView,
    pub playing: bool,
    /// When the view's song position was read (ms, the server's clock).
    pub sampled_ms: u64,
}

/// The JSON of `/ride.json`. `ride` is `None` while no ride runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RideTvView {
    /// [`TV_PROTOCOL_VERSION`].
    pub v: u8,
    /// The server's clock at the moment of the answer (ms): a client gets its offset from it.
    pub server_now_ms: u64,
    /// When the song position in `ride.pulse` was read, on the server's clock.
    pub sampled_ms: u64,
    /// The music is playing: the position advances by itself between answers.
    pub playing: bool,
    pub title: String,
    pub audio: TvAudio,
    /// Only for audio `tv`, and only to devices on the local network.
    pub listen: Option<RideListen>,
    /// The course profile (elevation 0..1 against seconds).
    pub profile: Vec<ProfilePoint>,
    /// The parts under the profile (kind, start, length), for the mountain's colours.
    pub parts: Vec<TvPart>,
    pub ride: Option<RideView>,
    pub text: TvText,
    /// How to draw the beat: `ring`, `bar` or `flash`.
    pub pulse_style: PulseStyle,
    /// How to draw the course: `ribbon`, `alpine` or `stage`. Added to protocol 1: an older server omits it
    /// and the page falls back to ribbon.
    pub course_look: CourseLook,
    /// Label the tallest summits with their time (additive like `course_look`).
    pub course_summits: bool,
    /// The resistance steps to mark on the mountain (additive: an older server omits them).
    pub hot_points: Vec<HotPoint>,
    /// `bike` or `dot` (additive like `course_look`: an older server omits it and the page draws the bike).
    pub course_rider: CourseRider,
    /// A ride is running and its music is paused: the view is frozen.
    pub paused: bool,
    /// The digit of a 3-2-1 countdown showing at the moment the position was sampled (0 = none). A client
    /// that animates itself reads `ride.cues` against its own clock instead.
    pub countdown_s: u8,
    /// The lyrics of the song that plays now, when the app has them and the rider wants them (`tv_lyrics`).
    /// Added to protocol 1: an older client ignores it, an older server omits it.
    pub lyrics: Option<TvLyrics>,
}

impl Default for RideTvView {
    fn default() -> Self {
        Self {
            v: TV_PROTOCOL_VERSION,
            server_now_ms: 0,
            sampled_ms: 0,
            playing: false,
            title: String::new(),
            audio: TvAudio::Tv,
            listen: None,
            profile: Vec::new(),
            parts: Vec::new(),
            ride: None,
            text: TvText::default(),
            pulse_style: PulseStyle::default(),
            course_look: CourseLook::default(),
            course_summits: false,
            hot_points: Vec::new(),
            course_rider: CourseRider::default(),
            paused: false,
            countdown_s: 0,
            lyrics: None,
        }
    }
}

impl RideTvView {
    /// The answer for a request at `now_ms`. `listen` is withheld when `share_listen` is false (the peer
    /// is not on the local network) or the audio stays on the phone.
    pub fn build(session: &TvSession, frame: Option<&TvFrame>, now_ms: u64, lang: Lang, share_listen: bool) -> Self {
        let listen = session.listen.clone().filter(|_| share_listen && session.audio == TvAudio::Tv);
        let base = Self {
            server_now_ms: now_ms,
            title: session.title.clone(),
            audio: session.audio,
            listen,
            profile: session.profile.clone(),
            parts: session.parts.clone(),
            pulse_style: session.pulse_style,
            course_look: session.course_look,
            course_summits: session.course_summits,
            hot_points: session.hot_points.clone(),
            course_rider: session.course_rider,
            ..Self::default()
        };
        match frame {
            Some(f) => Self {
                sampled_ms: f.sampled_ms,
                playing: f.playing,
                text: text_for(&f.view, lang),
                paused: !f.playing && !f.view.finished,
                countdown_s: countdown_digit(&f.view),
                ride: Some(f.view.clone()),
                ..base
            },
            None => base,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".into())
    }

    /// Reads a `/ride.json` answer. Unknown fields are ignored; a newer protocol version is refused.
    pub fn from_json(json: &str) -> Result<Self> {
        let view: Self = serde_json::from_str(json)?;
        if view.v > TV_PROTOCOL_VERSION {
            return Err(Error::Playlist(format!("ride TV protocol {} is newer than this app understands", view.v)));
        }
        Ok(view)
    }
}

/// The digit of the countdown that is showing in `view` (a countdown cue that came due within the last second), or 0.
fn countdown_digit(view: &RideView) -> u8 {
    const DIGIT_MS: i64 = 1000;
    view.cues
        .iter()
        .filter(|c| (-DIGIT_MS..=0).contains(&c.in_ms))
        .find_map(|c| match c.kind {
            CueKind::Countdown3 => Some(3),
            CueKind::Countdown2 => Some(2),
            CueKind::Countdown1 => Some(1),
            _ => None,
        })
        .unwrap_or(0)
}

fn text_for(view: &RideView, lang: Lang) -> TvText {
    let level = view.level.map(|at| (at, view.level_count));
    TvText {
        segment: words::segment_name(lang, view.kind).to_owned(),
        position: words::position_word(lang, view.position).to_owned(),
        effort: words::effort_label(lang, view.effort, level),
        next: view
            .next
            .map(|n| format!("{} · {}", words::segment_name(lang, n.kind), words::shown_text(&n.shown)))
            .unwrap_or_default(),
        unit: view.shown.unit.label().to_owned(),
        step: view.step.filter(|s| s.index >= 2).map(|s| words::step_chip(lang, s.index, s.count)).unwrap_or_default(),
    }
}

const PAGE: &str = include_str!("tv/ride.html");

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// `id=name` for every part kind, `|`-separated: the page names the next part that differs from the
/// current one, which the JSON only describes by kind.
fn kind_names(lang: Lang) -> String {
    const KINDS: [(&str, SegmentKind); 8] = [
        ("warm_up", SegmentKind::WarmUp),
        ("seated_flat", SegmentKind::SeatedFlat),
        ("seated_climb", SegmentKind::SeatedClimb),
        ("standing_climb", SegmentKind::StandingClimb),
        ("jumps", SegmentKind::Jumps),
        ("sprint", SegmentKind::Sprint),
        ("recovery", SegmentKind::Recovery),
        ("cool_down", SegmentKind::CoolDown),
    ];
    KINDS.iter().map(|(id, kind)| format!("{id}={}", words::segment_name(lang, *kind))).collect::<Vec<_>>().join("|")
}

/// The TV page for `title` in `lang`. Every inserted text is escaped.
pub fn render_page(title: &str, lang: Lang) -> String {
    let code = match lang {
        Lang::En => "en",
        Lang::Es => "es",
    };
    let title = if title.trim().is_empty() { "HomeFi" } else { title.trim() };
    [
        ("{{LANG}}", code.to_owned()),
        ("{{TITLE}}", escape(title)),
        ("{{WAITING}}", escape(words::tv_waiting(lang))),
        ("{{RPM}}", escape(words::tv_rpm(lang))),
        ("{{NEXT}}", escape(words::tv_next(lang))),
        ("{{DONE}}", escape(words::tv_done(lang))),
        ("{{GO}}", escape(words::tv_work(lang))),
        ("{{REST}}", escape(words::tv_rest(lang))),
        ("{{PAUSED}}", escape(words::tv_paused(lang))),
        ("{{RECONNECTING}}", escape(words::tv_reconnecting(lang))),
        ("{{LEFT}}", escape(words::tv_left(lang))),
        ("{{ROUND}}", escape(words::tv_round(lang))),
        ("{{NICE}}", escape(words::tv_nice(lang))),
        ("{{TOTAL}}", escape(words::tv_total(lang))),
        ("{{PARTS}}", escape(words::tv_parts(lang))),
        ("{{IN}}", escape(words::tv_in(lang))),
        ("{{HINT}}", escape(words::tv_alternating_hint(lang))),
        ("{{KEEP}}", escape(words::tv_keep_going(lang))),
        ("{{KINDS}}", escape(&kind_names(lang))),
    ]
    .iter()
    .fold(PAGE.to_owned(), |page, (key, value)| page.replace(key, value))
}

/// On the local network: loopback, private, link-local or unique-local addresses.
pub fn is_lan(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => a.is_loopback() || a.is_private() || a.is_link_local(),
        IpAddr::V6(a) => {
            let first = a.segments()[0];
            a.is_loopback() || (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80
        }
    }
}

/// A one-ride join code for the listen-only hub: 8 characters, random per ride.
pub fn ride_token() -> String {
    (0..TOKEN_LEN)
        .map(|_| {
            // `RandomState` is seeded from the OS per instance: enough for a one-ride, listen-only code
            let n = std::collections::hash_map::RandomState::new().build_hasher().finish();
            TOKEN_ALPHABET[(n % TOKEN_ALPHABET.len() as u64) as usize] as char
        })
        .collect()
}

/// What guests of the ride hub may do: listen. Adding is capped at zero songs, nobody skips, pauses or edits.
pub fn listen_permissions() -> Permissions {
    Permissions {
        add: AddMode::Limit,
        max_per_guest: 0,
        skip: SkipMode::Nobody,
        playback: false,
        edit: EditMode::None,
        ..Permissions::default()
    }
}

#[cfg(feature = "share")]
mod server;
#[cfg(feature = "share")]
pub use server::{discover, ride_hub_config, RideTvEndpoint, TvConfig, TvServer, RIDE_SERVICE_TYPE};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exercise::builder::{test_plan, CueSettings};
    use crate::exercise::model::{Position, Segment, SegmentKind, Workout};
    use crate::exercise::ride::RideClock;

    fn view() -> RideView {
        let workout = Workout {
            segments: vec![
                Segment { kind: SegmentKind::WarmUp, seconds: 120, ..Segment::default() },
                Segment {
                    kind: SegmentKind::StandingClimb,
                    seconds: 180,
                    position: Position::Standing,
                    ..Segment::default()
                },
            ],
            ..Workout::default()
        };
        RideClock::view(&test_plan(workout, CueSettings::default()), 0, 5_000, 1_000)
    }

    fn session() -> TvSession {
        TvSession {
            title: "Easy ride".into(),
            profile: vec![ProfilePoint { t_s: 0, elevation: 0.0 }, ProfilePoint { t_s: 300, elevation: 1.0 }],
            parts: vec![TvPart { kind: SegmentKind::WarmUp, start_s: 0, seconds: 300 }],
            audio: TvAudio::Tv,
            listen: Some(RideListen { port: 47802, code: "K7M2QXPA".into(), name: "Ride".into() }),
            pulse_style: PulseStyle::Bar,
            course_look: CourseLook::Alpine,
            course_summits: true,
            hot_points: vec![HotPoint { t_s: 100, effort: 7, level: 2, count: 3 }],
            course_rider: CourseRider::Dot,
        }
    }

    #[test]
    fn the_json_shape_is_the_contract() {
        let frame = TvFrame { view: view(), playing: true, sampled_ms: 900 };
        let tv = RideTvView::build(&session(), Some(&frame), 1_000, Lang::En, true);
        let json: serde_json::Value = serde_json::from_str(&tv.to_json()).unwrap();
        assert_eq!(json["v"], 1);
        for key in ["server_now_ms", "sampled_ms", "playing", "title", "audio", "listen", "profile", "ride", "text"] {
            assert!(json.get(key).is_some(), "missing {key}");
        }
        assert_eq!(json["audio"], "tv");
        assert_eq!(json["pulse_style"], "bar");
        assert_eq!((json["course_look"].as_str(), json["course_summits"].as_bool()), (Some("alpine"), Some(true)));
        assert_eq!((json["paused"].as_bool(), json["countdown_s"].as_u64()), (Some(false), Some(0)));
        assert_eq!(json["listen"]["code"], "K7M2QXPA");
        assert_eq!(json["text"]["next"], "Standing climb · 87 RPM");
        assert_eq!(json["text"]["position"], "Seated");
        let ride = &json["ride"];
        for key in [
            "target_rpm",
            "remaining_s",
            "segment_seconds",
            "segment_start_s",
            "marker",
            "pulse",
            "effort",
            "notice",
            "finished",
        ] {
            assert!(ride.get(key).is_some(), "missing ride.{key}");
        }
        assert_eq!(ride["pulse"]["source"], "metronome");
        assert_eq!(ride["position"]["kind"], "seated");
        assert_eq!(RideTvView::from_json(&tv.to_json()).unwrap(), tv);
        // added without a new protocol version
        assert_eq!((json["course_rider"].as_str(), json["hot_points"][0]["level"].as_u64()), (Some("dot"), Some(2)));
        assert_eq!((json["text"]["unit"].as_str(), json["text"]["step"].as_str()), (Some("RPM"), Some("")));
        assert!(ride.get("shown").is_some() && ride.get("level").is_some() && ride.get("step").is_some());
    }

    #[test]
    fn the_words_follow_the_bike_and_the_step() {
        use crate::exercise::bike::{BikeProfile, ConsoleUnit};
        use crate::exercise::model::{Effort, EffortStep};
        let climb = Segment {
            kind: SegmentKind::SeatedClimb,
            seconds: 300,
            effort: Effort(6),
            steps: vec![EffortStep { at_s: 100, effort: Effort(7) }],
            ..Segment::default()
        };
        let workout = Workout { segments: vec![climb, Segment::default()], ..Workout::default() };
        let plan = test_plan(workout, CueSettings::default());
        let bike = BikeProfile { console: ConsoleUnit::Kph, kph_per_rpm: 0.2, ..BikeProfile::default() };
        let tv = |ms, lang| {
            let frame = TvFrame { view: RideClock::view_on(&plan, &bike, 0, ms, 0), playing: true, sampled_ms: 0 };
            RideTvView::build(&session(), Some(&frame), 0, lang, true).text
        };
        let before = tv(10_000, Lang::En);
        assert_eq!((before.unit.as_str(), before.step.as_str()), ("km/h", ""));
        assert_eq!(before.effort, "Steady 6 · level 5/8");
        assert_eq!(before.next, "Flat · 17.4 km/h");
        let after = tv(150_000, Lang::Es);
        assert_eq!((after.step.as_str(), after.effort.as_str()), ("Resistencia +1 · 2 de 2", "Duro 7 · nivel 6/8"));
    }

    #[test]
    fn paused_and_countdown_follow_the_frame() {
        use crate::exercise::ride::Cue;
        let mut v = view();
        v.cues = vec![
            Cue { id: 1, kind: CueKind::Countdown2, in_ms: -300 },
            Cue { id: 2, kind: CueKind::Countdown1, in_ms: 700 },
        ];
        let paused = RideTvView::build(
            &session(),
            Some(&TvFrame { view: v.clone(), playing: false, sampled_ms: 1 }),
            2,
            Lang::En,
            true,
        );
        assert!(paused.paused);
        assert_eq!(paused.countdown_s, 2, "the cue that came due, not the one still ahead");
        let live = RideTvView::build(
            &session(),
            Some(&TvFrame { view: view(), playing: true, sampled_ms: 1 }),
            2,
            Lang::En,
            true,
        );
        assert!(!live.paused && live.countdown_s == 0);
        assert!(!RideTvView::build(&session(), None, 2, Lang::En, true).paused, "no ride is not a paused ride");
        let mut done = view();
        done.finished = true;
        assert!(
            !RideTvView::build(
                &session(),
                Some(&TvFrame { view: done, playing: false, sampled_ms: 1 }),
                2,
                Lang::En,
                true
            )
            .paused
        );
    }

    #[test]
    fn idle_and_the_listen_rules() {
        let idle = RideTvView::build(&session(), None, 5, Lang::Es, true);
        assert!(idle.ride.is_none() && idle.v == 1 && idle.server_now_ms == 5);
        // not on the LAN, or audio on the phone: no way to join the hub
        assert!(RideTvView::build(&session(), None, 5, Lang::En, false).listen.is_none());
        let phone = TvSession { audio: TvAudio::Phone, ..session() };
        let v = RideTvView::build(&phone, None, 5, Lang::En, true);
        assert!(v.listen.is_none() && v.audio == TvAudio::Phone);
    }

    #[test]
    fn a_client_ignores_new_fields_and_refuses_a_newer_protocol() {
        let ok = RideTvView::from_json(r#"{"v":1,"server_now_ms":7,"future":{"x":1}}"#).unwrap();
        assert_eq!(ok.server_now_ms, 7);
        assert!(RideTvView::from_json(r#"{"v":2}"#).is_err());
        assert!(RideTvView::from_json("nope").is_err());
        assert_eq!(RideTvView::from_json("{}").unwrap().v, 1);
    }

    #[test]
    fn the_page_escapes_what_it_inserts() {
        let page = render_page("<script>alert(1)</script> & \"x\"", Lang::Es);
        assert!(!page.contains("<script>alert(1)"));
        assert!(page.contains("&lt;script&gt;alert(1)&lt;/script&gt; &amp; &quot;x&quot;"));
        assert!(page.contains("lang=\"es\"") && page.contains("Esperando el paseo"));
        assert!(!page.contains("{{"), "every placeholder is filled");
        assert!(render_page("  ", Lang::En).contains("<title>HomeFi</title>"));
        assert!(page.contains("/ride.json"));
        assert!(page.contains("homefiSongPositionMs"), "the hook for a TV that plays the audio");
        assert!(page.contains("textContent") && !page.contains("innerHTML = frame.lyrics"), "lyrics only as text");
        assert!(render_page("x", Lang::En).contains("Reconnecting…") && page.contains("En pausa"));
    }

    fn lrc(lines: usize) -> crate::lyrics::Lyrics {
        let text: String = (0..lines).map(|i| format!("[{:02}:{:02}.00] line {i}\n", i / 60, i % 60)).collect();
        crate::lyrics::Lyrics::parse(&text, None).unwrap()
    }

    #[test]
    fn lyrics_round_trip_and_are_optional() {
        let frame = TvFrame { view: view(), playing: true, sampled_ms: 900 };
        let mut tv = RideTvView::build(&session(), Some(&frame), 1_000, Lang::En, true);
        assert!(tv.lyrics.is_none());
        let json: serde_json::Value = serde_json::from_str(&tv.to_json()).unwrap();
        assert!(json["lyrics"].is_null(), "no lyrics: null, the key stays");
        tv.lyrics = TvLyrics::new("tidal:1", &lrc(3), -120, 200_000);
        let back = RideTvView::from_json(&tv.to_json()).unwrap();
        assert_eq!(back, tv);
        let lyrics = back.lyrics.unwrap();
        assert_eq!((lyrics.song_key.as_str(), lyrics.synced, lyrics.offset_ms), ("tidal:1", true, -120));
        assert_eq!(lyrics.lines[1], TvLyricLine { t_ms: 1_000, text: "line 1".into() });
        // a server from before lyrics: the key is missing
        let old = RideTvView::from_json(r#"{"v":1,"server_now_ms":7,"ride":null}"#).unwrap();
        assert!(old.lyrics.is_none());
    }

    #[test]
    fn lyrics_are_capped_and_plain_ones_flagged() {
        let big = TvLyrics::new("k", &lrc(1000), 0, 0).unwrap();
        assert_eq!(big.lines.len(), MAX_TV_LYRIC_LINES);
        let wide = crate::lyrics::Lyrics::parse(&format!("{}\n", "word ".repeat(30)).repeat(900), None).unwrap();
        let capped = TvLyrics::new("k", &wide, 0, 0).unwrap();
        let bytes: usize = capped.lines.iter().map(|l| l.text.len()).sum();
        assert!(bytes <= MAX_TV_LYRICS_BYTES && capped.lines.len() < wide.lines.len());
        assert!(capped.lines.iter().all(|l| l.text.chars().count() <= 160));
        let plain = crate::lyrics::Lyrics::parse("one\n\n\n\ntwo\n\n", None).unwrap();
        let tv = TvLyrics::new("k", &plain, 0, 0).unwrap();
        assert!(!tv.synced);
        assert_eq!(tv.lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(), ["one", "", "two"]);
        let blank = crate::lyrics::Lyrics { lines: vec![], synced: false, provider: None };
        assert!(TvLyrics::new("k", &blank, 0, 0).is_none());
    }

    #[test]
    fn lan_addresses_and_one_ride_tokens() {
        for ip in ["127.0.0.1", "192.168.1.20", "10.0.0.5", "172.16.3.1", "169.254.1.1", "::1", "fe80::1", "fd00::2"] {
            assert!(is_lan(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["8.8.8.8", "172.32.0.1", "2001:4860::1"] {
            assert!(!is_lan(ip.parse().unwrap()), "{ip}");
        }
        let (a, b) = (ride_token(), ride_token());
        assert_eq!(a.len(), 8);
        assert!(a.bytes().all(|c| TOKEN_ALPHABET.contains(&c)));
        assert_ne!(a, b);
    }

    #[test]
    fn the_ride_hub_only_lets_guests_listen() {
        use crate::share::ShareTrack;
        let p = listen_permissions();
        assert!(p.check_add(&ShareTrack::default(), 0).is_err());
        assert!(p.check_skip().is_err() && p.check_playback().is_err());
        assert_eq!(p.edit, EditMode::None);
    }
}
