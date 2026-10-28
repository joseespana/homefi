//! The shared queue and skip votes: pure logic, no I/O.
//!
//! ## Turns
//!
//! With [`QueueOrder::Turns`] the upcoming songs are interleaved one per person
//! and round (the hub's own songs count as one more person). Each person keeps
//! their own songs in the order they chose; only how people take turns is
//! computed. Within a round, whoever has waited longest goes first: a person
//! "waits since" the later of the moment their last song started and the
//! moment they added their earliest waiting song. So someone who just had a
//! song played moves to the end of the round, and a newcomer slots in after
//! the people who were already waiting.
//!
//! The order is recomputed on every change (add, remove, move, a song
//! starting, switching the order mode). The song playing now is kept apart in
//! `now_playing` and never moves.

use std::collections::{BTreeSet, HashMap};

use super::permissions::QueueOrder;
use super::protocol::{Member, QueueItem, ShareTrack};

#[derive(Debug, Clone)]
struct Entry {
    item: QueueItem,
    /// Logical clock value when it was added (or approved).
    added_at: u64,
    /// The host's "play next": stays first whatever the turns say.
    pinned: bool,
}

impl Entry {
    fn owner(&self) -> &str {
        &self.item.added_by.guest_id
    }
}

#[derive(Debug, Clone)]
pub struct SharedQueue {
    order: QueueOrder,
    now_playing: Option<QueueItem>,
    /// In play order.
    upcoming: Vec<Entry>,
    /// Logical clock: ticks on every add and every song start, so "who waited
    /// longest" does not depend on wall time.
    clock: u64,
    next_id: u64,
    /// Owner `guest_id` -> clock when their last song started.
    last_started: HashMap<String, u64>,
}

impl SharedQueue {
    pub fn new(order: QueueOrder) -> Self {
        Self { order, now_playing: None, upcoming: Vec::new(), clock: 0, next_id: 1, last_started: HashMap::new() }
    }

    pub fn order(&self) -> QueueOrder {
        self.order
    }

    /// Switching to turns reorders right away; switching to first come keeps
    /// the current order.
    pub fn set_order(&mut self, order: QueueOrder) {
        self.order = order;
        self.reorder();
    }

    pub fn now_playing(&self) -> Option<&QueueItem> {
        self.now_playing.as_ref()
    }

    /// Upcoming songs in play order.
    pub fn upcoming(&self) -> Vec<QueueItem> {
        self.upcoming.iter().map(|e| e.item.clone()).collect()
    }

    pub fn ids(&self) -> Vec<u64> {
        self.upcoming.iter().map(|e| e.item.queue_id).collect()
    }

    pub fn get(&self, queue_id: u64) -> Option<&QueueItem> {
        self.upcoming.iter().map(|e| &e.item).find(|i| i.queue_id == queue_id)
    }

    /// Songs of `guest_id` still waiting to play.
    pub fn waiting_count(&self, guest_id: &str) -> usize {
        self.upcoming.iter().filter(|e| e.owner() == guest_id).count()
    }

    /// Wraps `track` with a fresh `queue_id` without queueing it (used for
    /// songs held for approval).
    pub fn new_item(&mut self, track: ShareTrack, by: Member) -> QueueItem {
        let queue_id = self.next_id;
        self.next_id += 1;
        QueueItem { queue_id, track, added_by: by }
    }

    pub fn add(&mut self, track: ShareTrack, by: Member, next: bool) -> QueueItem {
        let item = self.new_item(track, by);
        self.insert(item.clone(), next);
        item
    }

    /// Queues an item made by [`Self::new_item`]. `next` puts it before the
    /// owner's other songs with turns, or first overall with first come.
    pub fn insert(&mut self, item: QueueItem, next: bool) {
        self.clock += 1;
        let entry = Entry { item, added_at: self.clock, pinned: false };
        let at = match (next, self.order) {
            (false, _) => self.upcoming.len(),
            (true, QueueOrder::FirstCome) => 0,
            (true, QueueOrder::Turns) => {
                self.upcoming.iter().position(|e| e.owner() == entry.owner()).unwrap_or(self.upcoming.len())
            }
        };
        self.upcoming.insert(at, entry);
        self.reorder();
    }

