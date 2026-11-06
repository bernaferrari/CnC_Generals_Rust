//! Frame-local host destroy log for GameWorld shadow parity.

use super::ObjectId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostDestroyEvent {
    pub id: ObjectId,
}

fn with_log_mut<R>(f: impl FnOnce(&mut Vec<HostDestroyEvent>) -> R) -> R {
    let services = crate::main_services::main_services();
    let mut log = services
        .host_destroy_log
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    f(&mut log)
}

pub fn record(id: ObjectId) {
    with_log_mut(|log| log.push(HostDestroyEvent { id }));
}

pub fn drain() -> Vec<HostDestroyEvent> {
    with_log_mut(std::mem::take)
}

pub fn clear() {
    with_log_mut(|log| log.clear());
}
