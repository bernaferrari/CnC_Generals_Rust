//! Host EMP Pulse DeliverPayload residual (China cargo plane + bomb).
//!
//! C++: `SUPERWEAPON_EMPPulse` DeliverPayload
//! Transport=`ChinaJetCargoPlane`, Payload=`EMPPulseBomb` ×1,
//! DeliveryDistance **150**, DropVariance X:20 Y:20 Z:0.
//! Bomb → EMPPulseEffectSpheroid EMPUpdate disable residual.
//!
//! Residual playability slice:
//! - Spawn ChinaJetCargoPlane transport residual toward target
//! - Drop EMPPulseBomb near DeliveryDistance
//! - Bomb falls; on ground impact triggers host EMP disable field
//!
//! Fail-closed: not full pathfinder / spheroid GPU scale-tint residual.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::game_logic::host_emp_pulse::{
    EMP_PULSE_BOMB_TEMPLATE, EMP_PULSE_DELIVERY_DISTANCE, EMP_PULSE_OCL_TRANSPORT,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostEmpPulseFlightData {
    pub target: Vec3,
    pub launch: Vec3,
    pub player_id: u32,
    pub caster_id: u32,
}

impl HostEmpPulseFlightData {
    pub fn start(launch: Vec3, target: Vec3, player_id: u32, caster_id: u32) -> Self {
        Self {
            target,
            launch,
            player_id,
            caster_id,
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
        let over = dist <= EMP_PULSE_DELIVERY_DISTANCE * 0.5;
        (new_pos, vel, over)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostEmpPulseFlightRegistry {
    pub transports_spawned: u32,
    pub bombs_dropped: u32,
    pub spheroids_spawned: u32,
    pub detonations: u32,
}

impl HostEmpPulseFlightRegistry {
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

    pub fn record_spheroid(&mut self) {
        self.spheroids_spawned = self.spheroids_spawned.saturating_add(1);
    }

    pub fn record_detonation(&mut self) {
        self.detonations = self.detonations.saturating_add(1);
    }

    pub fn honesty_host_path_ok(&self) -> bool {
        self.transports_spawned > 0 && self.bombs_dropped > 0 && self.detonations > 0
    }
}

pub fn honesty_emp_pulse_flight_residual_ok() -> bool {
    EMP_PULSE_OCL_TRANSPORT == "ChinaJetCargoPlane"
        && EMP_PULSE_BOMB_TEMPLATE == "EMPPulseBomb"
        && (EMP_PULSE_DELIVERY_DISTANCE - 150.0).abs() < 0.1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn honesty_pack() {
        assert!(honesty_emp_pulse_flight_residual_ok());
    }
}
