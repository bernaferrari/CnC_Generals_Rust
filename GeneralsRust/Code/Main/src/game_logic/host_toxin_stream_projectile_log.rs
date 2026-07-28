//! Frame-local ToxinStream projectile impact logs for GW shadow parity.

use super::{ObjectId, Team};
use glam::Vec3;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct ToxinStreamImpactEvent {
    pub id: ObjectId,
    pub source: Option<ObjectId>,
    pub intended: Option<ObjectId>,
    pub pos: Vec3,
    pub team: Team,
}

#[derive(Debug, Clone)]
pub struct ToxinStreamPointEvent {
    pub shooter: ObjectId,
    pub pos: Vec3,
    pub intended: Option<ObjectId>,
    pub aim: Vec3,
}

thread_local! {
    static IMPACTS: RefCell<Vec<ToxinStreamImpactEvent>> = RefCell::new(Vec::new());
    static STREAMS: RefCell<Vec<ToxinStreamPointEvent>> = RefCell::new(Vec::new());
}

pub fn record_impact(ev: ToxinStreamImpactEvent) {
    IMPACTS.with(|l| l.borrow_mut().push(ev));
}

pub fn record_stream(ev: ToxinStreamPointEvent) {
    STREAMS.with(|l| l.borrow_mut().push(ev));
}

pub fn drain_impacts() -> Vec<ToxinStreamImpactEvent> {
    IMPACTS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn drain_streams() -> Vec<ToxinStreamPointEvent> {
    STREAMS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn clear() {
    IMPACTS.with(|l| l.borrow_mut().clear());
    STREAMS.with(|l| l.borrow_mut().clear());
}