    /// The host's "play next": first in the queue, skipping the turns (after
    /// earlier pinned songs, so several play in the order they were pinned).
    pub fn add_pinned(&mut self, track: ShareTrack, by: Member) -> QueueItem {
        let item = self.new_item(track, by);
        self.clock += 1;
        let at = self.upcoming.iter().take_while(|e| e.pinned).count();
        self.upcoming.insert(at, Entry { item: item.clone(), added_at: self.clock, pinned: true });
        self.reorder();
        item
    }

    /// Removes every waiting song of `guest_id`; returns how many.
    pub fn remove_owner(&mut self, guest_id: &str) -> usize {
        let before = self.upcoming.len();
        self.upcoming.retain(|e| e.owner() != guest_id);
        let removed = before - self.upcoming.len();
        if removed > 0 {
            self.reorder();
        }
        removed
    }

    pub fn remove(&mut self, queue_id: u64) -> Option<QueueItem> {
        let pos = self.upcoming.iter().position(|e| e.item.queue_id == queue_id)?;
        let entry = self.upcoming.remove(pos);
        self.reorder();
        Some(entry.item)
    }

    /// Moves a song to index `to` of [`Self::upcoming`]. With turns a person's
    /// songs can only trade places among themselves (the turn order is not
    /// negotiable), so the move reorders the owner's own list: the song lands
    /// before the owner's song that was closest after `to`.
    pub fn move_to(&mut self, queue_id: u64, to: usize) -> bool {
        let Some(pos) = self.upcoming.iter().position(|e| e.item.queue_id == queue_id) else {
            return false;
        };
        let entry = self.upcoming.remove(pos);
        // `to` refers to the list as the guest saw it, i.e. including the song.
        let to = to.min(self.upcoming.len());
        let at = match self.order {
            QueueOrder::FirstCome => to,
            QueueOrder::Turns => {
                let owner = entry.owner().to_owned();
                let before = self.upcoming[..to].iter().filter(|e| e.owner() == owner).count();
                let mut own = self.upcoming.iter().enumerate().filter(|(_, e)| e.owner() == owner).map(|(i, _)| i);
                match own.nth(before) {
                    Some(i) => i,
                    None => {
                        self.upcoming.iter().rposition(|e| e.owner() == owner).map_or(self.upcoming.len(), |i| i + 1)
                    }
                }
            }
        };
        self.upcoming.insert(at, entry);
        self.reorder();
        true
    }

    /// The host started playing `queue_id` (it leaves the upcoming list). Returns
    /// `None` when it is not queued.
    pub fn start(&mut self, queue_id: u64) -> Option<&QueueItem> {
        let pos = self.upcoming.iter().position(|e| e.item.queue_id == queue_id)?;
        let entry = self.upcoming.remove(pos);
        self.set_playing(entry.item);
        self.now_playing.as_ref()
    }

    /// The host plays something that was not in the shared queue.
    pub fn play_other(&mut self, track: ShareTrack, by: Member) -> QueueItem {
        let item = self.new_item(track, by);
        self.set_playing(item.clone());
        item
    }

    pub fn stop(&mut self) {
        self.now_playing = None;
    }

    fn set_playing(&mut self, item: QueueItem) {
        self.clock += 1;
        self.last_started.insert(item.added_by.guest_id.clone(), self.clock);
        self.now_playing = Some(item);
        self.reorder();
    }

    /// Mirrors the host player's queue as-is (e.g. after the host reordered it
    /// by hand): known `queue_id`s keep their owner, anything else becomes a
    /// song of `host`. No reordering happens here, so the host's order sticks
    /// until the next change.
    pub fn replace(&mut self, items: Vec<(Option<u64>, ShareTrack)>, host: &Member) {
        let mut old: HashMap<u64, Entry> = self.upcoming.drain(..).map(|e| (e.item.queue_id, e)).collect();
        for (id, track) in items {
            match id.and_then(|id| old.remove(&id)) {
                Some(entry) => self.upcoming.push(entry),
                None => {
                    let item = self.new_item(track, host.clone());
                    self.clock += 1;
                    self.upcoming.push(Entry { item, added_at: self.clock, pinned: false });
                }
            }
        }
    }

