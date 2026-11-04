//! Disc errors: technical `Display` for logs, [`DiscError::humane`] for screens.
//! Nothing from the OS layer (device paths, sense codes) reaches the UI.

use homefi_core::humane::Lang;

use crate::drives::Media;

pub type Result<T> = std::result::Result<T, DiscError>;

#[derive(Debug, thiserror::Error)]
pub enum DiscError {
    #[error("no disc drive found")]
    NoDrive,
    #[error("no disc in the drive")]
    NoDisc,
    #[error("the disc is not an audio CD ({0:?})")]
    NotAudioCd(Media),
    #[error("the drive is busy")]
    Busy,
    #[error("the disc was removed")]
    Removed,
    #[error("no permission to read the drive: {hint}")]
    Permission { hint: String },
    #[error("read error at sector {lba}: {message}")]
    Read { lba: u32, message: String },
    #[error("track {track} has {samples} unreadable samples")]
    Unreadable { track: u8, samples: u64 },
    #[error("encoder: {0}")]
    Encode(String),
    #[error("ffmpeg is needed for this format")]
    NeedsFfmpeg,
    #[error(transparent)]
    Tag(#[from] homefi_core::Error),
    #[error("lookup: {0}")]
    Lookup(String),
    #[error("cancelled")]
    Cancelled,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// The track number is not on the disc (added to the contract, see contract-deviations.md).
    #[error("track {0} is not on the disc")]
    NoSuchTrack(u8),
    /// The track is a data track, not CD-DA (added to the contract).
    #[error("track {0} is a data track")]
    NotAudioTrack(u8),
}

impl DiscError {
    /// What a screen shows: plain words and what to do, in the app's language.
    pub fn humane(&self, lang: Lang) -> String {
        let es = lang == Lang::Es;
        let pick = |en: String, es_text: String| if es { es_text } else { en };
        match self {
            DiscError::NoDrive => pick(
                "No disc drive found. Plug in a USB drive and HomeFi will notice it.".into(),
                "No se encontró ninguna unidad de discos. Conecta una unidad USB y HomeFi la detectará.".into(),
            ),
            DiscError::NoDisc => pick("Insert an audio CD.".into(), "Inserta un CD de audio.".into()),
            DiscError::NotAudioCd(Media::Protected(_)) => pick(
                "This disc is copy-protected (CSS / AACS). HomeFi doesn't bypass copy protection, so it can't copy it."
                    .into(),
                "Este disco tiene protección contra copia (CSS / AACS). HomeFi no la salta, así que no puede copiarlo."
                    .into(),
            ),
            DiscError::NotAudioCd(Media::DataDisc { .. }) => pick(
                "This is a data disc. Open its folder to play the files it contains.".into(),
                "Este es un disco de datos. Abre su carpeta para reproducir los archivos que contiene.".into(),
            ),
            DiscError::NotAudioCd(_) => pick(
                "This disc has no audio tracks a CD player can read.".into(),
                "Este disco no tiene pistas de audio que un reproductor de CD pueda leer.".into(),
            ),
            DiscError::Busy => pick(
                "The drive is being used by something else. Close other programs that use the disc and try again."
                    .into(),
                "La unidad la está usando otra cosa. Cierra otros programas que usen el disco e inténtalo de nuevo."
                    .into(),
            ),
            DiscError::Removed => pick("The disc was removed.".into(), "Se sacó el disco.".into()),
            DiscError::Permission { hint } => {
                let tail = if hint.is_empty() { String::new() } else { format!(" {hint}") };
                pick(format!("HomeFi can't read the drive.{tail}"), format!("HomeFi no puede leer la unidad.{tail}"))
            }
            DiscError::Read { .. } => pick(
                "The drive couldn't read the disc. Clean it and try again.".into(),
                "La unidad no pudo leer el disco. Límpialo e inténtalo de nuevo.".into(),
            ),
            DiscError::Unreadable { track, samples } => pick(
                format!("Track {track} has {samples} samples HomeFi couldn't read (scratches?)."),
                format!("La pista {track} tiene {samples} muestras que HomeFi no pudo leer (¿rayones?)."),
            ),
            DiscError::Encode(_) => {
                pick("The audio couldn't be encoded.".into(), "No se pudo codificar el audio.".into())
            }
            DiscError::NeedsFfmpeg => {
                pick("Install ffmpeg to use this format.".into(), "Instala ffmpeg para usar este formato.".into())
            }
            DiscError::Tag(error) => error.humane_in(lang),
            DiscError::Lookup(_) => {
                pick("The disc couldn't be looked up online.".into(), "No se pudo buscar el disco en línea.".into())
            }
            DiscError::Cancelled => pick("Cancelled.".into(), "Cancelado.".into()),
            // Same words as every other file problem (disk full, permissions...).
            DiscError::Io(error) => homefi_core::Error::Io(std::io::Error::from(error.kind())).humane_in(lang),
            DiscError::NoSuchTrack(track) => {
                pick(format!("Track {track} is not on this disc."), format!("La pista {track} no está en este disco."))
            }
            DiscError::NotAudioTrack(track) => pick(
                format!("Track {track} is a data track, not audio."),
                format!("La pista {track} es de datos, no de audio."),
            ),
        }
    }

    /// Errors that mean the drive or disc is gone: retrying a sector won't help.
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            DiscError::NoDrive
                | DiscError::NoDisc
                | DiscError::Busy
                | DiscError::Removed
                | DiscError::Permission { .. }
                | DiscError::Cancelled
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn humane_speaks_both_languages() {
        let en = DiscError::NoDisc.humane(Lang::En);
        let es = DiscError::NoDisc.humane(Lang::Es);
        assert_ne!(en, es);
        assert!(!en.contains("NoDisc"));
    }

    #[test]
    fn protected_discs_say_why() {
        let media = Media::Protected(crate::drives::ProtectionKind::DvdCss);
        assert!(DiscError::NotAudioCd(media).humane(Lang::En).contains("copy-protected"));
    }

    #[test]
    fn fatal_errors_are_not_retried() {
        assert!(DiscError::Removed.is_fatal());
        assert!(!DiscError::Read { lba: 1, message: String::new() }.is_fatal());
    }
}
