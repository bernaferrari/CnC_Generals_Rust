//! Frame-local AngryMob projectile impact logs for GW shadow parity.

use super::ObjectId;
use glam::Vec3;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct AngryMobProjectileImpactEvent {
    pub id: ObjectId,
    pub source: Option<ObjectId>,
    pub intended: Option<ObjectId>,
    pub pos: Vec3,
    pub kind: u8,
}

thread_local! {
    static IMPACTS: RefCell<Vec<AngryMobProjectileImpactEvent>> = RefCell::new(Vec::new());
}

pub fn record_impact(ev: AngryMobProjectileImpactEvent) {
    IMPACTS.with(|l| l.borrow_mut().push(ev));
}

pub fn drain_impacts() -> Vec<AngryMobProjectileImpactEvent> {
    IMPACTS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn clear() {
    IMPACTS.with(|l| l.borrow_mut().clear());
}
