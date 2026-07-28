//! Frame-local Flashbang/Comanche/Helix projectile logs for GW shadow parity.

use super::ObjectId;
use glam::Vec3;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct FlashbangImpactEvent {
    pub id: ObjectId,
    pub source: Option<ObjectId>,
    pub intended: Option<ObjectId>,
    pub pos: Vec3,
}

thread_local! {
    static FLASHBANG: RefCell<Vec<FlashbangImpactEvent>> = RefCell::new(Vec::new());
    static COMANCHE_EXPIRE: RefCell<Vec<ObjectId>> = RefCell::new(Vec::new());
}

pub fn record_flashbang(ev: FlashbangImpactEvent) {
    FLASHBANG.with(|l| l.borrow_mut().push(ev));
}

pub fn record_comanche_expire(id: ObjectId) {
    COMANCHE_EXPIRE.with(|l| l.borrow_mut().push(id));
}

pub fn drain_flashbang() -> Vec<FlashbangImpactEvent> {
    FLASHBANG.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn drain_comanche_expires() -> Vec<ObjectId> {
    COMANCHE_EXPIRE.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn clear() {
    FLASHBANG.with(|l| l.borrow_mut().clear());
    COMANCHE_EXPIRE.with(|l| l.borrow_mut().clear());
}
