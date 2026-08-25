//! Frame-local host VoiceFear log (C++ ActiveBody.cpp:624-639).
//!
//! When health crosses YELLOW_DAMAGE_PERCENT with a 25% roll, the live host
//! queues the template VoiceFear event for `GameLogic::process_audio_events`.

use super::ObjectId;
use glam::Vec3;
use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq)]
pub struct HostVoiceFearEvent {
    pub victim: ObjectId,
    pub position: Vec3,
    pub player_id: Option<u32>,
    pub event_name: String,
}

thread_local! {
    static LOG: RefCell<Vec<HostVoiceFearEvent>> = const { RefCell::new(Vec::new()) };
}

pub fn record(
    victim: ObjectId,
    position: Vec3,
    player_id: Option<u32>,
    event_name: impl Into<String>,
) {
    let event_name = event_name.into();
    if event_name.is_empty() {
        return;
    }
    LOG.with(|log| {
        log.borrow_mut().push(HostVoiceFearEvent {
            victim,
            position,
            player_id,
            event_name,
        });
    });
}

pub fn drain() -> Vec<HostVoiceFearEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_drain() {
        clear();
        record(ObjectId(3), Vec3::ZERO, Some(1), "RangerVoiceFear");
        let ev = drain();
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0].event_name, "RangerVoiceFear");
        assert!(drain().is_empty());
    }
}
