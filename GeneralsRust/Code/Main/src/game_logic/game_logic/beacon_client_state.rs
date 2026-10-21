//! Beacon command/presentation state owned by one receiving session.
//! The host update is a migration boundary until authored native ClientUpdate
//! owns execution. Object IDs never select another session's entries.
use super::ObjectId;
use std::collections::HashMap;

#[derive(Default)]
pub(crate) struct BeaconClientState {
    entries: HashMap<ObjectId, BeaconState>,
}

#[derive(Default)]
struct BeaconState {
    hidden: bool,
    caption: Option<String>,
    last_pulse: Option<u32>,
    smoke: Option<u32>,
}

impl BeaconClientState {
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }
    pub(crate) fn remove(&mut self, id: ObjectId) {
        self.entries.remove(&id);
    }
    pub(crate) fn created(&mut self, id: ObjectId, frame: u32) {
        self.entries.entry(id).or_default().last_pulse = Some(frame);
    }
    pub(crate) fn hide(&mut self, id: ObjectId) -> Option<u32> {
        let entry = self.entries.entry(id).or_default();
        entry.hidden = true;
        entry.smoke.take()
    }
    pub(crate) fn is_hidden(&self, id: ObjectId) -> bool {
        self.entries.get(&id).is_some_and(|entry| entry.hidden)
    }
    pub(crate) fn set_caption(&mut self, id: ObjectId, caption: Option<String>) {
        self.entries.entry(id).or_default().caption = caption;
    }
    pub(crate) fn caption(&self, id: ObjectId) -> Option<&str> {
        self.entries.get(&id)?.caption.as_deref()
    }
    pub(crate) fn smoke(&self, id: ObjectId) -> Option<u32> {
        self.entries.get(&id)?.smoke
    }
    pub(crate) fn set_smoke(&mut self, id: ObjectId, smoke: u32) {
        self.entries.entry(id).or_default().smoke = Some(smoke);
    }
    pub(crate) fn pulse_due(&mut self, id: ObjectId, frame: u32, interval: u32) -> bool {
        let last = self
            .entries
            .entry(id)
            .or_default()
            .last_pulse
            .get_or_insert(frame);
        // C++ UnsignedInt deadline arithmetic and strict comparison.
        if frame > last.wrapping_add(interval) {
            *last = frame;
            true
        } else {
            false
        }
    }
}
