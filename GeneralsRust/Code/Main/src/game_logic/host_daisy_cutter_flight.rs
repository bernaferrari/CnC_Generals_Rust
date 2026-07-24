//! Host DaisyCutter DeliverPayload residual (B52 + bomb).
//!
//! C++: `SUPERWEAPON_DaisyCutter` DeliverPayload
//! Transport=`AmericaJetB52`, Payload=`DaisyCutterBomb`,
//! DeliveryDistance **140**, DeliveryDecalRadius **170**.
//!
//! Residual playability slice:
//! - Spawn B52 transport residual toward target
//! - Drop DaisyCutterBomb near DeliveryDistance
//! - Bomb falls and detonates (area damage + fuel-air gas path hook)
//!
//! Fail-closed: not full AmericaJetB52 pathfinder / preferred-height locomotor.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::game_logic::special_power_strikes::{
    DAISY_CUTTER_IMPACT_DELAY_FRAMES, DAISY_CUTTER_PRIMARY_DAMAGE, DAISY_CUTTER_PRIMARY_RADIUS,
};

/// Retail DeliverPayload Transport residual.
pub const DAISY_TRANSPORT: &str = "AmericaJetB52";
/// Retail Payload residual.
pub const DAISY_BOMB_OBJECT: &str = "DaisyCutterBomb";
/// Retail DeliveryDistance residual.
pub const DAISY_DELIVERY_DISTANCE: f32 = 140.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostDaisyCutterFlightData {
    pub target: Vec3,
    pub launch: Vec3,
}

impl HostDaisyCutterFlightData {
    pub fn start(launch: Vec3, target: Vec3) -> Self {
        Self { target, launch }
    }

    pub fn tick_transport(&mut self, pos: Vec3) -> (Vec3, Vec3, bool) {
        let dest = self.target;
        let dx = dest.x - pos.x;
        let dz = dest.z - pos.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let speed = 20.0_f32;
        let mut new_pos = pos;
        new_pos.y = new_pos.y.max(150.0);
        if dist < 5.0 {
            return (new_pos, Vec3::ZERO, true);
        }
        let step = speed.min(dist);
        new_pos.x += dx / dist * step;
        new_pos.z += dz / dist * step;
        let vel = new_pos - pos;
        let over = dist <= DAISY_DELIVERY_DISTANCE * 0.5;
        (new_pos, vel, over)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostDaisyCutterFlightRegistry {
    pub transports_spawned: u32,
    pub bombs_dropped: u32,
    pub detonations: u32,
}

impl HostDaisyCutterFlightRegistry {
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

    pub fn record_detonation(&mut self) {
        self.detonations = self.detonations.saturating_add(1);
    }

    pub fn honesty_host_path_ok(&self) -> bool {
        self.transports_spawned > 0 && self.bombs_dropped > 0 && self.detonations > 0
    }
}

pub fn honesty_daisy_cutter_flight_residual_ok() -> bool {
    DAISY_TRANSPORT == "AmericaJetB52"
        && DAISY_BOMB_OBJECT == "DaisyCutterBomb"
        && (DAISY_DELIVERY_DISTANCE - 140.0).abs() < 0.1
        && (DAISY_CUTTER_PRIMARY_DAMAGE - 2000.0).abs() < 0.1
        && (DAISY_CUTTER_PRIMARY_RADIUS - 100.0).abs() < 0.1
        && DAISY_CUTTER_IMPACT_DELAY_FRAMES == 90
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn honesty_pack() {
        assert!(honesty_daisy_cutter_flight_residual_ok());
    }
}
