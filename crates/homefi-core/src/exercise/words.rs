//! Every user-facing word of Exercise mode, once, in English and Spanish. The text is returned already
//! translated, so the apps only show it.

use super::model::{Position, PresetId, SegmentKind};
use crate::concert::words::say;
use crate::error::Error;
use crate::humane::Lang;

/// Genres named in an automatic ride name.
const SAVED_NAME_GENRES: usize = 3;

pub(crate) fn segment_name(lang: Lang, kind: SegmentKind) -> &'static str {
    let (en, es) = match kind {
        SegmentKind::WarmUp => ("Warm-up", "Calentamiento"),
        SegmentKind::SeatedFlat => ("Flat", "Plano"),
        SegmentKind::SeatedClimb => ("Seated climb", "Subida sentado"),
        SegmentKind::StandingClimb => ("Standing climb", "Subida de pie"),
        SegmentKind::Jumps => ("Jumps", "Saltos"),
        SegmentKind::Sprint => ("Sprints", "Sprints"),
        SegmentKind::Recovery => ("Recovery", "Recuperación"),
        SegmentKind::CoolDown => ("Cool-down", "Enfriamiento"),
    };
    pick(lang, en, es)
}

pub(crate) fn position_word(lang: Lang, position: Position) -> &'static str {
    match position {
        Position::Seated => pick(lang, "Seated", "Sentado"),
        Position::Standing => pick(lang, "Standing", "De pie"),
        Position::Alternating { .. } => pick(lang, "Up and down", "Arriba y abajo"),
    }
}

/// "Comfortable 4", "Hard 7": a resistance cue, not a measurement.
pub(crate) fn effort_label(lang: Lang, effort: u8) -> String {
    let (en, es) = match effort {
        0..=2 => ("Very easy", "Muy suave"),
        3..=4 => ("Comfortable", "Cómodo"),
        5..=6 => ("Steady", "Constante"),
        7..=8 => ("Hard", "Duro"),
        _ => ("All out", "A fondo"),
    };
    format!("{} {effort}", pick(lang, en, es))
}

pub(crate) fn style_name(lang: Lang, style: PresetId) -> &'static str {
    let (en, es) = match style {
        PresetId::FirstRide => ("First ride", "Primer paseo"),
        PresetId::Easy => ("Easy ride", "Paseo suave"),
        PresetId::Classic => ("Classic ride", "Paseo clásico"),
        PresetId::Intervals => ("Intervals", "Intervalos"),
    };
    pick(lang, en, es)
}

pub(crate) fn ride_name(lang: Lang, workout_name: &str, style: PresetId, minutes: u32) -> String {
    let name = if workout_name.trim().is_empty() { style_name(lang, style) } else { workout_name.trim() };
    say(lang, format!("{name} · {minutes} min"), format!("{name} · {minutes} min"))
}

/// "30 min · Classic · Rock + Metal": the automatic name of a saved ride.
pub(crate) fn saved_ride_name(minutes: u32, style: &str, genres: &[String]) -> String {
    let mut name = format!("{minutes} min · {style}");
    if !genres.is_empty() {
        let list = genres.iter().take(SAVED_NAME_GENRES).cloned().collect::<Vec<_>>().join(" + ");
        name.push_str(&format!(" · {list}"));
    }
    name
}

/// The style as one short word for a title ("Classic").
pub(crate) fn style_short(lang: Lang, style: PresetId) -> &'static str {
    let (en, es) = match style {
        PresetId::FirstRide => ("First ride", "Primer paseo"),
        PresetId::Easy => ("Easy", "Suave"),
        PresetId::Classic => ("Classic", "Clásico"),
        PresetId::Intervals => ("Intervals", "Intervalos"),
    };
    pick(lang, en, es)
}

/// An imported ride with nothing to play.
pub(crate) fn ride_file_empty(lang: Lang) -> String {
    say(lang, "This ride has no songs to play.".into(), "Este recorrido no tiene canciones para reproducir.".into())
}

pub(crate) fn ride_file_newer(lang: Lang) -> String {
    say(
        lang,
        "This ride was saved by a newer HomeFi. Update the app to open it.".into(),
        "Este recorrido lo guardó una versión más nueva de HomeFi. Actualiza la app para abrirlo.".into(),
    )
}

