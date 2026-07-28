//! Frame-local Inferno shell impact logs for GW shadow parity.

use super::{ObjectId, Team};
use glam::Vec3;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct InfernoShellImpactEvent {
    pub id: ObjectId,
    pub source: Option<ObjectId>,
    pub intended: Option<ObjectId>,
    pub pos: Vec3,
    pub upgraded: bool,
    pub team: Team,
}

thread_local! {
    static IMPACTS: RefCell<Vec<InfernoShellImpactEvent>> = RefCell::new(Vec::new());
}

pub fn record_impact(ev: InfernoShellImpactEvent) {
    IMPACTS.with(|l| l.borrow_mut().push(ev));
}

pub fn drain_impacts() -> Vec<InfernoShellImpactEvent> {
    IMPACTS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn clear() {
    IMPACTS.with(|l| l.borrow_mut().clear());
}
