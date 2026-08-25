//! Frame-local host disable-timer log for GameWorld SetDisableTimers parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostDisableTimersEvent {
    pub object: ObjectId,
    pub emp_until_frame: u32,
    pub hacked_until_frame: u32,
    pub paralyzed_until_frame: u32,
}

/// C++ Object::setDisabledUntil / clearDisabled MiscAudio (Object.cpp:2060-2248).
#[derive(Debug, Clone, PartialEq)]
pub struct HostDisableAudioEvent {
    pub object: ObjectId,
    pub position: [f32; 3],
    pub event_name: String,
}

thread_local! {
    static LOG: RefCell<Vec<HostDisableTimersEvent>> = RefCell::new(Vec::new());
    static AUDIO: RefCell<Vec<HostDisableAudioEvent>> = const { RefCell::new(Vec::new()) };
}

pub fn record(
    object: ObjectId,
    emp_until_frame: u32,
    hacked_until_frame: u32,
    paralyzed_until_frame: u32,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostDisableTimersEvent {
            object,
            emp_until_frame,
            hacked_until_frame,
            paralyzed_until_frame,
        });
    });
}

pub fn record_audio(object: ObjectId, position: [f32; 3], event_name: impl Into<String>) {
    let event_name = event_name.into();
    if event_name.is_empty() {
        return;
    }
    AUDIO.with(|log| {
        log.borrow_mut().push(HostDisableAudioEvent {
            object,
            position,
            event_name,
        });
    });
}

/// Presentation-only drain.
pub fn take_audio() -> Vec<HostDisableAudioEvent> {
    AUDIO.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn has_pending(object: ObjectId) -> bool {
    LOG.with(|log| log.borrow().iter().any(|e| e.object == object))
}

pub fn drain() -> Vec<HostDisableTimersEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
    AUDIO.with(|log| log.borrow_mut().clear());
}

pub fn len() -> usize {
    LOG.with(|log| log.borrow().len())
}