    fn reorder(&mut self) {
        if self.order != QueueOrder::Turns || self.upcoming.len() < 2 {
            return;
        }
        // Pinned songs stay first; the rest take turns.
        let (pinned, rest): (Vec<Entry>, Vec<Entry>) = self.upcoming.drain(..).partition(|e| e.pinned);
        self.upcoming = pinned;
        // Group per owner, keeping each owner's own order.
        let mut groups: Vec<(String, Vec<Entry>)> = Vec::new();
        for entry in rest {
            match groups.iter_mut().find(|(o, _)| o == entry.owner()) {
                Some((_, list)) => list.push(entry),
                None => groups.push((entry.owner().to_owned(), vec![entry])),
            }
        }
        let waiting_since = |owner: &str, list: &[Entry]| {
            let first_add = list.iter().map(|e| e.added_at).min().unwrap_or(0);
            self.last_started.get(owner).copied().unwrap_or(0).max(first_add)
        };
        let mut keyed: Vec<(u64, String, std::vec::IntoIter<Entry>)> =
            groups.into_iter().map(|(o, list)| (waiting_since(&o, &list), o, list.into_iter())).collect();
        keyed.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));

        loop {
            let before = self.upcoming.len();
            for (_, _, list) in keyed.iter_mut() {
                if let Some(e) = list.next() {
                    self.upcoming.push(e);
                }
            }
            if self.upcoming.len() == before {
                break;
            }
        }
    }
}

