//! Disc settings: one home for the defaults and the clamps. JSON is camelCase like the other
//! desktop sections; reading is lenient (a bad field falls back to its default) so an old or
//! hand-edited `desktop.json` never loses the whole section.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::drives::DriveId;
use crate::encode::RipFormat;
use crate::read::{C2Use, ReadMode, ReadPlan};
use crate::source::ReadSpeed;

pub const WATCH_SECONDS: std::ops::RangeInclusive<u32> = 2..=30;
pub const AGREE: std::ops::RangeInclusive<u8> = 2..=4;
pub const MAX_REREADS: std::ops::RangeInclusive<u8> = 1..=50;
pub const OFFSET_SAMPLES: std::ops::RangeInclusive<i32> = -2000..=2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OpenOnInsert {
    /// Show a notice with an "Open" button.
    #[default]
    Notice,
    /// Jump straight to the Disc screen.
    Open,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RailItem {
    #[default]
    WhenInserted,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OnUnreadable {
    /// Keep the track and mark the damaged samples in the log.
    #[default]
    Keep,
    Skip,
}

/// Where disc information may come from. Nothing is looked up online unless asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Sources {
    /// The text stored on the disc (local).
    pub cd_text: bool,
    /// "Look up discs on MusicBrainz": off until the user turns it on.
    pub musicbrainz: bool,
}

impl Default for Sources {
    fn default() -> Self {
        Sources { cd_text: true, musicbrainz: false }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CoverSource {
    /// Cover Art Archive, through MusicBrainz, when that lookup is on.
    #[default]
    Caa,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CoverSettings {
    /// Also write `cover.jpg` next to the files. Embedding follows the app's `embed_cover`.
    pub save_file: bool,
    pub source: CoverSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DiscSettings {
    /// Tell me when a disc is inserted.
    pub watch: bool,
    pub watch_seconds: u32,
    pub open_on_insert: OpenOnInsert,
    pub rail_item: RailItem,
    pub format: RipFormat,
    /// Decode-and-compare after encoding FLAC.
    pub flac_verify: bool,
    pub mode: ReadMode,
    /// Identical reads required in secure mode.
    pub agree: u8,
    pub max_rereads: u8,
    pub c2: C2Use,
    /// Read offset for drives without their own entry in `offsets`.
    pub offset_samples: i32,
    /// Read offset per drive id (people own more than one drive).
    pub offsets: BTreeMap<String, i32>,
    pub speed: ReadSpeed,
    pub on_unreadable: OnUnreadable,
    pub sources: Sources,
    pub cover: CoverSettings,
    /// Where rips go; `None` = the download folder.
    pub rip_dir: Option<PathBuf>,
    /// File template; `None` = the downloads template.
    pub template: Option<String>,
    pub write_log: bool,
    pub write_cue: bool,
    pub eject_when_done: bool,
    /// Show the "Name the songs?" form before ripping.
    pub ask_names: bool,
    pub hide_cd_only_note: bool,
}

impl Default for DiscSettings {
    fn default() -> Self {
        DiscSettings {
            watch: true,
            watch_seconds: 3,
            open_on_insert: OpenOnInsert::default(),
            rail_item: RailItem::default(),
            format: RipFormat::default(),
            flac_verify: true,
            mode: ReadMode::Secure,
            agree: 2,
            max_rereads: 20,
            c2: C2Use::Auto,
            offset_samples: 0,
            offsets: BTreeMap::new(),
            speed: ReadSpeed::Auto,
            on_unreadable: OnUnreadable::Keep,
            sources: Sources::default(),
            cover: CoverSettings::default(),
            rip_dir: None,
            template: None,
            write_log: true,
            write_cue: false,
            eject_when_done: true,
            ask_names: true,
            hide_cd_only_note: false,
        }
    }
}

impl DiscSettings {
    /// Puts every number inside its range.
    pub fn sanitized(mut self) -> Self {
        self.watch_seconds = self.watch_seconds.clamp(*WATCH_SECONDS.start(), *WATCH_SECONDS.end());
        self.agree = self.agree.clamp(*AGREE.start(), *AGREE.end());
        self.max_rereads = self.max_rereads.clamp(*MAX_REREADS.start(), *MAX_REREADS.end());
        self.offset_samples = self.offset_samples.clamp(*OFFSET_SAMPLES.start(), *OFFSET_SAMPLES.end());
        for offset in self.offsets.values_mut() {
            *offset = (*offset).clamp(*OFFSET_SAMPLES.start(), *OFFSET_SAMPLES.end());
        }
        self.format = self.format.clamped();
        self.rip_dir = self.rip_dir.filter(|dir| !dir.as_os_str().is_empty());
        self.template = self.template.filter(|t| !t.trim().is_empty());
        self
    }

    /// Read offset of one drive: its own entry, else the general one.
    pub fn offset_for(&self, drive: &DriveId) -> i32 {
        self.offsets.get(&drive.0).copied().unwrap_or(self.offset_samples)
    }

    /// The reading plan for `drive`.
    pub fn read_plan(&self, drive: &DriveId) -> ReadPlan {
        ReadPlan {
            mode: self.mode,
            offset_samples: self.offset_for(drive),
            c2: self.c2,
            max_rereads: self.max_rereads,
            speed: self.speed,
            agree: self.agree,
        }
    }
}

/// `#[serde(deserialize_with = "homefi_disc::lenient_disc")]`: unknown or ill-typed fields keep
/// their default, numbers are clamped; never fails on shape.
pub fn lenient_disc<'de, D: Deserializer<'de>>(deserializer: D) -> Result<DiscSettings, D::Error> {
    let value = Value::deserialize(deserializer)?;
    let mut settings = DiscSettings::default();
    let Some(map) = value.as_object() else {
        return Ok(settings);
    };
    macro_rules! field {
        ($key:literal, $target:expr) => {
            if let Some(parsed) = map.get($key).and_then(|v| serde_json::from_value(v.clone()).ok()) {
                $target = parsed;
            }
        };
    }
    field!("watch", settings.watch);
    field!("watchSeconds", settings.watch_seconds);
    field!("openOnInsert", settings.open_on_insert);
    field!("railItem", settings.rail_item);
    field!("format", settings.format);
    field!("flacVerify", settings.flac_verify);
    field!("mode", settings.mode);
    field!("agree", settings.agree);
    field!("maxRereads", settings.max_rereads);
    field!("c2", settings.c2);
    field!("offsetSamples", settings.offset_samples);
    field!("offsets", settings.offsets);
    field!("speed", settings.speed);
    field!("onUnreadable", settings.on_unreadable);
    field!("sources", settings.sources);
    field!("cover", settings.cover);
    field!("ripDir", settings.rip_dir);
    field!("template", settings.template);
    field!("writeLog", settings.write_log);
    field!("writeCue", settings.write_cue);
    field!("ejectWhenDone", settings.eject_when_done);
    field!("askNames", settings.ask_names);
    field!("hideCdOnlyNote", settings.hide_cd_only_note);
    Ok(settings.sanitized())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lenient(json: &str) -> DiscSettings {
        let mut de = serde_json::Deserializer::from_str(json);
        lenient_disc(&mut de).unwrap()
    }

    #[test]
    fn defaults_are_the_documented_ones() {
        let s = DiscSettings::default();
        assert!(s.watch && s.flac_verify && s.write_log && s.eject_when_done && s.ask_names);
        assert_eq!(s.format, RipFormat::Flac { level: 5 });
        assert_eq!(s.mode, ReadMode::Secure);
        assert!(!s.sources.musicbrainz, "no online lookup unless asked");
        assert!(s.sources.cd_text);
        assert_eq!((s.agree, s.max_rereads, s.watch_seconds), (2, 20, 3));
    }

    #[test]
    fn empty_and_null_give_defaults() {
        assert_eq!(lenient("{}"), DiscSettings::default());
        assert_eq!(lenient("null"), DiscSettings::default());
        assert_eq!(lenient("12"), DiscSettings::default());
    }

    #[test]
    fn unknown_format_falls_back_and_numbers_are_clamped() {
        let s = lenient(
            r#"{"format":{"kind":"wavpack"},"watchSeconds":999,"agree":0,"maxRereads":200,
                "offsetSamples":-99999,"mode":"fast","bogus":1,"watch":"yes"}"#,
        );
        assert_eq!(s.format, RipFormat::Flac { level: 5 });
        assert_eq!(s.watch_seconds, 30);
        assert_eq!(s.agree, 2);
        assert_eq!(s.max_rereads, 50);
        assert_eq!(s.offset_samples, -2000);
        assert_eq!(s.mode, ReadMode::Fast);
        assert!(s.watch, "a field of the wrong type keeps its default");
    }

    #[test]
    fn out_of_range_flac_level_is_clamped() {
        assert_eq!(lenient(r#"{"format":{"kind":"flac","level":99}}"#).format, RipFormat::Flac { level: 8 });
    }

    #[test]
    fn round_trips_through_json() {
        let mut s = DiscSettings::default();
        s.offsets.insert("disk4".into(), 6);
        s.speed = ReadSpeed::X(8);
        s.rip_dir = Some("/music/rips".into());
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"watchSeconds\""));
        assert_eq!(lenient(&json), s);
    }

    #[test]
    fn offsets_are_per_drive() {
        let mut s = DiscSettings { offset_samples: 12, ..DiscSettings::default() };
        s.offsets.insert("disk4".into(), 667);
        assert_eq!(s.offset_for(&DriveId("disk4".into())), 667);
        assert_eq!(s.offset_for(&DriveId("disk5".into())), 12);
        assert_eq!(s.read_plan(&DriveId("disk4".into())).offset_samples, 667);
    }
}
