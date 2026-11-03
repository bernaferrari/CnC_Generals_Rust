//! Frame-local host destroy log for GameWorld shadow parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostDestroyEvent {
    pub id: ObjectId,
}

struct HostDestroyLogLogBag {
    value: std::sync::Mutex<Vec<HostDestroyEvent>>,
}
static LOG: std::sync::LazyLock<HostDestroyLogLogBag> = std::sync::LazyLock::new(|| HostDestroyLogLogBag {
    value: std::sync::Mutex::new(Vec::new()),
});


pub fn record(id: ObjectId) {
    { let mut log = LOG.value.lock().unwrap_or_else(|p| p.into_inner()); log.push(HostDestroyEvent { id }) };
}

pub fn drain() -> Vec<HostDestroyEvent> {
    { let mut log = LOG.value.lock().unwrap_or_else(|p| p.into_inner()); std::mem::take(&mut log) }
}

pub fn clear() {
    { let mut log = LOG.value.lock().unwrap_or_else(|p| p.into_inner()); log.clear() };
}
