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
    /// C++ DeliveringState finished → HeadOffMapState.
    #[serde(default)]
    pub delivery_complete: bool,
    /// C++ HeadOffMapState: dest is HUGE_DIST after delivery.
    #[serde(default)]
    pub passed_target: bool,
    /// Map extent for C++ `isOffMap` / HeadOffMap HUGE_DIST (world_min/max).
    #[serde(default)]
    pub map_min: Vec3,
    #[serde(default)]
    pub map_max: Vec3,
}

impl HostEmpPulseFlightData {
    pub fn start(launch: Vec3, target: Vec3, player_id: u32, caster_id: u32) -> Self {
        Self {
            target,
            launch,
            player_id,
            caster_id,
            delivery_complete: false,
            passed_target: false,
            map_min: Vec3::ZERO,
            map_max: Vec3::ZERO,
        }
    }

    pub fn map_extent_ok(&self) -> bool {
        self.map_max.x > self.map_min.x && self.map_max.z > self.map_min.z
    }

    /// C++ DeliveringState `isCloseEnoughToTarget` residual (live band).
    pub fn in_delivery_band(&self, pos: Vec3) -> bool {
        let dx = self.target.x - pos.x;
        let dz = self.target.z - pos.z;
        let dist = (dx * dx + dz * dz).sqrt();
        dist < 5.0 || dist <= EMP_PULSE_DELIVERY_DISTANCE * 0.5
    }

    /// C++ Approach/Delivering toward moveToPos, then HeadOffMap HUGE_DIST.
    /// Returns (new_pos, vel, off_map / CLEAN_UP).
    pub fn tick_transport(&mut self, pos: Vec3) -> (Vec3, Vec3, bool) {
        use crate::game_logic::host_deliver_payload::{
            RESIDUAL_MAP_EXTENT_MAX_X, RESIDUAL_MAP_EXTENT_MAX_Z, RESIDUAL_MAP_EXTENT_MIN_X,
            RESIDUAL_MAP_EXTENT_MIN_Z, head_off_map_exit_point_residual, is_off_map_residual,
        };
        let hx = self.target.x - self.launch.x;
        let hz = self.target.z - self.launch.z;
        if self.delivery_complete && !self.passed_target {
            self.passed_target = true;
        }
        let (min_x, min_z, max_x, max_z) = if self.map_extent_ok() {
            (
                self.map_min.x,
                self.map_min.z,
                self.map_max.x,
                self.map_max.z,
            )
        } else {
            (
                RESIDUAL_MAP_EXTENT_MIN_X,
                RESIDUAL_MAP_EXTENT_MIN_Z,
                RESIDUAL_MAP_EXTENT_MAX_X,
                RESIDUAL_MAP_EXTENT_MAX_Z,
            )
        };
        let dest = if self.delivery_complete {
            head_off_map_exit_point_residual(pos, hx, hz, min_x, min_z, max_x, max_z)
        } else {
            self.target
        };
        let dx = dest.x - pos.x;
        let dz = dest.z - pos.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let speed = 18.0_f32;
        let mut new_pos = pos;
        new_pos.y = new_pos.y.max(150.0);
        let vel = if dist >= 1.0 {
            let step = speed.min(dist);
            new_pos.x += dx / dist * step;
            new_pos.z += dz / dist * step;
            new_pos - pos
        } else {
            Vec3::ZERO
        };
        let at_exit =
            self.delivery_complete && is_off_map_residual(new_pos, min_x, min_z, max_x, max_z);
        (new_pos, vel, at_exit)
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

    #[test]
    fn head_off_map_flies_past_target_then_destroys() {
        let mut data = HostEmpPulseFlightData::start(
            Vec3::new(0.0, 150.0, 0.0),
            Vec3::new(100.0, 0.0, 0.0),
            0,
            0,
        );
        data.map_min = Vec3::new(0.0, 0.0, 0.0);
        data.map_max = Vec3::new(200.0, 0.0, 200.0);
        data.delivery_complete = true;
        let mut pos = Vec3::new(98.0, 150.0, 0.0);
        let mut left = false;
        let mut destroyed = false;
        for _ in 0..80 {
            let (next, vel, at_exit) = data.tick_transport(pos);
            assert!(vel.x > 0.0 || at_exit, "must keep flying past the target");
            pos = next;
            if pos.x > 100.0 {
                left = true;
            }
            if at_exit {
                destroyed = true;
                break;
            }
        }
        assert!(
            left && destroyed,
            "C++ HeadOffMap+CleanUp after delivery, pos={pos:?}"
        );
    }
}
