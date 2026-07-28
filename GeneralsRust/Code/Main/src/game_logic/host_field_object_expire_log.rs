//! Frame-local field-object lifetime expire logs for GW shadow parity.
//!
//! Covers NukeRadiationField / AnthraxToxinField / InfernoFireField object residuals.

use super::{ObjectId, Team};
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldObjectKind {
    NukeRadiation,
    AnthraxToxin,
    InfernoFire,
}

#[derive(Debug, Clone)]
pub struct FieldObjectExpireEvent {
    pub id: ObjectId,
    pub team: Option<Team>,
    pub kind: FieldObjectKind,
}

thread_local! {
    static EXPIRES: RefCell<Vec<FieldObjectExpireEvent>> = RefCell::new(Vec::new());
}

pub fn record(ev: FieldObjectExpireEvent) {
    EXPIRES.with(|l| l.borrow_mut().push(ev));
}

pub fn drain() -> Vec<FieldObjectExpireEvent> {
    EXPIRES.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn clear() {
    EXPIRES.with(|l| l.borrow_mut().clear());
}
