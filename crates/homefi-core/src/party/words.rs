//! Every user-facing word of the party playlist, once, in English and Spanish.
//! The text is returned already translated, so the apps only show it.

use super::{AgeGroup, PartyGuests, PartyVibe};
use crate::concert::words::say;
use crate::error::Error;
use crate::humane::Lang;

const NAME_SEPARATOR: &str = " · ";
const MINUTES_PER_HOUR: u16 = 60;
/// Characters of the wish used as a name when the AI gave no title.
const CHAT_NAME_WISH_CHARS: usize = 40;

pub(crate) fn needs_ai(lang: Lang) -> String {
    say(
        lang,
        "Party playlists are made by your AI. Set one up in Settings → AI.".into(),
        "Las playlists de fiesta las arma tu IA. Configura una en Ajustes → IA.".into(),
    )
}

pub(crate) fn no_session(lang: Lang) -> String {
    say(lang, "The conversation was closed. Start again.".into(), "La conversación se cerró. Empieza de nuevo.".into())
}

/// Longest title or artist kept in a song wish (chars); two of them plus the sentence fit [`super::MAX_NOTES_CHARS`].
const SEED_FIELD_CHARS: usize = 100;
/// Longest album-free detail (genre) kept in a song wish (chars).
const SEED_GENRE_CHARS: usize = 40;

fn seed_text(text: &str, max: usize) -> String {
    text.trim().chars().take(max).collect::<String>().trim_end().to_owned()
}

/// "A playlist like “title” by artist (year, genre): ..." for `seed`; missing details are left out.
pub(crate) fn song_wish(seed: &super::SongSeed, lang: Lang) -> String {
    let title = seed_text(&seed.title, SEED_FIELD_CHARS);
    let artist = seed_text(&seed.artist, SEED_FIELD_CHARS);
    let genre = seed.genre.as_deref().map(|g| seed_text(g, SEED_GENRE_CHARS)).filter(|g| !g.is_empty());
    let details: Vec<String> = seed.year.map(|y| y.to_string()).into_iter().chain(genre).collect();
    let details = if details.is_empty() { String::new() } else { format!(" ({})", details.join(", ")) };
    match lang {
        Lang::En => format!("A playlist like “{title}” by {artist}{details}: same style, era and mood. Start with it."),
        Lang::Es => format!(
            "Una playlist como “{title}” de {artist}{details}: mismo estilo, época y ambiente. Empieza con ella."
        ),
    }
}

pub(crate) fn chat_limit(lang: Lang) -> String {
    say(
        lang,
        "That's plenty to go on. Build the playlist when you're ready.".into(),
        "Con eso basta. Arma la playlist cuando quieras.".into(),
    )
}

/// The turn when the AI only called tools: what it understood (`title`, when set), then the question.
pub(crate) fn empty_turn(lang: Lang, title: &str) -> String {
    let title = title.trim();
    match (lang, title.is_empty()) {
        (Lang::En, true) => "Anything else before I build it?".into(),
        (Lang::Es, true) => "¿Algo más antes de armarla?".into(),
        (Lang::En, false) => format!("Got it: “{title}”. Anything else before I build it?"),
        (Lang::Es, false) => format!("Entendido: «{title}». ¿Algo más antes de armarla?"),
    }
}

/// Errors the build can end with, worded for people.
pub(crate) enum Problem {
    NothingToUse,
    NoSongs,
    /// A chat playlist before the user said what they want.
    NothingAsked,
    /// The AI answered without a single song.
    NoProposals,
    /// Most of TIDAL's searches failed or timed out.
    TidalBusy {
        failed: usize,
        asked: usize,
    },
}

pub(crate) fn problem(lang: Lang, p: Problem) -> Error {
    let text = match p {
        Problem::NothingToUse => say(
            lang,
            "Connect TIDAL or add music folders so HomeFi can find the songs.".into(),
            "Conecta TIDAL o agrega carpetas de música para que HomeFi encuentre las canciones.".into(),
        ),
        Problem::NoSongs => say(
            lang,
            "Your AI didn't suggest any songs HomeFi could find. Try again or change the answers.".into(),
            "Tu IA no sugirió canciones que HomeFi pudiera encontrar. Inténtalo de nuevo o cambia las respuestas."
                .into(),
        ),
        Problem::NoProposals => say(
            lang,
            "Your AI answered without suggesting any songs. Try again.".into(),
            "Tu IA respondió sin sugerir canciones. Inténtalo de nuevo.".into(),
        ),
        Problem::TidalBusy { failed, asked } => say(
            lang,
            format!("TIDAL didn't answer {failed} of the {asked} searches, so the songs couldn't be checked. Try again in a minute."),
            format!("TIDAL no respondió {failed} de las {asked} búsquedas, así que no se pudieron comprobar las canciones. Inténtalo de nuevo en un minuto."),
        ),
        Problem::NothingAsked => say(
            lang,
            "Tell your AI first what you'd like to hear.".into(),
            "Primero dile a tu IA qué te gustaría escuchar.".into(),
        ),
    };
    Error::Playlist(text)
}

