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
/// Retail SUPERWEAPON_AnthraxBomb DeliveryDecalRadius residual.
pub const ANTHRAX_DELIVERY_DECAL_RADIUS: f32 = 200.0;
/// Retail DeliverPayload DropOffset residual (C++ X/Y/Z, Z-up).
pub const ANTHRAX_DROP_OFFSET: (f32, f32, f32) = (0.0, 0.0, 0.0);

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

    /// Leftover SUPERWEAPON_AnthraxBomb / AnthraxBombGamma payload.
    pub fn from_ocl(ocl: &str) -> Self {
        let n = ocl.to_ascii_lowercase();
        if n.contains("anthraxbombgamma") {
            AnthraxBombPayloadTier::Gamma
        } else {
            AnthraxBombPayloadTier::Base
        }
    }

    pub fn ocl(self) -> &'static str {
        use crate::game_logic::host_ocl_special_power::{ANTHRAX_BOMB_GAMMA_OCL, ANTHRAX_BOMB_OCL};
        match self {
            AnthraxBombPayloadTier::Base => ANTHRAX_BOMB_OCL,
            AnthraxBombPayloadTier::Gamma => ANTHRAX_BOMB_GAMMA_OCL,
        }
    }

    /// C++ FireOCL CreateObject for the impact puddle.
    pub fn toxin_object(self) -> &'static str {
        use crate::game_logic::special_power_strikes::{
            ANTHRAX_TOXIN_OBJECT_NAME, ANTHRAX_TOXIN_OBJECT_NAME_GAMMA,
        };
        match self {
            AnthraxBombPayloadTier::Base => ANTHRAX_TOXIN_OBJECT_NAME,
            AnthraxBombPayloadTier::Gamma => ANTHRAX_TOXIN_OBJECT_NAME_GAMMA,
        }
    }

    /// Palace `Chem_Upgrade_GLAAnthraxGamma` / `SUPERWEAPON_AnthraxBombGamma`.
    pub fn from_player_upgrade_names<'a, I>(names: I) -> Self
    where
        I: IntoIterator<Item = &'a str>,
    {
        use crate::game_logic::host_toxin_tractor::is_anthrax_gamma_upgrade_name;
        for name in names {
            if is_anthrax_gamma_upgrade_name(name) {
                return AnthraxBombPayloadTier::Gamma;
            }
            let n = name.to_ascii_lowercase();
            if n.contains("anthraxbombgamma") {
                return AnthraxBombPayloadTier::Gamma;
            }
        }
        AnthraxBombPayloadTier::Base
    }
}

