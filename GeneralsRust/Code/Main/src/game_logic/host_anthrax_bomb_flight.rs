//! Host AnthraxBomb DeliverPayload residual (GLA cargo plane + bomb).
//!
//! C++: `SUPERWEAPON_AnthraxBomb` DeliverPayload
//! Transport=`GLAJetCargoPlane`, Payload=`AnthraxBomb`,
//! DeliveryDistance **140**, DeliveryDecalRadius **200**.
//! Gamma tier: `SUPERWEAPON_AnthraxBombGamma` Payload=`AnthraxBombGamma`.
//!
//! Residual playability slice:
//! - Spawn GLAJetCargoPlane transport residual toward target
//! - Drop AnthraxBomb near DeliveryDistance
//! - Bomb falls, applies impact damage, spawns toxin field residual hook
//!
//! Fail-closed: not full GLAJetCargoPlane pathfinder / preferred-height locomotor.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::game_logic::special_power_strikes::{
    ANTHRAX_BOMB_IMPACT_DAMAGE, ANTHRAX_BOMB_IMPACT_RADIUS,
};

/// Retail DeliverPayload Transport residual.
pub const ANTHRAX_TRANSPORT: &str = "GLAJetCargoPlane";
/// Retail Payload residual.
pub const ANTHRAX_BOMB_OBJECT: &str = "AnthraxBomb";
/// Retail Gamma Payload residual.
pub const ANTHRAX_BOMB_GAMMA_OBJECT: &str = "AnthraxBombGamma";
/// Retail DeliveryDistance residual.
pub const ANTHRAX_DELIVERY_DISTANCE: f32 = 140.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AnthraxBombPayloadTier {
    #[default]
    Base,
    Gamma,
}

impl AnthraxBombPayloadTier {
    pub fn bomb(self) -> &'static str {
        match self {
            AnthraxBombPayloadTier::Base => ANTHRAX_BOMB_OBJECT,
            AnthraxBombPayloadTier::Gamma => ANTHRAX_BOMB_GAMMA_OBJECT,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostAnthraxBombFlightData {
    pub target: Vec3,
    pub launch: Vec3,
    pub tier: AnthraxBombPayloadTier,
}

impl HostAnthraxBombFlightData {
    pub fn start(launch: Vec3, target: Vec3, tier: AnthraxBombPayloadTier) -> Self {
        Self {
            target,
            launch,
            tier,
        }
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
        let over = dist <= ANTHRAX_DELIVERY_DISTANCE * 0.5;
        (new_pos, vel, over)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostAnthraxBombFlightRegistry {
    pub transports_spawned: u32,
    pub bombs_dropped: u32,
    pub detonations: u32,
    pub toxin_fields_spawned: u32,
}

impl HostAnthraxBombFlightRegistry {
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

    pub fn record_toxin_field(&mut self) {
        self.toxin_fields_spawned = self.toxin_fields_spawned.saturating_add(1);
    }

    pub fn honesty_host_path_ok(&self) -> bool {
        self.transports_spawned > 0 && self.bombs_dropped > 0 && self.detonations > 0
    }
}

pub fn honesty_anthrax_bomb_flight_residual_ok() -> bool {
    ANTHRAX_TRANSPORT == "GLAJetCargoPlane"
        && ANTHRAX_BOMB_OBJECT == "AnthraxBomb"
        && ANTHRAX_BOMB_GAMMA_OBJECT == "AnthraxBombGamma"
        && (ANTHRAX_DELIVERY_DISTANCE - 140.0).abs() < 0.1
        && (ANTHRAX_BOMB_IMPACT_DAMAGE - 200.0).abs() < 0.1
        && (ANTHRAX_BOMB_IMPACT_RADIUS - 100.0).abs() < 0.1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn honesty_pack() {
        assert!(honesty_anthrax_bomb_flight_residual_ok());
    }
}