/// "2 h", "1 h 30 min", "45 min" (the same in both languages).
pub(crate) fn length(minutes: u16) -> String {
    let (hours, rest) = (minutes / MINUTES_PER_HOUR, minutes % MINUTES_PER_HOUR);
    match (hours, rest) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

pub(crate) fn short_note(lang: Lang, short_minutes: u64, target_minutes: u16) -> String {
    let length = length(target_minutes);
    say(
        lang,
        format!("{short_minutes} min short of {length}: not enough of the suggested songs were found."),
        format!("Faltan {short_minutes} min para {length}: no se encontraron suficientes canciones sugeridas."),
    )
}

pub(crate) fn failed_note(lang: Lang, failed: usize) -> String {
    match (lang, failed) {
        (Lang::En, 1) => "TIDAL didn't answer 1 search.".into(),
        (Lang::Es, 1) => "TIDAL no respondió 1 búsqueda.".into(),
        (Lang::En, n) => format!("TIDAL didn't answer {n} searches."),
        (Lang::Es, n) => format!("TIDAL no respondió {n} búsquedas."),
    }
}

pub(crate) fn explicit_note(lang: Lang, left_out: usize) -> String {
    match (lang, left_out) {
        (Lang::En, 1) => "1 explicit song was left out.".into(),
        (Lang::Es, 1) => "Se dejó fuera 1 canción explícita.".into(),
        (Lang::En, n) => format!("{n} explicit songs were left out."),
        (Lang::Es, n) => format!("Se dejaron fuera {n} canciones explícitas."),
    }
}

/// "Dance party · 2 h" / "Fiesta para bailar · 2 h".
pub(crate) fn name(lang: Lang, vibe: PartyVibe, minutes: u16) -> String {
    let length = length(minutes);
    match lang {
        Lang::En => format!("{} party{NAME_SEPARATOR}{length}", vibe_en(vibe)),
        Lang::Es => format!("Fiesta {}{NAME_SEPARATOR}{length}", vibe_es(vibe)),
    }
}

fn vibe_en(vibe: PartyVibe) -> &'static str {
    match vibe {
        PartyVibe::Background => "Background",
        PartyVibe::Chill => "Chill",
        PartyVibe::Dance => "Dance",
        PartyVibe::Singalong => "Sing-along",
        PartyVibe::Throwback => "Throwback",
    }
}

fn vibe_es(vibe: PartyVibe) -> &'static str {
    match vibe {
        PartyVibe::Background => "de fondo",
        PartyVibe::Chill => "relajada",
        PartyVibe::Dance => "para bailar",
        PartyVibe::Singalong => "para cantar",
        PartyVibe::Throwback => "de recuerdos",
    }
}

/// "For friends · 18–29, 30–49" / "Para amigos · 18–29, 30–49".
pub(crate) fn description(lang: Lang, guests: PartyGuests, ages: &[AgeGroup]) -> String {
    let lead = say(lang, format!("For {}", guests_word(lang, guests)), format!("Para {}", guests_word(lang, guests)));
    if ages.is_empty() {
        return lead;
    }
    let ages: Vec<&str> = ages.iter().map(|a| age_word(lang, *a)).collect();
    format!("{lead}{NAME_SEPARATOR}{}", ages.join(", "))
}

pub(crate) fn guests_word(lang: Lang, guests: PartyGuests) -> &'static str {
    match (lang, guests) {
        (Lang::En, PartyGuests::Friends) => "friends",
        (Lang::En, PartyGuests::Family) => "family",
        (Lang::En, PartyGuests::Coworkers) => "coworkers",
        (Lang::En, PartyGuests::Mixed) => "a mix of people",
        (Lang::Es, PartyGuests::Friends) => "amigos",
        (Lang::Es, PartyGuests::Family) => "familia",
        (Lang::Es, PartyGuests::Coworkers) => "compañeros de trabajo",
        (Lang::Es, PartyGuests::Mixed) => "gente de todo tipo",
    }
}

pub(crate) fn age_word(lang: Lang, age: AgeGroup) -> &'static str {
    match (lang, age) {
        (Lang::En, AgeGroup::Kids) => "kids",
        (Lang::En, AgeGroup::Teens) => "teens",
        (Lang::Es, AgeGroup::Kids) => "niños",
        (Lang::Es, AgeGroup::Teens) => "adolescentes",
        (_, AgeGroup::YoungAdults) => "18–29",
        (_, AgeGroup::Adults) => "30–49",
        (_, AgeGroup::Seniors) => "50+",
    }
}

