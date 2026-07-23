//! Frame-local host FireSoundLoopTime residual log (FiringTracker audio loop).
//!
//! C++ FiringTracker::shotFired extends `m_frameToStopLoopingSound` while
//! FireSoundLoopTime > 0; when the deadline elapses the looping fire audio stops.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostFireSoundLoopEvent {
    pub unit: ObjectId,
    pub sound: String,
    /// true = start/refresh loop; false = stop after idle deadline.
    pub start: bool,
}

thread_local! {
    static LOG: RefCell<Vec<HostFireSoundLoopEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostFireSoundLoopEvent>> = RefCell::new(Vec::new());
}

pub fn record(unit: ObjectId, sound: String, start: bool) {
    LOG.with(|log| {
        log.borrow_mut()
            .push(HostFireSoundLoopEvent { unit, sound, start });
    });
}

pub fn drain() -> Vec<HostFireSoundLoopEvent> {
    let v = LOG.with(|log| std::mem::take(&mut *log.borrow_mut()));
    if !v.is_empty() {
        LAST_DRAIN.with(|last| *last.borrow_mut() = v.clone());
    }
    v
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
    LAST_DRAIN.with(|last| last.borrow_mut().clear());
}

pub fn take_last_drain() -> Vec<HostFireSoundLoopEvent> {
    LAST_DRAIN.with(|last| std::mem::take(&mut *last.borrow_mut()))
}
