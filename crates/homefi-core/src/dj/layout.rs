//! How the DJ screen is arranged: the order and visibility of its sections and which sound sits
//! on each sampler pad. Platform-neutral so a setup made on the desktop opens on a tablet: the
//! screens honor what fits (a phone shows the first four pads and may stack the sections).
//! Read leniently: unknown words are dropped or replaced by the default of their slot, never
//! failing the whole settings file.

use serde::{Deserialize, Deserializer, Serialize};

use super::{SfxKind, PAD_COUNT};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DjSectionKind {
    DeckA,
    Mixer,
    DeckB,
    Sampler,
    Crate,
}

impl DjSectionKind {
    /// Today's order on screen.
    pub const ALL: [DjSectionKind; 5] = [Self::DeckA, Self::Mixer, Self::DeckB, Self::Sampler, Self::Crate];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DjSection {
    pub kind: DjSectionKind,
    pub visible: bool,
}

impl Default for DjSection {
    fn default() -> Self {
        Self { kind: DjSectionKind::DeckA, visible: true }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DjLayout {
    /// Every section once, in screen order. Hidden ones keep their place for when they come back.
    #[serde(deserialize_with = "lenient_sections")]
    pub sections: Vec<DjSection>,
    /// The sound of each of the [`PAD_COUNT`] pads (a phone uses the first four). A sound may repeat.
    #[serde(deserialize_with = "lenient_pads")]
    pub pads: Vec<SfxKind>,
}

impl Default for DjLayout {
    /// Today's layout: every section visible, pad `i` plays `SfxKind::ALL[i]`.
    fn default() -> Self {
        Self {
            sections: DjSectionKind::ALL.iter().map(|&kind| DjSection { kind, visible: true }).collect(),
            pads: SfxKind::ALL.to_vec(),
        }
    }
}

impl DjLayout {
    /// Each section exactly once (repeats dropped, missing ones added visible at the end) and
    /// exactly [`PAD_COUNT`] pads (extra ones cut, missing ones take their default sound).
    pub fn clamped(&self) -> Self {
        let mut sections: Vec<DjSection> = Vec::with_capacity(DjSectionKind::ALL.len());
        for s in &self.sections {
            if !sections.iter().any(|x| x.kind == s.kind) {
                sections.push(*s);
            }
        }
        for kind in DjSectionKind::ALL {
            if !sections.iter().any(|x| x.kind == kind) {
                sections.push(DjSection { kind, visible: true });
            }
        }
        let pads = (0..PAD_COUNT).map(|i| self.pads.get(i).copied().unwrap_or(SfxKind::ALL[i])).collect();
        Self { sections, pads }
    }

    /// The pad sounds as the engine renders them: always [`PAD_COUNT`] of them.
    pub fn pad_sounds(&self) -> [SfxKind; PAD_COUNT] {
        let clamped = self.clamped();
        std::array::from_fn(|i| clamped.pads[i])
    }
}

/// An item that reads as `T`, or as nothing when it is not a `T`.
#[derive(Deserialize)]
#[serde(untagged)]
enum Maybe<T> {
    Known(T),
    Other(serde::de::IgnoredAny),
}

fn lenient_list<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Vec<Option<T>>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum List<T> {
        Items(Vec<Maybe<T>>),
        Other(serde::de::IgnoredAny),
    }
    Ok(match List::<T>::deserialize(d)? {
        List::Items(items) => items
            .into_iter()
            .map(|m| match m {
                Maybe::Known(t) => Some(t),
                Maybe::Other(_) => None,
            })
            .collect(),
        List::Other(_) => Vec::new(),
    })
}

fn lenient_sections<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<DjSection>, D::Error> {
    Ok(lenient_list::<D, DjSection>(d)?.into_iter().flatten().collect())
}

/// An unknown sound keeps its pad with that pad's default sound.
fn lenient_pads<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<SfxKind>, D::Error> {
    Ok(lenient_list::<D, SfxKind>(d)?
        .into_iter()
        .enumerate()
        .map(|(i, k)| k.unwrap_or_else(|| SfxKind::ALL[i.min(PAD_COUNT - 1)]))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_todays_layout() {
        let l = DjLayout::default();
        assert_eq!(l.pads, SfxKind::ALL);
        assert_eq!(l.sections.len(), 5);
        assert!(l.sections.iter().all(|s| s.visible));
        assert_eq!(l.sections[0].kind, DjSectionKind::DeckA);
        assert_eq!(l.clamped(), l);
    }

    #[test]
    fn clamping_repairs_sections_and_pads() {
        let wild = DjLayout {
            sections: vec![
                DjSection { kind: DjSectionKind::Sampler, visible: false },
                DjSection { kind: DjSectionKind::Sampler, visible: true },
                DjSection { kind: DjSectionKind::DeckB, visible: true },
            ],
            pads: vec![SfxKind::Hit; 12],
        };
        let c = wild.clamped();
        let kinds: Vec<_> = c.sections.iter().map(|s| s.kind).collect();
        assert_eq!(
            kinds,
            [
                DjSectionKind::Sampler,
                DjSectionKind::DeckB,
                DjSectionKind::DeckA,
                DjSectionKind::Mixer,
                DjSectionKind::Crate
            ]
        );
        assert!(!c.sections[0].visible, "the first of a repeat wins");
        assert_eq!(c.pads.len(), PAD_COUNT);
        let short = DjLayout { pads: vec![SfxKind::Hit], ..DjLayout::default() }.clamped();
        assert_eq!(short.pads[0], SfxKind::Hit);
        assert_eq!(short.pads[1], SfxKind::ALL[1]);
        assert_eq!(DjLayout { pads: vec![], sections: vec![] }.clamped(), DjLayout::default());
    }

    #[test]
    fn unknown_words_do_not_break_reading() {
        let json = r#"{"sections":[{"kind":"hologram","visible":true},{"kind":"mixer","visible":false},7],
                       "pads":["hit","kazoo","laser"],"future":1}"#;
        let l: DjLayout = serde_json::from_str(json).unwrap();
        assert_eq!(l.sections, [DjSection { kind: DjSectionKind::Mixer, visible: false }]);
        assert_eq!(l.pads, [SfxKind::Hit, SfxKind::ALL[1], SfxKind::Laser]);
        let l: DjLayout = serde_json::from_str(r#"{"sections": 5, "pads": "x"}"#).unwrap();
        assert_eq!(l.clamped(), DjLayout::default());
        let l: DjLayout = serde_json::from_str("{}").unwrap();
        assert_eq!(l, DjLayout::default());
    }
}
