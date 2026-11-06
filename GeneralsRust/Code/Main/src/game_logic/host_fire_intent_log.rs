//! Frame-local host fire-intent log for GameWorld SetFireIntent parity.

use super::ObjectId;

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

fn with_log_mut<R>(f: impl FnOnce(&mut Vec<HostFireIntentEvent>) -> R) -> Option<R> {
    let services = crate::main_services::try_main_services()?;
    let mut log = services
        .host_fire_intent_log
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    Some(f(&mut log))
}

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
    let _ = with_log_mut(|log| {
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
    });
}

pub fn has_pending(object: ObjectId) -> bool {
    with_log_mut(|log| log.iter().any(|e| e.object == object)).unwrap_or(false)
}

pub fn drain() -> Vec<HostFireIntentEvent> {
    with_log_mut(std::mem::take).unwrap_or_default()
}

pub fn len() -> usize {
    with_log_mut(|log| log.len()).unwrap_or(0)
}

pub fn clear() {
    let _ = with_log_mut(|log| log.clear());
}
