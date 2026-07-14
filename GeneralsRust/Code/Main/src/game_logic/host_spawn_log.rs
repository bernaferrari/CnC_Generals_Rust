//! Frame-local host spawn log for GameWorld shadow parity.
//!
//! `GameLogic::create_object` records successful spawns. Shadow session sync
//! already maps new ObjectIds; the log is the honesty signal that spawn went
//! through a drainable channel (future: Spawn WorldMutation).

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq)]
pub struct HostSpawnEvent {
    pub id: ObjectId,
    pub template: String,
    pub team_ordinal: u8,
    pub position: [f32; 3],
}

thread_local! {
    static LOG: RefCell<Vec<HostSpawnEvent>> = RefCell::new(Vec::new());
}

pub fn record(id: ObjectId, template: String, team_ordinal: u8, position: [f32; 3]) {
    LOG.with(|log| {
        log.borrow_mut().push(HostSpawnEvent {
            id,
            template,
            team_ordinal,
            position,
        });
    });
}

pub fn drain() -> Vec<HostSpawnEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}

pub fn len() -> usize {
    LOG.with(|log| log.borrow().len())
}
