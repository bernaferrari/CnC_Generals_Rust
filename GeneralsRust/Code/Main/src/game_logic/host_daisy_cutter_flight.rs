//! Host DaisyCutter / MOAB DeliverPayload residual (jet + bomb).
//!
//! C++:
//! - `SUPERWEAPON_DaisyCutter` Transport=`AmericaJetB52` Payload=`DaisyCutterBomb`
//!   DeliveryDistance **140**
//! - `SUPERWEAPON_MOAB` Transport=`AmericaJetB3` Payload=`MOAB`
//!   DeliveryDistance **160** PreOpenDistance **160**
//!
//! Residual playability slice:
//! - Spawn transport residual toward target
//! - Drop bomb near DeliveryDistance
//! - Bomb falls and detonates (Daisy 2000/r100 or MOAB 2000/r150)
//!
//! Fail-closed: not full jet pathfinder / preferred-height locomotor.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::game_logic::special_power_strikes::{
    CRUISE_MISSILE_DAMAGE, CRUISE_MISSILE_RADIUS, DAISY_CUTTER_IMPACT_DELAY_FRAMES,
    DAISY_CUTTER_PRIMARY_DAMAGE, DAISY_CUTTER_PRIMARY_RADIUS,
};

/// Retail Daisy DeliverPayload Transport residual.
pub const DAISY_TRANSPORT: &str = "AmericaJetB52";
/// Retail Daisy Payload residual.
pub const DAISY_BOMB_OBJECT: &str = "DaisyCutterBomb";
/// Retail Daisy DeliveryDistance residual.
pub const DAISY_DELIVERY_DISTANCE: f32 = 140.0;

/// Retail MOAB DeliverPayload Transport residual.
pub const MOAB_TRANSPORT: &str = "AmericaJetB3";
/// Retail MOAB Payload residual.
pub const MOAB_BOMB_OBJECT: &str = "MOAB";
/// Retail MOAB DeliveryDistance residual.
pub const MOAB_DELIVERY_DISTANCE: f32 = 160.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DaisyFlightPayloadTier {
    #[default]
    DaisyCutter,
    Moab,
}

impl DaisyFlightPayloadTier {
    /// C++ `findOCL` result: `SUPERWEAPON_MOAB` vs default Daisy.
    pub fn from_ocl_name(ocl_name: &str) -> Self {
        if ocl_name.to_ascii_lowercase().contains("moab") {
            Self::Moab
        } else {
            Self::DaisyCutter
        }
    }

    pub fn transport(self) -> &'static str {
        match self {
            DaisyFlightPayloadTier::DaisyCutter => DAISY_TRANSPORT,
            DaisyFlightPayloadTier::Moab => MOAB_TRANSPORT,
        }
    }

    pub fn bomb(self) -> &'static str {
        match self {
            DaisyFlightPayloadTier::DaisyCutter => DAISY_BOMB_OBJECT,
            DaisyFlightPayloadTier::Moab => MOAB_BOMB_OBJECT,
        }
    }

    pub fn delivery_distance(self) -> f32 {
        match self {
            DaisyFlightPayloadTier::DaisyCutter => DAISY_DELIVERY_DISTANCE,
            DaisyFlightPayloadTier::Moab => MOAB_DELIVERY_DISTANCE,
        }
    }

    pub fn primary_damage(self) -> f32 {
        match self {
            DaisyFlightPayloadTier::DaisyCutter => DAISY_CUTTER_PRIMARY_DAMAGE,
            DaisyFlightPayloadTier::Moab => CRUISE_MISSILE_DAMAGE,
        }
    }

    pub fn primary_radius(self) -> f32 {
        match self {
            DaisyFlightPayloadTier::DaisyCutter => DAISY_CUTTER_PRIMARY_RADIUS,
            DaisyFlightPayloadTier::Moab => CRUISE_MISSILE_RADIUS,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostDaisyCutterFlightData {
    pub target: Vec3,
    pub launch: Vec3,
    pub tier: DaisyFlightPayloadTier,
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

impl HostDaisyCutterFlightData {
    pub fn start(launch: Vec3, target: Vec3, tier: DaisyFlightPayloadTier) -> Self {
        Self {
            target,
            launch,
            tier,
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
        dist < 5.0 || dist <= self.tier.delivery_distance() * 0.5
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
        let speed = 20.0_f32;
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
pub struct HostDaisyCutterFlightRegistry {
    pub transports_spawned: u32,
    pub bombs_dropped: u32,
    pub detonations: u32,
    pub moab_transports_spawned: u32,
}

impl HostDaisyCutterFlightRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn record_transport(&mut self, tier: DaisyFlightPayloadTier) {
        self.transports_spawned = self.transports_spawned.saturating_add(1);
        if tier == DaisyFlightPayloadTier::Moab {
            self.moab_transports_spawned = self.moab_transports_spawned.saturating_add(1);
        }
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
        && MOAB_TRANSPORT == "AmericaJetB3"
        && MOAB_BOMB_OBJECT == "MOAB"
        && (DAISY_DELIVERY_DISTANCE - 140.0).abs() < 0.1
        && (MOAB_DELIVERY_DISTANCE - 160.0).abs() < 0.1
        && (DAISY_CUTTER_PRIMARY_DAMAGE - 2000.0).abs() < 0.1
        && (DAISY_CUTTER_PRIMARY_RADIUS - 100.0).abs() < 0.1
        && (CRUISE_MISSILE_DAMAGE - 2000.0).abs() < 0.1
        && (CRUISE_MISSILE_RADIUS - 150.0).abs() < 0.1
        && DAISY_CUTTER_IMPACT_DELAY_FRAMES == 90
        && DaisyFlightPayloadTier::Moab.transport() == "AmericaJetB3"
        && DaisyFlightPayloadTier::from_ocl_name("SUPERWEAPON_MOAB") == DaisyFlightPayloadTier::Moab
        && DaisyFlightPayloadTier::from_ocl_name("SUPERWEAPON_DaisyCutter")
            == DaisyFlightPayloadTier::DaisyCutter
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn honesty_pack() {
        assert!(honesty_daisy_cutter_flight_residual_ok());
    }

    #[test]
    fn head_off_map_flies_past_target_then_destroys() {
        let mut data = HostDaisyCutterFlightData::start(
            Vec3::new(0.0, 160.0, 0.0),
            Vec3::new(100.0, 0.0, 0.0),
            DaisyFlightPayloadTier::DaisyCutter,
        );
        data.map_min = Vec3::new(0.0, 0.0, 0.0);
        data.map_max = Vec3::new(200.0, 0.0, 200.0);
        data.delivery_complete = true;
        let mut pos = Vec3::new(98.0, 160.0, 0.0);
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
