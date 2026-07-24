//! Host CarpetBomb OCL DeliverPayload residual (B52 + line drops).
//!
//! C++: `SUPERWEAPON_CarpetBomb` DeliverPayload Transport=AmericaJetB52,
//! Payload=CarpetBomb ×15, DropDelay 300ms, DropVariance, DeliveryDistance 400.
//!
//! Residual playability slice:
//! - Spawn transport at edge residual toward target line
//! - Schedule bomb drop points via `carpet_bomb_points` + DropDelay stagger
//! - Spawn CarpetBomb payload objects that HeightDie / explode
//! - Impact damage residual at each drop
//!
//! Fail-closed: not full AmericaJetB52 pathfinder / preferred-height locomotor.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::game_logic::special_power_strikes::{
    carpet_bomb_impact_frame_for_tier, carpet_bomb_points_for_tier, CarpetBombFactionTier,
    CARPET_BOMB_DAMAGE, CARPET_BOMB_DELIVERY_DISTANCE, CARPET_BOMB_PAYLOAD_OBJECT,
    CARPET_BOMB_RADIUS, CARPET_BOMB_TRANSPORT,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingCarpetBombDrop {
    pub drop_frame: u32,
    pub target: Vec3,
    pub source_id: u32,
    pub bomb_index: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostCarpetBombFlightData {
    pub target: Vec3,
    pub launch: Vec3,
    pub tier: CarpetBombFactionTier,
    pub transport_alive: bool,
}

impl HostCarpetBombFlightData {
    pub fn start(launch: Vec3, target: Vec3, tier: CarpetBombFactionTier) -> Self {
        Self {
            target,
            launch,
            tier,
            transport_alive: true,
        }
    }

    /// Advance transport toward target residual (edge approach).
    pub fn tick_transport(&mut self, pos: Vec3) -> (Vec3, Vec3, bool) {
        let dest = self.target;
        let dx = dest.x - pos.x;
        let dz = dest.z - pos.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let speed = 18.0_f32;
        let mut new_pos = pos;
        new_pos.y = new_pos.y.max(120.0);
        if dist < 5.0 {
            return (new_pos, Vec3::ZERO, true);
        }
        let step = speed.min(dist);
        new_pos.x += dx / dist * step;
        new_pos.z += dz / dist * step;
        let vel = new_pos - pos;
        // DeliveryDistance residual: mark "over target" when within delivery band.
        let over = dist <= CARPET_BOMB_DELIVERY_DISTANCE * 0.5;
        (new_pos, vel, over)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostCarpetBombFlightRegistry {
    pub transports_spawned: u32,
    pub bombs_scheduled: u32,
    pub bombs_dropped: u32,
    pub impacts: u32,
    pub pending_drops: Vec<PendingCarpetBombDrop>,
}

impl HostCarpetBombFlightRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn schedule_drops(
        &mut self,
        activate_frame: u32,
        source_id: u32,
        target: Vec3,
        tier: CarpetBombFactionTier,
    ) {
        let points = carpet_bomb_points_for_tier(target, tier);
        for (i, pt) in points.into_iter().enumerate() {
            let drop_frame =
                carpet_bomb_impact_frame_for_tier(activate_frame, i as u32, tier);
            // Drop slightly before strike residual impact (approach residual).
            let drop_frame = drop_frame.saturating_sub(tier.drop_delay_frames().min(3));
            self.pending_drops.push(PendingCarpetBombDrop {
                drop_frame,
                target: pt,
                source_id,
                bomb_index: i as u32,
            });
            self.bombs_scheduled = self.bombs_scheduled.saturating_add(1);
        }
    }
    pub fn take_due_drops(&mut self, frame: u32) -> Vec<PendingCarpetBombDrop> {
        let mut due = Vec::new();
        let mut keep = Vec::new();
        for p in self.pending_drops.drain(..) {
            if p.drop_frame <= frame {
                due.push(p);
            } else {
                keep.push(p);
            }
        }
        self.pending_drops = keep;
        due
    }
    pub fn record_transport(&mut self) {
        self.transports_spawned = self.transports_spawned.saturating_add(1);
    }
    pub fn record_drop(&mut self) {
        self.bombs_dropped = self.bombs_dropped.saturating_add(1);
    }
    pub fn record_impact(&mut self) {
        self.impacts = self.impacts.saturating_add(1);
    }
    pub fn honesty_host_path_ok(&self) -> bool {
        self.transports_spawned > 0 || self.bombs_scheduled > 0 || self.impacts > 0
    }
}

pub fn honesty_carpet_bomb_flight_residual_ok() -> bool {
    CARPET_BOMB_TRANSPORT == "AmericaJetB52"
        && CARPET_BOMB_PAYLOAD_OBJECT == "CarpetBomb"
        && (CARPET_BOMB_DAMAGE - 300.0).abs() < 0.1
        && (CARPET_BOMB_RADIUS - 50.0).abs() < 0.1
        && carpet_bomb_points_for_tier(Vec3::ZERO, CarpetBombFactionTier::America).len() == 15
        && {
            let mut reg = HostCarpetBombFlightRegistry::new();
            reg.schedule_drops(0, 1, Vec3::new(100.0, 0.0, 0.0), CarpetBombFactionTier::America);
            reg.bombs_scheduled == 15
                && reg.pending_drops[0].drop_frame
                    < reg.pending_drops[14].drop_frame
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedules_staggered_usa_payload() {
        assert!(honesty_carpet_bomb_flight_residual_ok());
    }
}
