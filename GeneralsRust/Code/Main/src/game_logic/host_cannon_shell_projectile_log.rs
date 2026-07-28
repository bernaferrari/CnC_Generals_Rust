//! Frame-local SCUD/Neutron/Nuke shell impact logs for GW shadow parity.

use super::{ObjectId, Team};
use glam::Vec3;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CannonShellKind {
    Scud { toxin: bool },
    Neutron,
    Nuke,
}

#[derive(Debug, Clone)]
pub struct CannonShellImpactEvent {
    pub id: ObjectId,
    pub source: Option<ObjectId>,
    pub team: Team,
    pub pos: Vec3,
    pub kind: CannonShellKind,
}

thread_local! {
    static IMPACTS: RefCell<Vec<CannonShellImpactEvent>> = RefCell::new(Vec::new());
}

pub fn record_impact(ev: CannonShellImpactEvent) {
    IMPACTS.with(|l| l.borrow_mut().push(ev));
}

pub fn drain_impacts() -> Vec<CannonShellImpactEvent> {
    IMPACTS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn clear() {
    IMPACTS.with(|l| l.borrow_mut().clear());
}
