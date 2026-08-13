//! Frame-local host player sciences / alive log for GameWorld parity.

use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostPlayerMetaEvent {
    Sciences {
        player_id: u32,
        unlocked_sciences: Vec<String>,
    },
    Alive {
        player_id: u32,
        is_alive: bool,
    },
}

thread_local! {
    static LOG: RefCell<Vec<HostPlayerMetaEvent>> = RefCell::new(Vec::new());
}

pub fn record_sciences(player_id: u32, unlocked_sciences: impl IntoIterator<Item = String>) {
    let mut v: Vec<String> = unlocked_sciences.into_iter().collect();
    v.sort();
    LOG.with(|log| {
        log.borrow_mut().push(HostPlayerMetaEvent::Sciences {
            player_id,
            unlocked_sciences: v,
        });
    });
}

pub fn record_alive(player_id: u32, is_alive: bool) {
    LOG.with(|log| {
        log.borrow_mut().push(HostPlayerMetaEvent::Alive {
            player_id,
            is_alive,
        });
    });
}

pub fn drain() -> Vec<HostPlayerMetaEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

/// Move the queue out for a whole-world staging boundary without dropping the
/// active world's pending presentation mutations.
pub(crate) fn take_for_world_stage() -> Vec<HostPlayerMetaEvent> {
    drain()
}

/// Restore or install the queue owned by a whole-world staging boundary.
pub(crate) fn replace_for_world_stage(next: Vec<HostPlayerMetaEvent>) -> Vec<HostPlayerMetaEvent> {
    LOG.with(|log| std::mem::replace(&mut *log.borrow_mut(), next))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}

pub fn len() -> usize {
    LOG.with(|log| log.borrow().len())
}
