//! Frame-local host disguise log for GameWorld SetDisguise parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostDisguiseEvent {
    pub object: ObjectId,
    /// Empty template clears disguise.
    pub template: String,
    /// 255 = none.
    pub team_ordinal: u8,
}

thread_local! {
    static LOG: RefCell<Vec<HostDisguiseEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, template: Option<String>, team_ordinal: u8) {
    LOG.with(|log| {
        log.borrow_mut().push(HostDisguiseEvent {
            object,
            template: template.unwrap_or_default(),
            team_ordinal,
        });
    });
}

pub fn drain() -> Vec<HostDisguiseEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
