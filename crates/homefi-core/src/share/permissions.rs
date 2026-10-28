//! What guests may do, all decided by the hub. The checks are pure so the hub
//! server, the tests and (later) the web page agree on the rules.

use serde::{Deserialize, Serialize};

use super::protocol::{Member, QueueItem, ShareTrack};

/// Who may add songs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AddMode {
    /// Anyone, any number of songs.
    #[default]
    Free,
    /// Every song waits for the host to approve it.
    Approve,
    /// At most [`Permissions::max_per_guest`] waiting songs per guest.
    Limit,
}

/// How guests' songs are ordered.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueOrder {
    /// In the order they were added.
    FirstCome,
    /// Round robin per person (the hub counts as one); see `queue::SharedQueue`.
    #[default]
    Turns,
}

/// Who may skip the current song.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipMode {
    Nobody,
    Everyone,
    /// Half of the connected guests (rounded up) must vote.
    #[default]
    Vote,
}

/// Which queued songs a guest may remove or move.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditMode {
    /// Only the ones they added.
    #[default]
    Own,
    All,
    None,
}

/// The hub's rules (the table in `docs/shared-control.md`). Defaults: add
/// freely, turns, skip by vote, no pause / volume, remove own, explicit allowed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Permissions {
    pub add: AddMode,
    /// Only used with [`AddMode::Limit`]; songs waiting in the queue (or for
    /// approval) per guest.
    pub max_per_guest: u32,
    pub order: QueueOrder,
    pub skip: SkipMode,
    /// Pause / resume and volume.
    pub playback: bool,
    /// Remove and move.
    pub edit: EditMode,
    pub allow_explicit: bool,
    /// When a guest leaves (or is kicked), their waiting songs leave too.
    pub drop_songs_on_leave: bool,
}

impl Default for Permissions {
    fn default() -> Self {
        Self {
            add: AddMode::Free,
            max_per_guest: 3,
            order: QueueOrder::Turns,
            skip: SkipMode::Vote,
            playback: false,
            edit: EditMode::Own,
            allow_explicit: true,
            drop_songs_on_leave: false,
        }
    }
}

/// What happens to an accepted `add`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddDecision {
    Queue,
    /// Hold it until the host approves.
    AskHost,
}

/// What a guest's `skip` / `vote_skip` turns into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipDecision {
    Skip,
    Vote,
}

/// Refusal texts are shown to guests as-is (`denied { reason }`).
pub type Denial = String;

impl Permissions {
    /// `waiting` = songs this guest already has in the queue or awaiting approval.
    pub fn check_add(&self, track: &ShareTrack, waiting: usize) -> Result<AddDecision, Denial> {
        if track.explicit && !self.allow_explicit {
            return Err("Explicit songs are not allowed".into());
        }
        match self.add {
            AddMode::Free => Ok(AddDecision::Queue),
            AddMode::Approve => Ok(AddDecision::AskHost),
            AddMode::Limit if waiting >= self.max_per_guest as usize => {
                Err(format!("You can have at most {} songs waiting", self.max_per_guest))
            }
            AddMode::Limit => Ok(AddDecision::Queue),
        }
    }

    /// Both `skip` and `vote_skip` go through here: a guest app may send
    /// either, the hub's mode decides.
    pub fn check_skip(&self) -> Result<SkipDecision, Denial> {
        match self.skip {
            SkipMode::Nobody => Err("Skipping is turned off".into()),
            SkipMode::Everyone => Ok(SkipDecision::Skip),
            SkipMode::Vote => Ok(SkipDecision::Vote),
        }
    }

    pub fn check_playback(&self) -> Result<(), Denial> {
        if self.playback {
            Ok(())
        } else {
            Err("Only the host can pause or change the volume".into())
        }
    }

    /// Remove or move `item` on behalf of `who`.
    pub fn check_edit(&self, who: &Member, item: &QueueItem) -> Result<(), Denial> {
        match self.edit {
            EditMode::All => Ok(()),
            EditMode::Own if item.added_by.guest_id == who.guest_id => Ok(()),
            EditMode::Own => Err("You can only change your own songs".into()),
            EditMode::None => Err("Only the host can change the queue".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(id: &str) -> Member {
        Member { guest_id: id.into(), ..Default::default() }
    }

    #[test]
    fn defaults_match_the_design() {
        let p = Permissions::default();
        assert_eq!(p.check_add(&ShareTrack::default(), 100), Ok(AddDecision::Queue));
        assert_eq!(p.order, QueueOrder::Turns);
        assert_eq!(p.check_skip(), Ok(SkipDecision::Vote));
        assert!(p.check_playback().is_err());
        let own = QueueItem { added_by: member("g1"), ..Default::default() };
        assert!(p.check_edit(&member("g1"), &own).is_ok());
        assert!(p.check_edit(&member("g2"), &own).is_err());
        assert!(p.check_add(&ShareTrack { explicit: true, ..Default::default() }, 0).is_ok());
    }

    #[test]
    fn limits_and_blocks() {
        let p = Permissions {
            add: AddMode::Limit,
            max_per_guest: 2,
            allow_explicit: false,
            skip: SkipMode::Nobody,
            edit: EditMode::None,
            ..Default::default()
        };
        let t = ShareTrack::default();
        assert_eq!(p.check_add(&t, 1), Ok(AddDecision::Queue));
        assert!(p.check_add(&t, 2).is_err());
        assert!(p.check_add(&ShareTrack { explicit: true, ..Default::default() }, 0).is_err());
        assert!(p.check_skip().is_err());
        let own = QueueItem { added_by: member("g1"), ..Default::default() };
        assert!(p.check_edit(&member("g1"), &own).is_err());

        let approve = Permissions { add: AddMode::Approve, ..Default::default() };
        assert_eq!(approve.check_add(&t, 0), Ok(AddDecision::AskHost));
    }

    #[test]
    fn parses_partial_json() {
        let p: Permissions = serde_json::from_str(r#"{"skip":"everyone","unknown":1}"#).unwrap();
        assert_eq!(p.skip, SkipMode::Everyone);
        assert_eq!(p.order, QueueOrder::Turns);
    }
}
