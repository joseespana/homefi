//! Every user-facing word of the concert feature, once, in English and Spanish.
//! The text is returned already translated, so the apps only show it.

use super::TourSong;
use crate::error::Error;
use crate::humane::Lang;
use crate::playlist::EntryTag;

/// Picks the text of the language.
pub(crate) fn say(lang: Lang, en: String, es: String) -> String {
    match lang {
        Lang::En => en,
        Lang::Es => es,
    }
}

const NOTE_SEPARATOR: &str = " · ";

pub(crate) fn played_note(lang: Lang, plays: usize, shows: usize) -> String {
    say(lang, format!("Played in {plays} of {shows} shows"), format!("Tocada en {plays} de {shows} conciertos"))
}

/// "Played in 7 of 8 shows · encore · Chris Isaak cover".
pub(crate) fn song_note(lang: Lang, song: &TourSong, shows: usize) -> String {
    let mut note = played_note(lang, song.plays, shows);
    if song.tag == EntryTag::Encore {
        note.push_str(NOTE_SEPARATOR);
        note.push_str(&say(lang, "encore".into(), "bis".into()));
    }
    if let Some(original) = song.original_artist.as_deref() {
        note.push_str(NOTE_SEPARATOR);
        note.push_str(&say(lang, format!("{original} cover"), format!("cover de {original}")));
    }
    note
}

/// The note of a song in a single show: "encore · Chris Isaak cover" (empty when it has neither).
pub(crate) fn show_song_note(lang: Lang, song: &TourSong) -> String {
    let mut parts = Vec::new();
    if song.tag == EntryTag::Encore {
        parts.push(say(lang, "encore".into(), "bis".into()));
    }
    if let Some(original) = song.original_artist.as_deref() {
        parts.push(say(lang, format!("{original} cover"), format!("cover de {original}")));
    }
    parts.join(NOTE_SEPARATOR)
}

pub(crate) fn tour_name(lang: Lang, artist: &str) -> String {
    say(lang, format!("{artist} · Tour setlist"), format!("{artist} · Setlist de la gira"))
}

pub(crate) fn show_name(_lang: Lang, artist: &str, city: &str, date: &str) -> String {
    format!("{artist} · {city}, {date}")
}

pub(crate) fn popular_name(lang: Lang, artist: &str) -> String {
    say(lang, format!("{artist} · Most played"), format!("{artist} · Lo más escuchado"))
}

pub(crate) fn tour_description(lang: Lang, shows: usize) -> String {
    say(lang, format!("From {shows} recent shows"), format!("De {shows} conciertos recientes"))
}

pub(crate) fn show_description(lang: Lang, venue: &str, city: &str) -> String {
    say(lang, format!("One show: {venue}, {city}"), format!("Un concierto: {venue}, {city}"))
}

pub(crate) fn popular_description(lang: Lang, artist: &str) -> String {
    say(
        lang,
        format!("Tidal's most played for {artist}. Not the real setlist."),
        format!("Lo más escuchado de {artist} en Tidal. No es el setlist real."),
    )
}