pub(crate) fn ride_description(lang: Lang, genres: &[String]) -> String {
    if genres.is_empty() {
        return say(lang, "A HomeFi ride".into(), "Un paseo de HomeFi".into());
    }
    let list = genres.join(", ");
    say(lang, format!("A HomeFi ride · {list}"), format!("Un paseo de HomeFi · {list}"))
}

/// "Warm-up · 80-95 RPM · 90 BPM", the note of a playlist entry.
pub(crate) fn song_note(lang: Lang, kind: SegmentKind, min_rpm: u16, max_rpm: u16, bpm: Option<u16>) -> String {
    let mut note = format!("{} · {min_rpm}-{max_rpm} RPM", segment_name(lang, kind));
    if let Some(bpm) = bpm {
        note.push_str(&format!(" · {bpm} BPM"));
    }
    note
}

pub(crate) fn no_fit_note(lang: Lang, kind: SegmentKind) -> String {
    let name = segment_name(lang, kind);
    say(
        lang,
        format!("{name}: no song matched the cadence exactly, so the closest one plays."),
        format!("{name}: ninguna canción coincidió con la cadencia, suena la más cercana."),
    )
}

pub(crate) fn metronome_note(lang: Lang, kind: SegmentKind) -> String {
    let name = segment_name(lang, kind);
    say(
        lang,
        format!("{name}: the pulse is a metronome; this song's beat could not be read."),
        format!("{name}: el pulso es un metrónomo; no se pudo leer el ritmo de esta canción."),
    )
}

pub(crate) fn no_song_note(lang: Lang, kind: SegmentKind) -> String {
    let name = segment_name(lang, kind);
    say(lang, format!("{name}: no song was found."), format!("{name}: no se encontró ninguna canción."))
}

pub(crate) fn no_ai_note(lang: Lang) -> String {
    say(
        lang,
        "No AI is set up: the songs come from your genres, matched by tempo.".into(),
        "No hay una IA configurada: las canciones salen de tus géneros, elegidas por tempo.".into(),
    )
}

pub(crate) fn ai_failed_note(lang: Lang, message: &str) -> String {
    say(
        lang,
        format!("The AI could not pick the songs: {message} This ride comes from a plain Tidal search of your genres."),
        format!(
            "La IA no pudo elegir las canciones: {message} Esta ruta sale de una búsqueda simple en Tidal de tus géneros."
        ),
    )
}

pub(crate) fn ai_no_songs(lang: Lang) -> &'static str {
    pick(lang, "Your AI suggested no songs.", "Tu IA no sugirió canciones.")
}

pub(crate) fn health_notice(lang: Lang) -> String {
    say(
        lang,
        "HomeFi is not medical advice. Ride at a comfortable pace and stop if you feel unwell.".into(),
        "HomeFi no da consejo médico. Pedalea a un ritmo cómodo y para si te sientes mal.".into(),
    )
}

pub(crate) enum Problem {
    NothingToUse,
    NoSongs,
    TooSlow,
    NoRide,
    TvBusy(u16),
}

pub(crate) fn problem(lang: Lang, p: Problem) -> Error {
    let text = match p {
        Problem::NothingToUse => say(
            lang,
            "Connect Tidal or add music folders so HomeFi can find songs for the ride.".into(),
            "Conecta Tidal o agrega carpetas de música para que HomeFi encuentre canciones para el paseo.".into(),
        ),
        Problem::NoSongs => say(
            lang,
            "HomeFi found no songs for this ride. Try other genres.".into(),
            "HomeFi no encontró canciones para este paseo. Prueba otros géneros.".into(),
        ),
        Problem::TooSlow => say(
            lang,
            "Building the ride took too long. Try again.".into(),
            "Armar el paseo tardó demasiado. Vuelve a intentarlo.".into(),
        ),
        Problem::NoRide => say(lang, "This ride has no segments.".into(), "Este paseo no tiene segmentos.".into()),
        Problem::TvBusy(port) => say(
            lang,
            format!("Port {port} is in use. Pick another one in the Exercise settings."),
            format!("El puerto {port} está en uso. Elige otro en los ajustes de Ejercicio."),
        ),
    };
    Error::Playlist(text)
}

// ---------------------------------------------------------------- the TV page

pub(crate) fn tv_waiting(lang: Lang) -> &'static str {
    pick(lang, "Waiting for the ride", "Esperando el paseo")
}

pub(crate) fn tv_rpm(lang: Lang) -> &'static str {
    pick(lang, "RPM", "RPM")
}

