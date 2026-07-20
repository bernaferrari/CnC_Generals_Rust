//! Frame-local host AI mood/idle residual for GameWorld SetAiMood parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq)]
pub struct HostAiMoodEvent {
    pub object: ObjectId,
    pub idle_since_frame: u32,
    pub mood_attack_check_rate: u32,
    pub auto_acquire_when_idle: bool,
    pub attack_priority_set: String,
}

thread_local! {
    static LOG: RefCell<Vec<HostAiMoodEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    object: ObjectId,
    idle_since_frame: u32,
    mood_attack_check_rate: u32,
    auto_acquire_when_idle: bool,
    attack_priority_set: String,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostAiMoodEvent {
            object,
            idle_since_frame,
            mood_attack_check_rate,
            auto_acquire_when_idle,
            attack_priority_set,
        });
    });
}

pub fn drain() -> Vec<HostAiMoodEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
