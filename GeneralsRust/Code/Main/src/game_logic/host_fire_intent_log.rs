//! Frame-local host fire-intent log for GameWorld SetFireIntent parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostFireIntentEvent {
    pub object: ObjectId,
    pub last_fire_victim_host: u32,
    pub last_fire_slot: u8,
    pub last_fire_damage: f32,
    pub last_fire_range: f32,
    pub last_fire_sim_time: f32,
    pub last_fire_frame: u32,
    pub fire_intent_count: u32,
}

struct HostFireIntentLogLogBag {
    value: std::sync::Mutex<Vec<HostFireIntentEvent>>,
}
static LOG: std::sync::LazyLock<HostFireIntentLogLogBag> = std::sync::LazyLock::new(|| HostFireIntentLogLogBag {
    value: std::sync::Mutex::new(Vec::new()),
});


pub fn record(
    object: ObjectId,
    last_fire_victim_host: u32,
    last_fire_slot: u8,
    last_fire_damage: f32,
    last_fire_range: f32,
    last_fire_sim_time: f32,
    last_fire_frame: u32,
    fire_intent_count: u32,
) {
    { let mut log = LOG.value.lock().unwrap_or_else(|p| p.into_inner()); 
        log.push(HostFireIntentEvent {
            object,
            last_fire_victim_host,
            last_fire_slot,
            last_fire_damage,
            last_fire_range,
            last_fire_sim_time,
            last_fire_frame,
            fire_intent_count,
        });
     };
}

pub fn has_pending(object: ObjectId) -> bool {
    { let mut log = LOG.value.lock().unwrap_or_else(|p| p.into_inner()); log.iter().any(|e| e.object == object) }
}

pub fn drain() -> Vec<HostFireIntentEvent> {
    { let mut log = LOG.value.lock().unwrap_or_else(|p| p.into_inner()); std::mem::take(&mut log) }
}

pub fn len() -> usize {
    { let mut log = LOG.value.lock().unwrap_or_else(|p| p.into_inner()); log.len() }
}

pub fn clear() {
    { let mut log = LOG.value.lock().unwrap_or_else(|p| p.into_inner()); log.clear() };
}