/// Why the result is Tidal's most played and not a real setlist.
pub(crate) enum Fallback<'a> {
    NoAi,
    NoWebSearch { provider: &'a str },
    FewShows,
    AiFailed { message: &'a str },
}

pub(crate) fn fallback_note(lang: Lang, fallback: Fallback<'_>, artist: &str) -> String {
    match fallback {
        Fallback::NoAi => say(
            lang,
            format!("No AI is set up, so these are {artist}'s most played songs on Tidal, not the real setlist."),
            format!(
                "No hay una IA configurada, así que estas son las canciones más escuchadas de {artist} en Tidal, \
                 no el setlist real."
            ),
        ),
        Fallback::NoWebSearch { provider } => say(
            lang,
            format!("{provider} can't search the web, so these are {artist}'s most played songs on Tidal, not the real setlist."),
            format!(
                "{provider} no puede buscar en la web, así que estas son las canciones más escuchadas de {artist} \
                 en Tidal, no el setlist real."
            ),
        ),
        Fallback::FewShows => say(
            lang,
            format!(
                "Your AI didn't find enough recent setlists for {artist}, so these are their most played songs on Tidal."
            ),
            format!(
                "Tu IA no encontró suficientes setlists recientes de {artist}, así que estas son sus canciones más \
                 escuchadas en Tidal."
            ),
        ),
        Fallback::AiFailed { message } => say(
            lang,
            format!("{message} These are {artist}'s most played songs on Tidal instead."),
            format!("{message} En su lugar, estas son las canciones más escuchadas de {artist} en Tidal."),
        ),
    }
}

pub(crate) fn no_tidal_note(lang: Lang) -> String {
    say(
        lang,
        "Connect Tidal to find the songs that aren't in your library.".into(),
        "Conecta Tidal para encontrar las canciones que no están en tu biblioteca.".into(),
    )
}

pub(crate) enum Problem<'a> {
    NoArtistName,
    NothingToUse,
    NoSuchArtist(&'a str),
    TooSlow,
    NoShows,
}

/// The error for a problem, already worded for people.
pub(crate) fn problem(lang: Lang, p: Problem<'_>) -> Error {
    let text = match p {
        Problem::NoArtistName => say(lang, "Type the artist's name.".into(), "Escribe el nombre del artista.".into()),
        Problem::NothingToUse => say(
            lang,
            "Connect Tidal or set up an AI with web search to build this playlist.".into(),
            "Conecta Tidal o configura una IA con búsqueda web para armar esta playlist.".into(),
        ),
        Problem::NoSuchArtist(artist) => say(
            lang,
            format!("Tidal has no artist called “{artist}”."),
            format!("Tidal no tiene un artista llamado «{artist}»."),
        ),
        Problem::TooSlow => say(
            lang,
            "Building the playlist took too long. Try again.".into(),
            "Armar la playlist tardó demasiado. Vuelve a intentarlo.".into(),
        ),
        Problem::NoShows => say(
            lang,
            "This playlist has no shows to rearrange. Search again.".into(),
            "Esta playlist no tiene conciertos para reordenar. Busca de nuevo.".into(),
        ),
    };
    Error::Playlist(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(tag: EntryTag, cover: Option<&str>) -> TourSong {
        TourSong {
            name: "X".into(),
            original_artist: cover.map(str::to_owned),
            tape: false,
            plays: 7,
            share_pct: 88,
            median_pos: 0.5,
            encore_share_pct: 0,
            tag,
        }
    }

    #[test]
    fn notes_in_both_languages() {
        let s = song(EntryTag::Encore, Some("Chris Isaak"));
        assert_eq!(song_note(Lang::En, &s, 8), "Played in 7 of 8 shows · encore · Chris Isaak cover");
        assert_eq!(song_note(Lang::Es, &s, 8), "Tocada en 7 de 8 conciertos · bis · cover de Chris Isaak");
        assert_eq!(song_note(Lang::En, &song(EntryTag::Always, None), 8), "Played in 7 of 8 shows");
        assert_eq!(show_song_note(Lang::En, &s), "encore · Chris Isaak cover");
        assert_eq!(show_song_note(Lang::Es, &song(EntryTag::Always, None)), "");
    }

    #[test]
    fn names_and_errors() {
        assert_eq!(tour_name(Lang::Es, "Korn"), "Korn · Setlist de la gira");
        assert_eq!(show_name(Lang::En, "Korn", "Mexico City", "2026-10-24"), "Korn · Mexico City, 2026-10-24");
        assert_eq!(problem(Lang::En, Problem::NoSuchArtist("Zzz")).to_string(), "Tidal has no artist called “Zzz”.");
        assert!(
            fallback_note(Lang::Es, Fallback::AiFailed { message: "Falló." }, "Korn").starts_with("Falló. En su lugar")
        );
        assert!(fallback_note(Lang::En, Fallback::NoWebSearch { provider: "Groq" }, "Korn")
            .starts_with("Groq can't search"));
    }
}
