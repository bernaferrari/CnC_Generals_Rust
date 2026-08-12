//! Frame-local AutoDeposit (black market / oil derrick) logs for GW shadow parity.

use super::{ObjectId, Team};
use glam::Vec3;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoDepositKind {
    BlackMarket,
    OilDerrick,
}

#[derive(Debug, Clone)]
pub struct AutoDepositEvent {
    pub id: ObjectId,
    pub kind: AutoDepositKind,
    pub team: Team,
    /// Authoritative host player that owns the source object.  `None` is a
    /// genuinely unowned legacy/neutral source, not a request to pick an
    /// arbitrary player from `team`.
    pub owner_player_id: Option<u32>,
    pub pos: Vec3,
    pub amount: u32,
    pub next_deposit_frame: u32,
    pub stealthed: bool,
    pub detected: bool,
    pub supply_lines_boost: u32,
}

thread_local! {
    static EVENTS: RefCell<Vec<AutoDepositEvent>> = RefCell::new(Vec::new());
}

pub fn record(ev: AutoDepositEvent) {
    EVENTS.with(|l| l.borrow_mut().push(ev));
}

pub fn drain() -> Vec<AutoDepositEvent> {
    EVENTS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn clear() {
    EVENTS.with(|l| l.borrow_mut().clear());
}