/// C++ contained payload exits at the transport pose + DropOffset (host Y-up).
pub fn anthrax_payload_drop_pos(plane_pos: Vec3) -> Vec3 {
    Vec3::new(
        plane_pos.x + ANTHRAX_DROP_OFFSET.0,
        plane_pos.y + ANTHRAX_DROP_OFFSET.2,
        plane_pos.z + ANTHRAX_DROP_OFFSET.1,
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostAnthraxBombFlightData {
    pub target: Vec3,
    pub launch: Vec3,
    pub tier: AnthraxBombPayloadTier,
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

impl HostAnthraxBombFlightData {
    pub fn start(launch: Vec3, target: Vec3, tier: AnthraxBombPayloadTier) -> Self {
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

    /// C++ DeliverPayloadAIUpdate::isCloseEnoughToTarget (DeliveryDistance 140).
    pub fn in_delivery_band(&self, pos: Vec3) -> bool {
        let dx = self.target.x - pos.x;
        let dz = self.target.z - pos.z;
        let dist = (dx * dx + dz * dz).sqrt();
        dist <= ANTHRAX_DELIVERY_DISTANCE
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
    use crate::game_logic::host_ocl_special_power::{
        ANTHRAX_BOMB_GAMMA_OCL, ANTHRAX_BOMB_OCL, resolve_anthrax_bomb_ocl,
    };
    ANTHRAX_TRANSPORT == "GLAJetCargoPlane"
        && ANTHRAX_BOMB_OBJECT == "AnthraxBomb"
        && ANTHRAX_BOMB_GAMMA_OBJECT == "AnthraxBombGamma"
        && (ANTHRAX_DELIVERY_DISTANCE - 140.0).abs() < 0.1
        && (ANTHRAX_DELIVERY_DECAL_RADIUS - 200.0).abs() < 0.1
        && (ANTHRAX_BOMB_IMPACT_DAMAGE - 200.0).abs() < 0.1
        && (ANTHRAX_BOMB_IMPACT_RADIUS - 100.0).abs() < 0.1
        && AnthraxBombPayloadTier::from_ocl(ANTHRAX_BOMB_GAMMA_OCL) == AnthraxBombPayloadTier::Gamma
        && AnthraxBombPayloadTier::from_ocl(ANTHRAX_BOMB_OCL) == AnthraxBombPayloadTier::Base
        && resolve_anthrax_bomb_ocl("Chem_GLACommandCenter", [] as [&str; 0])
            == ANTHRAX_BOMB_GAMMA_OCL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn honesty_pack() {
        assert!(honesty_anthrax_bomb_flight_residual_ok());
    }

    #[test]
    fn head_off_map_flies_past_target_then_destroys() {
        // C++ HeadOffMapState + CleanUpState: fly HUGE_DIST, destroy when isOffMap.
        let mut data = HostAnthraxBombFlightData::start(
            Vec3::new(0.0, 150.0, 0.0),
            Vec3::new(100.0, 0.0, 0.0),
            AnthraxBombPayloadTier::Base,
        );
        data.map_min = Vec3::new(0.0, 0.0, 0.0);
        data.map_max = Vec3::new(200.0, 0.0, 200.0);
        data.delivery_complete = true;
        let mut pos = Vec3::new(98.0, 150.0, 0.0);
        let mut left = false;
        let mut destroyed = false;
        for _ in 0..80 {
            let (next, vel, at_exit) = data.tick_transport(pos);
            assert!(
                vel.x > 0.0 || at_exit,
                "must keep flying past the target, pos={next:?}"
            );
            pos = next;
            if pos.x > 100.0 {
                left = true;
            }
            if at_exit {
                destroyed = true;
                break;
            }
        }
        assert!(left, "cargo plane must leave the target, pos={pos:?}");
        assert!(
            destroyed,
            "C++ isOffMap must destroy the transport, pos={pos:?}"
        );
        assert!(pos.x > 200.0 || pos.x < 0.0 || pos.z < 0.0 || pos.z > 200.0);
    }

    #[test]
    fn waits_at_target_until_delivery_then_heads_off() {
        let mut data = HostAnthraxBombFlightData::start(
            Vec3::new(0.0, 150.0, 0.0),
            Vec3::new(100.0, 0.0, 0.0),
            AnthraxBombPayloadTier::Base,
        );
        data.map_min = Vec3::new(0.0, 0.0, 0.0);
        data.map_max = Vec3::new(200.0, 0.0, 200.0);
        let mut pos = Vec3::new(98.0, 150.0, 0.0);
        for _ in 0..8 {
            let (next, _, at_exit) = data.tick_transport(pos);
            pos = next;
            assert!(!at_exit, "must not HeadOffMap during DeliveringState");
        }
        assert!((pos.x - 100.0).abs() < 2.0, "holds moveToPos, pos={pos:?}");
        data.delivery_complete = true;
        let mut left = false;
        let mut destroyed = false;
        for _ in 0..80 {
            let (next, _, at_exit) = data.tick_transport(pos);
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

    #[test]
    fn delivery_band_is_full_140_not_half() {
        let data = HostAnthraxBombFlightData::start(
            Vec3::new(0.0, 150.0, 0.0),
            Vec3::new(200.0, 0.0, 0.0),
            AnthraxBombPayloadTier::Base,
        );
        assert!(data.in_delivery_band(Vec3::new(60.0, 150.0, 0.0)));
        assert!(data.in_delivery_band(Vec3::new(70.0, 150.0, 0.0)));
        assert!(data.in_delivery_band(Vec3::new(200.0 - 140.0, 150.0, 0.0)));
        assert!(!data.in_delivery_band(Vec3::new(200.0 - 141.0, 150.0, 0.0)));
        let plane = Vec3::new(60.0, 150.0, 0.0);
        let drop = anthrax_payload_drop_pos(plane);
        assert!((drop - plane).length() < 0.01);
        assert_ne!(drop.x, data.target.x);
    }

    #[test]
    fn gamma_tier_from_palace_upgrade_names() {
        assert_eq!(
            AnthraxBombPayloadTier::from_player_upgrade_names(["Upgrade_GLAAnthraxBeta"]),
            AnthraxBombPayloadTier::Base
        );
        assert_eq!(
            AnthraxBombPayloadTier::from_player_upgrade_names([
                "Upgrade_GLAAnthraxBeta",
                "Chem_Upgrade_GLAAnthraxGamma",
            ]),
            AnthraxBombPayloadTier::Gamma
        );
        assert_eq!(
            AnthraxBombPayloadTier::Gamma.toxin_object(),
            "PoisonFieldAnthraxGammaBomb"
        );
        assert_eq!(
            AnthraxBombPayloadTier::Base.toxin_object(),
            "PoisonFieldAnthraxBomb"
        );
        assert_eq!(
            AnthraxBombPayloadTier::from_ocl("SUPERWEAPON_AnthraxBombGamma"),
            AnthraxBombPayloadTier::Gamma
        );
        assert_eq!(
            AnthraxBombPayloadTier::Gamma.ocl(),
            "SUPERWEAPON_AnthraxBombGamma"
        );
    }
}
