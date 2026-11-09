//! The file a ride or a DJ setup is exported to (`.homefiride`, `.homefidj`; both plain JSON), one
//! envelope for every kind so the apps share one reader and one set of humane errors:
//!
//! ```json
//! { "homefi_export": 1, "kind": "ride", "app_version": "1.6.0", "exported_ms": 1793640000000, "data": { ... } }
//! ```
//!
//! What goes into `data` never holds tokens, API keys, account data or absolute local paths; the
//! callers (`exercise::library`, `dj::setups`) strip those before wrapping.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::humane::{language, Lang};

/// Version of the envelope. A file with a bigger number was written by a newer HomeFi.
pub const EXPORT_FORMAT_VERSION: u32 = 1;
/// A file this big is not ours: refused before it is parsed.
pub const MAX_EXPORT_BYTES: usize = 4 * 1024 * 1024;
pub const RIDE_EXTENSION: &str = "homefiride";
pub const DJ_SETUP_EXTENSION: &str = "homefidj";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportKind {
    Ride,
    DjSetup,
}

impl ExportKind {
    /// The file extension, without the dot.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Ride => RIDE_EXTENSION,
            Self::DjSetup => DJ_SETUP_EXTENSION,
        }
    }

    fn noun(self, lang: Lang) -> &'static str {
        match (self, lang) {
            (Self::Ride, Lang::En) => "ride",
            (Self::Ride, Lang::Es) => "recorrido",
            (Self::DjSetup, Lang::En) => "DJ setup",
            (Self::DjSetup, Lang::Es) => "configuración de DJ",
        }
    }
}

/// What every export file starts with: read first, so the kind and version are checked before `data`.
#[derive(Debug, Deserialize)]
struct Header {
    homefi_export: u32,
    kind: serde_json::Value,
}

#[derive(Serialize)]
struct Envelope<'a, T: Serialize> {
    homefi_export: u32,
    kind: ExportKind,
    app_version: &'a str,
    exported_ms: u64,
    data: &'a T,
}

#[derive(Deserialize)]
struct Body<T> {
    data: T,
}

fn fail(text: String) -> Error {
    Error::Export(text)
}

fn say(lang: Lang, en: String, es: String) -> String {
    match lang {
        Lang::En => en,
        Lang::Es => es,
    }
}

/// Pretty JSON of `data` inside the envelope.
pub fn wrap<T: Serialize>(kind: ExportKind, data: &T) -> Result<String> {
    let envelope = Envelope {
        homefi_export: EXPORT_FORMAT_VERSION,
        kind,
        app_version: env!("CARGO_PKG_VERSION"),
        exported_ms: crate::playlist::now_ms(),
        data,
    };
    Ok(serde_json::to_string_pretty(&envelope)?)
}

/// The `data` of an export file of `kind`. Errors say in plain words what is wrong: not a HomeFi
/// file, another kind of file, or one from a newer HomeFi.
pub fn unwrap<T: DeserializeOwned>(kind: ExportKind, text: &str) -> Result<T> {
    unwrap_in(language(), kind, text)
}

pub(crate) fn unwrap_in<T: DeserializeOwned>(lang: Lang, kind: ExportKind, text: &str) -> Result<T> {
    let wanted = kind.noun(lang);
    if text.len() > MAX_EXPORT_BYTES {
        return Err(fail(say(
            lang,
            format!("This file is too big to be a HomeFi {wanted}."),
            format!("Este archivo es demasiado grande para ser de HomeFi ({wanted})."),
        )));
    }
    let not_ours = || {
        fail(say(
            lang,
            format!("This is not a HomeFi {wanted} file."),
            format!("Este no es un archivo de HomeFi de tipo «{wanted}»."),
        ))
    };
    let header: Header = serde_json::from_str(text).map_err(|_| not_ours())?;
    if header.homefi_export > EXPORT_FORMAT_VERSION {
        return Err(fail(say(
            lang,
            "This file was made by a newer HomeFi. Update the app to open it.".into(),
            "Este archivo lo hizo una versión más nueva de HomeFi. Actualiza la app para abrirlo.".into(),
        )));
    }
    let found = serde_json::from_value::<ExportKind>(header.kind).ok();
    match found {
        Some(k) if k == kind => {}
        Some(other) => {
            let other = other.noun(lang);
            return Err(fail(say(
                lang,
                format!("This file is a HomeFi {other}, not a {wanted}."),
                format!("Este archivo es de tipo «{other}», no «{wanted}»."),
            )));
        }
        None => return Err(not_ours()),
    }
    let body: Body<T> = serde_json::from_str(text).map_err(|_| {
        fail(say(
            lang,
            format!("This HomeFi {wanted} file is damaged and can't be read."),
            format!("Este archivo de HomeFi ({wanted}) está dañado y no se puede leer."),
        ))
    })?;
    Ok(body.data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_and_unwrap_round_trip() {
        let text = wrap(ExportKind::Ride, &vec![1, 2, 3]).unwrap();
        assert!(text.contains("\"homefi_export\": 1") && text.contains("\"kind\": \"ride\""));
        assert!(text.contains("\"app_version\""));
        assert_eq!(unwrap::<Vec<i32>>(ExportKind::Ride, &text).unwrap(), [1, 2, 3]);
    }

    #[test]
    fn mistakes_get_humane_words() {
        let text = wrap(ExportKind::Ride, &1).unwrap();
        let wrong = unwrap_in::<i32>(Lang::En, ExportKind::DjSetup, &text).unwrap_err().to_string();
        assert!(wrong.contains("ride") && wrong.contains("DJ setup"), "{wrong}");
        let junk = unwrap_in::<i32>(Lang::Es, ExportKind::Ride, "hola").unwrap_err().to_string();
        assert!(junk.contains("Este no es un archivo"), "{junk}");
        let newer = r#"{"homefi_export": 9, "kind": "ride", "data": 1}"#;
        let newer = unwrap_in::<i32>(Lang::En, ExportKind::Ride, newer).unwrap_err().to_string();
        assert!(newer.contains("newer HomeFi"));
        let unknown = r#"{"homefi_export": 1, "kind": "toaster", "data": 1}"#;
        assert!(unwrap_in::<i32>(Lang::En, ExportKind::Ride, unknown).is_err());
        let damaged = r#"{"homefi_export": 1, "kind": "ride", "data": "x"}"#;
        assert!(unwrap_in::<i32>(Lang::En, ExportKind::Ride, damaged).unwrap_err().to_string().contains("damaged"));
        let huge = " ".repeat(MAX_EXPORT_BYTES + 1);
        assert!(unwrap_in::<i32>(Lang::En, ExportKind::Ride, &huge).is_err());
    }
}