/// "Sad 90s in English · 1 h"; without a title the start of the wish names it.
pub(crate) fn chat_name(lang: Lang, title: &str, wish: &str, minutes: u16) -> String {
    let length = length(minutes);
    let (title, wish) = (title.trim(), wish.trim());
    if !title.is_empty() {
        return format!("{title}{NAME_SEPARATOR}{length}");
    }
    if !wish.is_empty() {
        let mut head: String = wish.chars().take(CHAT_NAME_WISH_CHARS).collect();
        if wish.chars().count() > CHAT_NAME_WISH_CHARS {
            head = format!("{}…", head.trim_end());
        }
        return format!("{head}{NAME_SEPARATOR}{length}");
    }
    say(lang, format!("Playlist with AI{NAME_SEPARATOR}{length}"), format!("Playlist con IA{NAME_SEPARATOR}{length}"))
}

/// The chat's first line, fixed words. With a `wish` it recalls what was asked last time.
pub(crate) fn chat_opener_text(lang: Lang, wish: &str) -> String {
    let wish = wish.trim();
    match (wish.is_empty(), lang) {
        (true, Lang::En) => "What kind of playlist do you want today? Tell me how you feel, a decade, a language, a genre, how long…".into(),
        (true, Lang::Es) => "¿Qué tipo de playlist quieres hoy? Dime cómo te sientes, una década, un idioma, un género, cuánto debe durar…".into(),
        (false, Lang::En) => format!("Last time you asked for: “{wish}”. What do you want to change, or shall we build it again?"),
        (false, Lang::Es) => format!("La última vez pediste: “{wish}”. ¿Qué cambiamos, o la armamos de nuevo?"),
    }
}

/// Four example answers under the opener.
pub(crate) fn chat_opener_replies(lang: Lang) -> Vec<String> {
    let replies: [&str; 4] = match lang {
        Lang::En => ["I'm feeling down", "Happy and upbeat", "90s in English", "Something to focus"],
        Lang::Es => ["Estoy triste", "Contento y con energía", "De los 90 en inglés", "Para concentrarme"],
    };
    replies.map(str::to_owned).to_vec()
}

/// The three parts of an energy arc, as a note on each song.
pub(crate) enum Phase {
    WarmUp,
    Peak,
    WindDown,
}

pub(crate) fn phase(lang: Lang, phase: Phase) -> String {
    match phase {
        Phase::WarmUp => say(lang, "Warm-up".into(), "Para calentar".into()),
        Phase::Peak => say(lang, "Peak".into(), "En lo más alto".into()),
        Phase::WindDown => say(lang, "Wind-down".into(), "Para cerrar".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths() {
        assert_eq!(length(120), "2 h");
        assert_eq!(length(90), "1 h 30 min");
        assert_eq!(length(45), "45 min");
    }

    #[test]
    fn names_and_descriptions_in_both_languages() {
        assert_eq!(name(Lang::En, PartyVibe::Dance, 120), "Dance party · 2 h");
        assert_eq!(name(Lang::Es, PartyVibe::Dance, 120), "Fiesta para bailar · 2 h");
        let ages = [AgeGroup::YoungAdults, AgeGroup::Adults];
        assert_eq!(description(Lang::En, PartyGuests::Friends, &ages), "For friends · 18–29, 30–49");
        assert_eq!(description(Lang::Es, PartyGuests::Friends, &ages), "Para amigos · 18–29, 30–49");
        assert_eq!(description(Lang::En, PartyGuests::Mixed, &[]), "For a mix of people");
    }

    #[test]
    fn chat_names() {
        assert_eq!(chat_name(Lang::En, "Sad 90s", "x", 60), "Sad 90s · 1 h");
        assert_eq!(chat_name(Lang::En, " ", "Something calm", 90), "Something calm · 1 h 30 min");
        let long = "ñ".repeat(50);
        assert_eq!(chat_name(Lang::En, "", &long, 30), format!("{}… · 30 min", "ñ".repeat(40)));
        assert_eq!(chat_name(Lang::Es, "", "", 30), "Playlist con IA · 30 min");
        assert_eq!(chat_name(Lang::En, "", "", 30), "Playlist with AI · 30 min");
    }

    #[test]
    fn notes_and_errors() {
        assert_eq!(short_note(Lang::En, 20, 120), "20 min short of 2 h: not enough of the suggested songs were found.");
        assert_eq!(explicit_note(Lang::En, 3), "3 explicit songs were left out.");
        assert_eq!(explicit_note(Lang::Es, 1), "Se dejó fuera 1 canción explícita.");
        assert_eq!(
            problem(Lang::Es, Problem::NothingToUse).to_string(),
            "Conecta TIDAL o agrega carpetas de música para que HomeFi encuentre las canciones."
        );
        assert!(needs_ai(Lang::En).starts_with("Party playlists are made by your AI."));
    }
}
