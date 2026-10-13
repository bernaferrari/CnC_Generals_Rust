//! Hacker income data passed directly to receiving Main callbacks.

use super::{ObjectId, Team};
use glam::Vec3;

#[derive(Debug, Clone)]
pub struct HackerIncomeEvent {
    pub id: ObjectId,
    pub team: Team,
    /// Authoritative host player that owns this hacker.
    pub owner_player_id: Option<u32>,
    pub pos: Vec3,
    pub amount: u32,
    /// Exact `HackInternetAIUpdate::XpPerCashUpdate` captured by the receiving
    /// operation.  It must not collapse to the old retail `1`.
    pub xp_per_cash_update: f32,
    pub next_deposit_frame: u32,
    pub in_internet_center: bool,
    pub stealthed: bool,
    pub detected: bool,
    /// 0=Rookie .. 3=Heroic
    pub veterancy_ordinal: u8,
    pub container_radius: f32,
}
