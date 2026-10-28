//! Errors as people read them. [`Error`]'s `Display` is the technical text
//! (logs, the CLI); [`Error::humane`] is what a screen shows: what happened in
//! plain words and what to do, in the app's language, without URLs or codes.

use std::io::ErrorKind;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::error::Error;

/// Language of the messages on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Es,
}

static LANG: AtomicU8 = AtomicU8::new(0);

/// Sets the language of [`Error::humane`] from a code like `es`, `es-MX` or `en_US`.
pub fn set_language(code: &str) {
    let es = code.to_lowercase().starts_with("es");
    LANG.store(es as u8, Ordering::Relaxed);
}

pub fn language() -> Lang {
    if LANG.load(Ordering::Relaxed) == 1 {
        Lang::Es
    } else {
        Lang::En
    }
}

/// What kind of trouble an error is, from the user's point of view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    /// No connection at all (no network, DNS fails, Tidal unreachable).
    Offline,
    /// Connected, but the answer took too long.
    Timeout,
    /// The connection dropped in the middle.
    ConnectionLost,
    /// Tidal itself is failing (5xx).
    TidalDown,
    /// Too many requests in a short time (429).
    TooManyRequests,
    /// The Tidal session is no longer valid (401).
    SessionExpired,
    NotLoggedIn,
    /// Tidal does not have it (404).
    NotFound,
    /// Not available in the account's country.
    NotInRegion,
    /// The subscription or account does not allow it (403).
    NotAllowed,
    /// Tidal answered something the app does not understand.
    BadAnswer,
    DiskFull,
    NoPermission,
    FileMissing,
    /// Some other problem reading or writing a file.
    Storage,
    BadLink,
    /// The user's own settings (a template, the config file) are wrong.
    Settings,
    /// Already worded for people (shared control).
    Worded,
    Other,
}

impl Error {
    pub fn problem(&self) -> Problem {
        match self {
            Error::Http(e) => http_problem(e),
            Error::Api { status, sub_status, user_message } => {
                let msg = user_message.to_lowercase();
                if *status == 403
                    && (matches!(sub_status.as_str(), "4032" | "4035")
                        || ["region", "country", "territor"].iter().any(|w| msg.contains(w)))
                {
                    Problem::NotInRegion
                } else {
                    status_problem(*status, &msg)
                }
            }
            Error::Auth { status, .. } if *status >= 500 => Problem::TidalDown,
            Error::Auth { .. } => Problem::SessionExpired,
            Error::NotLoggedIn(_) => Problem::NotLoggedIn,
            Error::Decode { .. } | Error::Json(_) | Error::Stream(_) => Problem::BadAnswer,
            Error::Io(e) => match e.kind() {
                ErrorKind::StorageFull | ErrorKind::QuotaExceeded => Problem::DiskFull,
                ErrorKind::PermissionDenied | ErrorKind::ReadOnlyFilesystem => Problem::NoPermission,
                ErrorKind::NotFound => Problem::FileMissing,
                ErrorKind::TimedOut => Problem::Timeout,
                ErrorKind::ConnectionRefused
                | ErrorKind::HostUnreachable
                | ErrorKind::NetworkUnreachable
                | ErrorKind::NetworkDown => Problem::Offline,
                ErrorKind::ConnectionReset
                | ErrorKind::ConnectionAborted
                | ErrorKind::BrokenPipe
                | ErrorKind::UnexpectedEof => Problem::ConnectionLost,
                _ => Problem::Storage,
            },
            Error::Metadata { .. } | Error::Ffmpeg(_) => Problem::Storage,
            Error::Resource(_) => Problem::BadLink,
            Error::Template(_) | Error::Config(_) => Problem::Settings,
            Error::Share(_) => Problem::Worded,
            Error::AutoEq(_) => Problem::Worded,
            Error::Ai(_) => Problem::Worded,
        }
    }

    /// The message for a screen, in the language set with [`set_language`].
    pub fn humane(&self) -> String {
        self.humane_in(language())
    }

