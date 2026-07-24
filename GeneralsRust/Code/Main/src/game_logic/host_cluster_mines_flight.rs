//! Host ClusterMines DeliverPayload residual (China cargo plane + bomb).
//!
//! C++: `SUPERWEAPON_ClusterMines` DeliverPayload
//! Transport=`ChinaJetCargoPlane`, Payload=`ClusterMinesBomb` ×1,
//! DeliveryDistance **140**, DropVariance X:20 Y:20 Z:0,
//! bomb → GenerateMinefieldBehavior NumVirtualMines **8**.
//!
//! Residual playability slice:
//! - Spawn ChinaJetCargoPlane transport residual toward target
//! - Drop ClusterMinesBomb near DeliveryDistance
//! - Bomb falls; on ground impact places mine ring via host_mines residual
//!
//! Fail-closed: not full pathfinder / SmartBorder minefield matrix.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::game_logic::host_mines::{
    CLUSTER_MINES_BOMB_TEMPLATE, CLUSTER_MINES_DELIVERY_DISTANCE, CLUSTER_MINES_OCL_TRANSPORT,
    CLUSTER_MINE_NUM_VIRTUAL,
};

/// Retail Payload residual alias.
pub const CLUSTER_MINES_BOMB_OBJECT: &str = CLUSTER_MINES_BOMB_TEMPLATE;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostClusterMinesFlightData {
    pub target: Vec3,
    pub launch: Vec3,
}

impl HostClusterMinesFlightData {
    pub fn start(launch: Vec3, target: Vec3) -> Self {
        Self { target, launch }
    }

    pub fn tick_transport(&mut self, pos: Vec3) -> (Vec3, Vec3, bool) {
        let dest = self.target;
        let dx = dest.x - pos.x;
        let dz = dest.z - pos.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let speed = 18.0_f32;
        let mut new_pos = pos;
        new_pos.y = new_pos.y.max(150.0);
        if dist < 5.0 {
            return (new_pos, Vec3::ZERO, true);
        }
        let step = speed.min(dist);
        new_pos.x += dx / dist * step;
        new_pos.z += dz / dist * step;
        let vel = new_pos - pos;
        let over = dist <= CLUSTER_MINES_DELIVERY_DISTANCE * 0.5;
        (new_pos, vel, over)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostClusterMinesFlightRegistry {
    pub transports_spawned: u32,
    pub bombs_dropped: u32,
    pub minefields_placed: u32,
    pub mines_spawned: u32,
}

impl HostClusterMinesFlightRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn record_transport(&mut self) {
        self.transports_spawned = self.transports_spawned.saturating_add(1);
    }

    pub fn record_drop(&mut self) {
        self.bombs_dropped = self.bombs_dropped.saturating_add(1);
    }

    pub fn record_minefield(&mut self, mine_count: u32) {
        self.minefields_placed = self.minefields_placed.saturating_add(1);
        self.mines_spawned = self.mines_spawned.saturating_add(mine_count);
    }

    pub fn honesty_host_path_ok(&self) -> bool {
        self.transports_spawned > 0 && self.bombs_dropped > 0 && self.minefields_placed > 0
    }
}

pub fn honesty_cluster_mines_flight_residual_ok() -> bool {
    CLUSTER_MINES_OCL_TRANSPORT == "ChinaJetCargoPlane"
        && CLUSTER_MINES_BOMB_OBJECT == "ClusterMinesBomb"
        && (CLUSTER_MINES_DELIVERY_DISTANCE - 140.0).abs() < 0.1
        && CLUSTER_MINE_NUM_VIRTUAL == 8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn honesty_pack() {
        assert!(honesty_cluster_mines_flight_residual_ok());
    }
}
