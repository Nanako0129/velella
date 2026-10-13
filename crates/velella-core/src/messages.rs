//! Message buffers (wave-1 M fills this file). They live outside `State`, which is copied on
//! write. The session calls `observe` with every dispatch right after `State::apply`, once
//! `SessionHandle::messages()` has created the store.
use crate::rest::Rest;
use serde_json::Value;

pub(crate) struct MessageStore {
    #[allow(dead_code)] // M's history fetches go through it.
    rest: Rest,
    /// Dispatch names seen, so a test can show the observer is wired.
    #[cfg(test)]
    pub(crate) seen: std::sync::Mutex<Vec<String>>,
}

impl MessageStore {
    pub(crate) fn new(rest: Rest) -> MessageStore {
        MessageStore {
            rest,
            #[cfg(test)]
            seen: Default::default(),
        }
    }

    /// Runs on the session task for every dispatch; must not block.
    pub(crate) fn observe(&self, name: &str, d: &Value) {
        let _ = (name, d);
        #[cfg(test)]
        self.seen.lock().unwrap().push(name.to_owned());
    }
}