    pub fn humane_in(&self, lang: Lang) -> String {
        if let Some(words) = self.problem().words(lang) {
            return words;
        }
        let es = lang == Lang::Es;
        let say = |en: &str, sp: &str| if es { sp.to_owned() } else { en.to_owned() };
        match self.problem() {
            Problem::Storage => match self {
                Error::Ffmpeg(_) => say("Couldn't convert the video.", "No se pudo convertir el video."),
                Error::Metadata { path, .. } => {
                    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    if es {
                        format!("No se pudieron guardar los datos de «{name}».")
                    } else {
                        format!("Couldn't save the details of “{name}”.")
                    }
                }
                _ => say("Couldn't read or write the file.", "No se pudo leer o guardar el archivo."),
            },
            Problem::BadLink => say("That doesn't look like a Tidal link.", "Eso no parece un enlace de Tidal."),
            Problem::Settings => {
                let detail = match self {
                    Error::Template(m) | Error::Config(m) => m.as_str(),
                    _ => "",
                };
                if es {
                    format!("Hay un problema en tus ajustes: {detail}")
                } else {
                    format!("There's a problem in your settings: {detail}")
                }
            }
            Problem::Worded => match self {
                Error::Share(m) => m.clone(),
                Error::AutoEq(m) => m.clone(),
                Error::Ai(e) => e.humane_in(lang),
                e => e.to_string(),
            },
            _ => match self {
                // Tidal's own words are usually readable; leave out the codes
                Error::Api { user_message, .. } if !user_message.is_empty() && user_message != "Unknown API error" => {
                    if es {
                        format!("Tidal dice: {user_message}")
                    } else {
                        format!("Tidal says: {user_message}")
                    }
                }
                _ => say("Something went wrong. Try again.", "Algo salió mal. Vuelve a intentarlo."),
            },
        }
    }
}

impl Problem {
    /// The message for this kind of problem, when it needs no detail from the error.
    pub fn words(self, lang: Lang) -> Option<String> {
        let es = lang == Lang::Es;
        let say = |en: &str, sp: &str| Some(if es { sp.to_owned() } else { en.to_owned() });
        match self {
            Problem::Offline => say(
                "No internet connection. Check your Wi-Fi or data and try again.",
                "No hay conexión a internet. Revisa el Wi-Fi o los datos y vuelve a intentarlo.",
            ),
            Problem::Timeout => say(
                "Tidal is taking too long to answer. Try again in a moment.",
                "Tidal está tardando demasiado en responder. Vuelve a intentarlo en un momento.",
            ),
            Problem::ConnectionLost => {
                say("The connection dropped. Try again.", "Se cortó la conexión. Vuelve a intentarlo.")
            }
            Problem::TidalDown => say(
                "Tidal is having trouble right now. Try again in a few minutes.",
                "Tidal está teniendo problemas ahora mismo. Vuelve a intentarlo en unos minutos.",
            ),
            Problem::TooManyRequests => say(
                "Tidal asked us to slow down. Wait a minute and try again.",
                "Tidal pidió ir más despacio. Espera un minuto y vuelve a intentarlo.",
            ),
            Problem::SessionExpired => say(
                "Your Tidal session expired. Sign in again in Settings.",
                "Tu sesión de Tidal caducó. Vuelve a iniciar sesión en Ajustes.",
            ),
            Problem::NotLoggedIn => say(
                "You're not signed in to Tidal. Sign in from Settings.",
                "No has iniciado sesión en Tidal. Inicia sesión desde Ajustes.",
            ),
            Problem::NotFound => say(
                "Tidal doesn't have this anymore, or the link is wrong.",
                "Tidal ya no tiene esto, o el enlace está mal.",
            ),
            Problem::NotInRegion => {
                say("This isn't available in your country on Tidal.", "Esto no está disponible en tu país en Tidal.")
            }
            Problem::NotAllowed => say("Your Tidal account doesn't allow this.", "Tu cuenta de Tidal no permite esto."),
            Problem::BadAnswer => say(
                "Tidal sent something unexpected. Try again in a moment.",
                "Tidal respondió algo inesperado. Vuelve a intentarlo en un momento.",
            ),
            Problem::DiskFull => say(
                "There's no space left. Free some space and try again.",
                "No queda espacio. Libera algo de espacio y vuelve a intentarlo.",
            ),
            Problem::NoPermission => say(
                "HomeFi isn't allowed to use that folder. Pick another one or allow access.",
                "HomeFi no tiene permiso para usar esa carpeta. Elige otra o dale acceso.",
            ),
            Problem::FileMissing => say(
                "The file isn't there anymore. It may have been moved or deleted.",
                "El archivo ya no está. Puede que se haya movido o borrado.",
            ),
            _ => None,
        }
    }
}