/// How many `vote_skip`s skip the song: half of the connected guests, rounded
/// up (at least one).
pub fn votes_needed(connected_guests: usize) -> u32 {
    connected_guests.div_ceil(2).max(1) as u32
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoteOutcome {
    Counted {
        votes: u32,
        needed: u32,
    },
    AlreadyVoted,
    /// Enough votes: skip now (the votes were reset).
    Skip,
}

/// Votes to skip the current song. They belong to one song: a track change
/// clears them.
#[derive(Debug, Clone, Default)]
pub struct SkipVotes {
    queue_id: Option<u64>,
    voters: BTreeSet<String>,
}

impl SkipVotes {
    /// Call whenever the song playing now may have changed.
    pub fn track_changed(&mut self, queue_id: Option<u64>) {
        if self.queue_id != queue_id {
            self.queue_id = queue_id;
            self.voters.clear();
        }
    }

    pub fn count(&self) -> u32 {
        self.voters.len() as u32
    }

    pub fn vote(&mut self, guest_id: &str, connected_guests: usize) -> VoteOutcome {
        if !self.voters.insert(guest_id.to_owned()) {
            return VoteOutcome::AlreadyVoted;
        }
        self.check(connected_guests)
            .unwrap_or(VoteOutcome::Counted { votes: self.count(), needed: votes_needed(connected_guests) })
    }

    /// A guest left: drop their vote; fewer guests may now be enough to skip.
    pub fn forget(&mut self, guest_id: &str, connected_guests: usize) -> Option<VoteOutcome> {
        self.voters.remove(guest_id);
        self.check(connected_guests)
    }

    fn check(&mut self, connected_guests: usize) -> Option<VoteOutcome> {
        if !self.voters.is_empty() && self.count() >= votes_needed(connected_guests) {
            self.voters.clear();
            Some(VoteOutcome::Skip)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn who(id: &str) -> Member {
        Member { guest_id: id.into(), name: id.into(), color: String::new() }
    }

    fn song(title: &str) -> ShareTrack {
        ShareTrack { title: title.into(), ..Default::default() }
    }

    fn titles(q: &SharedQueue) -> Vec<String> {
        q.upcoming().into_iter().map(|i| i.track.title).collect()
    }

    #[test]
    fn turns_interleave_one_per_person() {
        let mut q = SharedQueue::new(QueueOrder::Turns);
        for t in ["h1", "h2", "h3"] {
            q.add(song(t), who("hub"), false);
        }
        q.add(song("a1"), who("ana"), false);
        q.add(song("a2"), who("ana"), false);
        q.add(song("b1"), who("ben"), false);
        assert_eq!(titles(&q), ["h1", "a1", "b1", "h2", "a2", "h3"]);
    }

    #[test]
    fn whoever_just_played_goes_to_the_end_of_the_round() {
        let mut q = SharedQueue::new(QueueOrder::Turns);
        let h1 = q.add(song("h1"), who("hub"), false);
        q.add(song("h2"), who("hub"), false);
        q.add(song("a1"), who("ana"), false);
        q.add(song("b1"), who("ben"), false);
        assert_eq!(titles(&q), ["h1", "a1", "b1", "h2"]);

        q.start(h1.queue_id);
        assert_eq!(q.now_playing().unwrap().track.title, "h1");
        assert_eq!(titles(&q), ["a1", "b1", "h2"]);

        // Ana's song plays; she adds another: Ben and the hub go before her.
        let a1 = q.get_by_title("a1");
        q.start(a1);
        q.add(song("a2"), who("ana"), false);
        assert_eq!(titles(&q), ["b1", "h2", "a2"]);
    }

    #[test]
    fn newcomer_waits_behind_people_already_waiting() {
        let mut q = SharedQueue::new(QueueOrder::Turns);
        q.add(song("a1"), who("ana"), false);
        q.add(song("a2"), who("ana"), false);
        q.add(song("c1"), who("cy"), false);
        assert_eq!(titles(&q), ["a1", "c1", "a2"]);
    }

    #[test]
    fn now_playing_never_moves_and_leaves_the_queue() {
        let mut q = SharedQueue::new(QueueOrder::Turns);
        let a1 = q.add(song("a1"), who("ana"), false);
        q.start(a1.queue_id);
        q.add(song("b1"), who("ben"), true);
        q.add(song("b2"), who("ben"), false);
        assert_eq!(q.now_playing().unwrap().queue_id, a1.queue_id);
        assert_eq!(titles(&q), ["b1", "b2"]);
        assert!(q.start(a1.queue_id).is_none(), "already started");
    }

    #[test]
    fn next_and_move_only_reorder_the_owners_songs_with_turns() {
        let mut q = SharedQueue::new(QueueOrder::Turns);
        q.add(song("a1"), who("ana"), false);
        q.add(song("b1"), who("ben"), false);
        q.add(song("a2"), who("ana"), false);
        q.add(song("a0"), who("ana"), true);
        assert_eq!(titles(&q), ["a0", "b1", "a1", "a2"]);

        let a2 = q.get_by_title("a2");
        assert!(q.move_to(a2, 0));
        assert_eq!(titles(&q), ["a2", "b1", "a0", "a1"]);
        let a2 = q.get_by_title("a2");
        assert!(q.move_to(a2, 10));
        assert_eq!(titles(&q), ["a0", "b1", "a1", "a2"]);
        assert!(!q.move_to(999, 0));
    }

    #[test]
    fn first_come_keeps_insertion_order() {
        let mut q = SharedQueue::new(QueueOrder::FirstCome);
        q.add(song("a1"), who("ana"), false);
        q.add(song("a2"), who("ana"), false);
        q.add(song("b1"), who("ben"), false);
        q.add(song("b0"), who("ben"), true);
        assert_eq!(titles(&q), ["b0", "a1", "a2", "b1"]);
        let b1 = q.get_by_title("b1");
        q.move_to(b1, 1);
        assert_eq!(titles(&q), ["b0", "b1", "a1", "a2"]);

        // Ana added first, so she has waited longest.
        q.set_order(QueueOrder::Turns);
        assert_eq!(titles(&q), ["a1", "b0", "a2", "b1"]);
    }

    #[test]
    fn remove_and_counts() {
        let mut q = SharedQueue::new(QueueOrder::Turns);
        let a1 = q.add(song("a1"), who("ana"), false);
        q.add(song("a2"), who("ana"), false);
        q.add(song("b1"), who("ben"), false);
        assert_eq!(q.waiting_count("ana"), 2);
        assert_eq!(q.remove(a1.queue_id).unwrap().track.title, "a1");
        assert_eq!(q.waiting_count("ana"), 1);
        assert!(q.remove(a1.queue_id).is_none());
        assert_eq!(titles(&q), ["a2", "b1"]);
    }

    #[test]
    fn replace_mirrors_the_host_order() {
        let mut q = SharedQueue::new(QueueOrder::Turns);
        let a1 = q.add(song("a1"), who("ana"), false);
        let b1 = q.add(song("b1"), who("ben"), false);
        q.replace(
            vec![(Some(b1.queue_id), song("b1")), (None, song("h1")), (Some(a1.queue_id), song("x"))],
            &who("hub"),
        );
        assert_eq!(titles(&q), ["b1", "h1", "a1"]);
        assert_eq!(q.upcoming()[1].added_by.guest_id, "hub");
        assert_eq!(q.upcoming()[2].added_by.guest_id, "ana");
        let ids = q.ids();
        assert!(ids.iter().all(|&id| id > 0) && ids[1] != a1.queue_id && ids[1] != b1.queue_id);
    }

    #[test]
    fn votes_need_half_the_guests_rounded_up() {
        assert_eq!(votes_needed(0), 1);
        assert_eq!(votes_needed(1), 1);
        assert_eq!(votes_needed(2), 1);
        assert_eq!(votes_needed(3), 2);
        assert_eq!(votes_needed(4), 2);
        assert_eq!(votes_needed(5), 3);

        let mut v = SkipVotes::default();
        v.track_changed(Some(1));
        assert_eq!(v.vote("a", 5), VoteOutcome::Counted { votes: 1, needed: 3 });
        assert_eq!(v.vote("a", 5), VoteOutcome::AlreadyVoted);
        assert_eq!(v.vote("b", 5), VoteOutcome::Counted { votes: 2, needed: 3 });
        assert_eq!(v.vote("c", 5), VoteOutcome::Skip);
        assert_eq!(v.count(), 0);
    }

    #[test]
    fn votes_reset_on_track_change_and_shrink_with_guests() {
        let mut v = SkipVotes::default();
        v.track_changed(Some(1));
        v.vote("a", 4);
        v.track_changed(Some(1));
        assert_eq!(v.count(), 1, "same track keeps votes");
        v.track_changed(Some(2));
        assert_eq!(v.count(), 0);

        v.vote("a", 5);
        v.vote("b", 5);
        // Two of five is not enough; when a non-voter leaves, two of four is.
        assert_eq!(v.forget("c", 4), Some(VoteOutcome::Skip));
        assert_eq!(v.forget("a", 4), None);
    }

    #[test]
    fn pinned_songs_skip_the_turns() {
        let mut q = SharedQueue::new(QueueOrder::Turns);
        q.add(song("a1"), who("ana"), false);
        q.add(song("b1"), who("ben"), false);
        q.add(song("h1"), who("hub"), false);
        q.add_pinned(song("now1"), who("hub"));
        q.add_pinned(song("now2"), who("hub"));
        assert_eq!(titles(&q), ["now1", "now2", "a1", "b1", "h1"]);
        // still first after more adds and after one starts
        q.add(song("a2"), who("ana"), false);
        assert_eq!(&titles(&q)[..2], ["now1", "now2"]);
        let now1 = q.get_by_title("now1");
        q.start(now1);
        assert_eq!(titles(&q)[0], "now2");
    }

    #[test]
    fn a_guest_leaving_can_take_their_songs() {
        let mut q = SharedQueue::new(QueueOrder::Turns);
        q.add(song("a1"), who("ana"), false);
        q.add(song("b1"), who("ben"), false);
        q.add(song("a2"), who("ana"), false);
        assert_eq!(q.remove_owner("ana"), 2);
        assert_eq!(titles(&q), ["b1"]);
        assert_eq!(q.remove_owner("ana"), 0);
    }

    impl SharedQueue {
        fn get_by_title(&self, title: &str) -> u64 {
            self.upcoming.iter().find(|e| e.item.track.title == title).unwrap().item.queue_id
        }
    }
}