pub(crate) fn tv_next(lang: Lang) -> &'static str {
    pick(lang, "Next", "Sigue")
}

pub(crate) fn tv_done(lang: Lang) -> &'static str {
    pick(lang, "Ride complete", "Paseo completo")
}

pub(crate) fn tv_work(lang: Lang) -> &'static str {
    pick(lang, "GO", "DALE")
}

pub(crate) fn tv_rest(lang: Lang) -> &'static str {
    pick(lang, "REST", "RECUPERA")
}

pub(crate) fn tv_paused(lang: Lang) -> &'static str {
    pick(lang, "Paused", "En pausa")
}

pub(crate) fn tv_reconnecting(lang: Lang) -> &'static str {
    pick(lang, "Reconnecting…", "Reconectando…")
}

pub(crate) fn tv_left(lang: Lang) -> &'static str {
    pick(lang, "left in this part", "restan de esta parte")
}

pub(crate) fn tv_round(lang: Lang) -> &'static str {
    pick(lang, "Round", "Ronda")
}

pub(crate) fn tv_nice(lang: Lang) -> &'static str {
    pick(lang, "Nice work!", "¡Buen trabajo!")
}

pub(crate) fn tv_total(lang: Lang) -> &'static str {
    pick(lang, "Total time", "Tiempo total")
}

pub(crate) fn tv_parts(lang: Lang) -> &'static str {
    pick(lang, "parts", "partes")
}

pub(crate) fn tv_in(lang: Lang) -> &'static str {
    pick(lang, "in", "en")
}

pub(crate) fn tv_keep_going(lang: Lang) -> &'static str {
    pick(lang, "Keep going", "Mantén el ritmo")
}

pub(crate) fn tv_alternating_hint(lang: Lang) -> &'static str {
    pick(lang, "Stand and sit on the beat", "Párate y siéntate al ritmo")
}

// ------------------------------------------------------------------- the chat

pub(crate) fn needs_ai(lang: Lang) -> String {
    say(
        lang,
        "Describing your ride in words needs an AI. Set one up in Settings, or pick a style below.".into(),
        "Describir tu paseo con palabras necesita una IA. Configura una en Ajustes o elige un estilo abajo.".into(),
    )
}

pub(crate) fn opener_text(lang: Lang) -> String {
    say(
        lang,
        "Tell me about your ride: how long, how hard and what music you like.".into(),
        "Cuéntame tu paseo: cuánto dura, qué tan duro y qué música te gusta.".into(),
    )
}

pub(crate) fn opener_replies(lang: Lang) -> Vec<String> {
    match lang {
        Lang::En => vec![
            "30 minutes, easy, 90s rock".into(),
            "45 minutes with sprints and metal".into(),
            "A first ride, 20 minutes".into(),
        ],
        Lang::Es => vec![
            "30 minutos, suave, rock de los 90".into(),
            "45 minutos con sprints y metal".into(),
            "Mi primer paseo, 20 minutos".into(),
        ],
    }
}

pub(crate) fn chat_limit(lang: Lang) -> String {
    say(
        lang,
        "That is enough chatting: build the ride, or change it by hand.".into(),
        "Ya conversamos bastante: arma el paseo o cámbialo a mano.".into(),
    )
}

pub(crate) fn empty_turn(lang: Lang) -> String {
    say(lang, "Done. Anything else about the ride?".into(), "Listo. ¿Algo más del paseo?".into())
}

fn pick(lang: Lang, en: &'static str, es: &'static str) -> &'static str {
    match lang {
        Lang::En => en,
        Lang::Es => es,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_in_both_languages() {
        assert_eq!(segment_name(Lang::Es, SegmentKind::StandingClimb), "Subida de pie");
        assert_eq!(effort_label(Lang::En, 7), "Hard 7");
        assert_eq!(effort_label(Lang::Es, 4), "Cómodo 4");
        assert_eq!(song_note(Lang::En, SegmentKind::WarmUp, 80, 95, Some(90)), "Warm-up · 80-95 RPM · 90 BPM");
        assert_eq!(song_note(Lang::En, SegmentKind::WarmUp, 80, 95, None), "Warm-up · 80-95 RPM");
        assert_eq!(ride_name(Lang::En, "", PresetId::Easy, 30), "Easy ride · 30 min");
        assert!(problem(Lang::Es, Problem::TvBusy(47801)).to_string().contains("47801"));
    }
}
