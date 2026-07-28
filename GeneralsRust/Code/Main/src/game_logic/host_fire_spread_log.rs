//! Frame-local fire-spread tick logs for GW shadow parity.

use super::ObjectId;
use glam::Vec3;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct FireSpreadTickEvent {
    pub id: ObjectId,
    pub state: u8,
    pub aflame_end_frame: u32,
    pub burned_end_frame: u32,
    pub next_spread_frame: u32,
    pub became_burned: bool,
    pub aflame: bool,
    pub try_spread: bool,
    pub spawn_embers: bool,
    pub ignite_target: Option<ObjectId>,
    pub pos: Vec3,
    pub spread_try_range: f32,
    pub flame_damage_accum: f32,
}

thread_local! {
    static EVENTS: RefCell<Vec<FireSpreadTickEvent>> = RefCell::new(Vec::new());
}

pub fn record(ev: FireSpreadTickEvent) {
    EVENTS.with(|l| l.borrow_mut().push(ev));
}

pub fn drain() -> Vec<FireSpreadTickEvent> {
    EVENTS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn clear() {
    EVENTS.with(|l| l.borrow_mut().clear());
}
