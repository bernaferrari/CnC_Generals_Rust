//! Cash deposit data passed directly to receiving Main callbacks.

use super::{ObjectId, Team};
use glam::Vec3;

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