/// What a failed HTTP request means for the user.
pub fn http_problem(e: &reqwest::Error) -> Problem {
    if e.is_timeout() {
        Problem::Timeout
    } else if e.is_connect() {
        Problem::Offline
    } else if e.is_decode() {
        Problem::BadAnswer
    } else if let Some(status) = e.status() {
        status_problem(status.as_u16(), "")
    } else {
        Problem::ConnectionLost
    }
}

/// A failed HTTP request in plain words (for code that has no [`Error`]).
pub fn humane_http(e: &reqwest::Error) -> String {
    let p = http_problem(e);
    p.words(language()).unwrap_or_else(|| Problem::Other.words(language()).unwrap_or_default())
}

fn status_problem(status: u16, msg: &str) -> Problem {
    match status {
        401 => Problem::SessionExpired,
        404 => Problem::NotFound,
        429 => Problem::TooManyRequests,
        403 if msg.contains("subscription") || msg.contains("not allowed") || msg.contains("permission") => {
            Problem::NotAllowed
        }
        500..=599 => Problem::TidalDown,
        _ => Problem::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn api(status: u16, sub: &str, msg: &str) -> Error {
        Error::Api { status, sub_status: sub.into(), user_message: msg.into() }
    }

    #[test]
    fn tidal_answers_become_plain_words() {
        assert_eq!(api(404, "2001", "Not found").problem(), Problem::NotFound);
        assert_eq!(api(401, "11003", "Token expired").problem(), Problem::SessionExpired);
        assert_eq!(api(429, "0", "").problem(), Problem::TooManyRequests);
        assert_eq!(api(503, "0", "").problem(), Problem::TidalDown);
        assert_eq!(api(403, "4035", "Asset is not available").problem(), Problem::NotInRegion);
        assert_eq!(api(403, "0", "Not available in your region").problem(), Problem::NotInRegion);
        assert_eq!(
            api(400, "1002", "Asset is not ready for playback").humane_in(Lang::En),
            "Tidal says: Asset is not ready for playback"
        );
    }

    #[test]
    fn files_and_language() {
        let full = Error::Io(std::io::Error::from(ErrorKind::StorageFull));
        assert_eq!(full.problem(), Problem::DiskFull);
        assert!(full.humane_in(Lang::Es).starts_with("No queda espacio"));
        let gone = Error::Io(std::io::Error::from(ErrorKind::NotFound));
        assert!(gone.humane_in(Lang::En).contains("isn't there anymore"));
        // no codes, no URLs
        let text = api(500, "999", "boom").humane_in(Lang::En);
        assert!(!text.contains("500") && !text.contains("http"));
    }

    #[tokio::test]
    async fn no_connection_says_so() {
        // nothing listens on port 9 of localhost: the connection is refused
        let e = reqwest::get("http://127.0.0.1:9/").await.unwrap_err();
        let e = Error::Http(e);
        assert_eq!(e.problem(), Problem::Offline);
        assert!(e.humane_in(Lang::En).starts_with("No internet connection"));
        assert!(!e.humane_in(Lang::En).contains("127.0.0.1"));
    }

    #[test]
    fn the_language_follows_the_device() {
        set_language("es-MX");
        assert_eq!(language(), Lang::Es);
        set_language("en_US");
        assert_eq!(language(), Lang::En);
    }
}
