//! Who is typing in the open channel (wave 3, track R). Pure bookkeeping: every method takes the
//! current time, so tests drive a clock they own and nothing here sleeps or reads the system time.
//!
//! An entry lives `TTL` after its user's last TYPING_START (Discord clients re-send about every
//! 8 s while a user types) and ends sooner when that user's message arrives. Names are resolved
//! by the caller when the event arrives.
use std::time::{Duration, Instant};

/// How long one TYPING_START keeps a user listed.
pub(crate) const TTL: Duration = Duration::from_secs(10);

#[derive(Debug, Default)]
pub(crate) struct Typing {
    /// (user id, display name, time of the last event), in the order users first started typing.
    entries: Vec<(u64, String, Instant)>,
}

impl Typing {
    /// Records an event: a new user is appended, a known one keeps its place and refreshes.
    pub(crate) fn start(&mut self, user_id: u64, name: String, now: Instant) {
        self.prune(now);
        match self.entries.iter_mut().find(|e| e.0 == user_id) {
            Some(e) => (e.1, e.2) = (name, now),
            None => self.entries.push((user_id, name, now)),
        }
    }

    /// A message from `user_id` ends their typing. True if they were listed.
    pub(crate) fn remove(&mut self, user_id: u64) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.0 != user_id);
        self.entries.len() != before
    }

    /// Drops entries whose `TTL` has run out. True if any went.
    pub(crate) fn prune(&mut self, now: Instant) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| Self::live(e.2, now));
        self.entries.len() != before
    }

    /// Names of the users still typing at `now`.
    pub(crate) fn names(&self, now: Instant) -> Vec<String> {
        self.entries
            .iter()
            .filter(|e| Self::live(e.2, now))
            .map(|e| e.1.clone())
            .collect()
    }

    /// An entry is gone at exactly `TTL` after its last event.
    fn live(last: Instant, now: Instant) -> bool {
        now.saturating_duration_since(last) < TTL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(t0: Instant, ms: u64) -> Instant {
        t0 + Duration::from_millis(ms)
    }

    #[test]
    fn an_entry_expires_exactly_ten_seconds_after_its_last_event() {
        let t0 = Instant::now();
        let mut t = Typing::default();
        t.start(2, "Ann".into(), t0);
        assert_eq!(t.names(at(t0, 9_999)), ["Ann"]);
        assert!(t.names(at(t0, 10_000)).is_empty());
        assert!(!t.prune(at(t0, 9_999)));
        assert!(t.prune(at(t0, 10_000)));
        assert!(!t.prune(at(t0, 10_001)), "nothing left to drop");
    }

    #[test]
    fn a_new_event_refreshes_the_entry_and_keeps_its_place() {
        let t0 = Instant::now();
        let mut t = Typing::default();
        t.start(2, "Ann".into(), t0);
        t.start(3, "Bob".into(), at(t0, 1_000));
        t.start(2, "Ann".into(), at(t0, 8_000));
        assert_eq!(t.names(at(t0, 8_000)), ["Ann", "Bob"]);
        // Bob's last event was at 1 s: gone at 11 s; Ann's was at 8 s: here until 18 s.
        assert_eq!(t.names(at(t0, 10_999)), ["Ann", "Bob"]);
        assert_eq!(t.names(at(t0, 11_000)), ["Ann"]);
        assert!(t.names(at(t0, 18_000)).is_empty());
    }

    #[test]
    fn removal_drops_only_that_user() {
        let t0 = Instant::now();
        let mut t = Typing::default();
        t.start(2, "Ann".into(), t0);
        t.start(3, "Bob".into(), t0);
        assert!(t.remove(2));
        assert!(!t.remove(2));
        assert_eq!(t.names(t0), ["Bob"]);
    }
}
